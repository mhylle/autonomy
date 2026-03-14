use serde::{Deserialize, Serialize};

/// Kinds of experiences an entity can remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryKind {
    FoundFood,
    WasAttacked,
    AttackedOther,
    Reproduced,
    Encountered,
    EnvironmentChange,
    NearDeath,
    Migrated,
    /// Culturally transmitted memory: learned by observing a tribemate.
    /// The `associated_entity_id` on the `MemoryEntry` points to the teacher.
    Observed,
}

/// A single remembered experience.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Simulation tick when the event occurred.
    pub tick: u64,
    /// What kind of event was experienced.
    pub kind: MemoryKind,
    /// Subjective importance of this memory (0.0 to 1.0).
    pub importance: f64,
    /// Emotional valence: negative = unpleasant, positive = pleasant (-1.0 to 1.0).
    pub emotional_valence: f64,
    /// World X coordinate where the event occurred.
    pub x: f64,
    /// World Y coordinate where the event occurred.
    pub y: f64,
    /// Optional entity ID associated with the event (e.g. attacker, mate, food source).
    pub associated_entity_id: Option<u64>,
}

/// Genome-encoded weights that control which memories are evicted first
/// when the memory is full.
///
/// Higher weight means that factor contributes more to keeping a memory.
/// The eviction score is a weighted sum; the entry with the *lowest* score
/// is evicted first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictionWeights {
    /// How much recency matters (newer = higher score).
    pub recency_weight: f64,
    /// How much raw importance matters.
    pub importance_weight: f64,
    /// How much emotional intensity (|valence|) matters.
    pub emotional_weight: f64,
    /// How much variety matters (rarer memory kinds score higher).
    pub variety_weight: f64,
}

impl Default for EvictionWeights {
    fn default() -> Self {
        Self {
            recency_weight: 0.4,
            importance_weight: 0.3,
            emotional_weight: 0.2,
            variety_weight: 0.1,
        }
    }
}

/// Memory component: stores a bounded collection of remembered experiences.
///
/// When the number of entries exceeds `capacity`, the entry with the lowest
/// eviction score is removed. The eviction score is a weighted combination
/// of recency, importance, emotional intensity, and variety.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Stored memory entries.
    pub entries: Vec<MemoryEntry>,
    /// Maximum number of memories this entity can hold.
    pub capacity: usize,
    /// Weights controlling which memories survive eviction.
    pub eviction_weights: EvictionWeights,
}

impl Memory {
    /// Create a new Memory with the given capacity and eviction weights.
    pub fn new(capacity: usize, eviction_weights: EvictionWeights) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            eviction_weights,
        }
    }

    /// Add a memory entry, evicting the lowest-scored entry if at capacity.
    ///
    /// `current_tick` is needed to compute recency scores for eviction.
    pub fn add(&mut self, entry: MemoryEntry, current_tick: u64) {
        if self.entries.len() >= self.capacity {
            self.evict_lowest(current_tick);
        }
        self.entries.push(entry);
    }

    /// Recall memories filtered by kind and maximum age (in ticks).
    ///
    /// Returns references to matching entries, most recent first.
    pub fn recall(&self, kind: MemoryKind, max_age: u64, current_tick: u64) -> Vec<&MemoryEntry> {
        let min_tick = current_tick.saturating_sub(max_age);
        let mut results: Vec<&MemoryEntry> = self
            .entries
            .iter()
            .filter(|e| e.kind == kind && e.tick >= min_tick)
            .collect();
        results.sort_by(|a, b| b.tick.cmp(&a.tick));
        results
    }

    /// Recall all memories within a maximum age, regardless of kind.
    ///
    /// Returns references to matching entries, most recent first.
    pub fn recall_all(&self, max_age: u64, current_tick: u64) -> Vec<&MemoryEntry> {
        let min_tick = current_tick.saturating_sub(max_age);
        let mut results: Vec<&MemoryEntry> = self
            .entries
            .iter()
            .filter(|e| e.tick >= min_tick)
            .collect();
        results.sort_by(|a, b| b.tick.cmp(&a.tick));
        results
    }

    /// Compute eviction score for a single entry.
    ///
    /// Higher score = more worth keeping.
    fn eviction_score(&self, entry: &MemoryEntry, current_tick: u64) -> f64 {
        let w = &self.eviction_weights;

        // Recency: normalized so that tick == current_tick gives 1.0,
        // and very old entries approach 0.0.
        let age = current_tick.saturating_sub(entry.tick) as f64;
        let recency = 1.0 / (1.0 + age * 0.01);

        // Importance: already 0.0..1.0.
        let importance = entry.importance;

        // Emotional intensity: absolute valence, already 0.0..1.0.
        let emotional = entry.emotional_valence.abs();

        // Variety: proportion of entries that are NOT this kind.
        let same_kind_count = self
            .entries
            .iter()
            .filter(|e| e.kind == entry.kind)
            .count();
        let variety = if self.entries.is_empty() {
            0.0
        } else {
            1.0 - (same_kind_count as f64 / self.entries.len() as f64)
        };

        w.recency_weight * recency
            + w.importance_weight * importance
            + w.emotional_weight * emotional
            + w.variety_weight * variety
    }

    /// Remove the entry with the lowest eviction score.
    fn evict_lowest(&mut self, current_tick: u64) {
        if self.entries.is_empty() {
            return;
        }

        let mut lowest_idx = 0;
        let mut lowest_score = f64::MAX;

        for (i, entry) in self.entries.iter().enumerate() {
            let score = self.eviction_score(entry, current_tick);
            if score < lowest_score {
                lowest_score = score;
                lowest_idx = i;
            }
        }

        self.entries.swap_remove(lowest_idx);
    }

    /// Number of stored memories.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the memory store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new(20, EvictionWeights::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(tick: u64, kind: MemoryKind, importance: f64, valence: f64) -> MemoryEntry {
        MemoryEntry {
            tick,
            kind,
            importance,
            emotional_valence: valence,
            x: 0.0,
            y: 0.0,
            associated_entity_id: None,
        }
    }

    #[test]
    fn default_memory_has_capacity_20() {
        let m = Memory::default();
        assert_eq!(m.capacity, 20);
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn add_entry_increases_len() {
        let mut m = Memory::new(5, EvictionWeights::default());
        m.add(make_entry(1, MemoryKind::FoundFood, 0.5, 0.5), 1);
        assert_eq!(m.len(), 1);
        m.add(make_entry(2, MemoryKind::WasAttacked, 0.8, -0.9), 2);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn capacity_enforced_on_add() {
        let mut m = Memory::new(3, EvictionWeights::default());
        m.add(make_entry(1, MemoryKind::FoundFood, 0.1, 0.1), 10);
        m.add(make_entry(2, MemoryKind::FoundFood, 0.2, 0.2), 10);
        m.add(make_entry(3, MemoryKind::FoundFood, 0.3, 0.3), 10);
        assert_eq!(m.len(), 3);

        // Adding a 4th should evict one, keeping len at 3.
        m.add(make_entry(10, MemoryKind::NearDeath, 1.0, -1.0), 10);
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn eviction_removes_lowest_scored_entry() {
        let mut m = Memory::new(3, EvictionWeights::default());

        // Old, low importance, low emotion => should be evicted first.
        m.add(make_entry(1, MemoryKind::FoundFood, 0.0, 0.0), 100);
        // Recent, high importance.
        m.add(make_entry(99, MemoryKind::NearDeath, 1.0, -1.0), 100);
        // Recent, moderate importance.
        m.add(make_entry(98, MemoryKind::Reproduced, 0.7, 0.8), 100);

        // Now add a 4th -- the old low-importance one should be evicted.
        m.add(
            make_entry(100, MemoryKind::Encountered, 0.5, 0.3),
            100,
        );
        assert_eq!(m.len(), 3);

        // The old FoundFood entry (tick=1, importance=0, valence=0) should be gone.
        let food_entries = m.recall(MemoryKind::FoundFood, u64::MAX, 100);
        assert!(
            food_entries.is_empty(),
            "low-scored FoundFood should have been evicted"
        );
    }

    #[test]
    fn recall_filters_by_kind() {
        let mut m = Memory::new(10, EvictionWeights::default());
        m.add(make_entry(1, MemoryKind::FoundFood, 0.5, 0.5), 10);
        m.add(make_entry(2, MemoryKind::WasAttacked, 0.8, -0.9), 10);
        m.add(make_entry(3, MemoryKind::FoundFood, 0.6, 0.4), 10);

        let food = m.recall(MemoryKind::FoundFood, u64::MAX, 10);
        assert_eq!(food.len(), 2);
        let attacked = m.recall(MemoryKind::WasAttacked, u64::MAX, 10);
        assert_eq!(attacked.len(), 1);
    }

    #[test]
    fn recall_filters_by_max_age() {
        let mut m = Memory::new(10, EvictionWeights::default());
        m.add(make_entry(10, MemoryKind::FoundFood, 0.5, 0.5), 10);
        m.add(make_entry(50, MemoryKind::FoundFood, 0.6, 0.4), 50);
        m.add(make_entry(90, MemoryKind::FoundFood, 0.7, 0.3), 90);

        // At tick 100, max_age=20 means only entries with tick >= 80.
        let recent = m.recall(MemoryKind::FoundFood, 20, 100);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].tick, 90);
    }

    #[test]
    fn recall_returns_most_recent_first() {
        let mut m = Memory::new(10, EvictionWeights::default());
        m.add(make_entry(5, MemoryKind::Encountered, 0.5, 0.0), 10);
        m.add(make_entry(1, MemoryKind::Encountered, 0.5, 0.0), 10);
        m.add(make_entry(8, MemoryKind::Encountered, 0.5, 0.0), 10);

        let results = m.recall(MemoryKind::Encountered, u64::MAX, 10);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].tick, 8);
        assert_eq!(results[1].tick, 5);
        assert_eq!(results[2].tick, 1);
    }

    #[test]
    fn recall_all_returns_all_kinds() {
        let mut m = Memory::new(10, EvictionWeights::default());
        m.add(make_entry(1, MemoryKind::FoundFood, 0.5, 0.5), 10);
        m.add(make_entry(2, MemoryKind::WasAttacked, 0.8, -0.9), 10);
        m.add(make_entry(3, MemoryKind::Reproduced, 0.9, 0.9), 10);

        let all = m.recall_all(u64::MAX, 10);
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn recall_all_respects_max_age() {
        let mut m = Memory::new(10, EvictionWeights::default());
        m.add(make_entry(10, MemoryKind::FoundFood, 0.5, 0.5), 10);
        m.add(make_entry(90, MemoryKind::WasAttacked, 0.8, -0.9), 90);

        let recent = m.recall_all(20, 100);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].kind, MemoryKind::WasAttacked);
    }

    #[test]
    fn eviction_weights_default_sum_to_one() {
        let w = EvictionWeights::default();
        let sum = w.recency_weight + w.importance_weight + w.emotional_weight + w.variety_weight;
        assert!(
            (sum - 1.0).abs() < f64::EPSILON,
            "default weights should sum to 1.0, got {}",
            sum
        );
    }

    #[test]
    fn memory_entry_with_associated_entity() {
        let entry = MemoryEntry {
            tick: 42,
            kind: MemoryKind::WasAttacked,
            importance: 0.9,
            emotional_valence: -0.8,
            x: 10.0,
            y: 20.0,
            associated_entity_id: Some(12345),
        };
        assert_eq!(entry.associated_entity_id, Some(12345));
        assert_eq!(entry.kind, MemoryKind::WasAttacked);
    }

    #[test]
    fn memory_kind_all_variants_exist() {
        // Ensure all 9 variants compile and can be compared.
        let kinds = [
            MemoryKind::FoundFood,
            MemoryKind::WasAttacked,
            MemoryKind::AttackedOther,
            MemoryKind::Reproduced,
            MemoryKind::Encountered,
            MemoryKind::EnvironmentChange,
            MemoryKind::NearDeath,
            MemoryKind::Migrated,
            MemoryKind::Observed,
        ];
        assert_eq!(kinds.len(), 9);
        // All should be distinct.
        for i in 0..kinds.len() {
            for j in (i + 1)..kinds.len() {
                assert_ne!(kinds[i], kinds[j]);
            }
        }
    }

    #[test]
    fn serialization_roundtrip_memory() {
        let mut m = Memory::new(5, EvictionWeights::default());
        m.add(make_entry(1, MemoryKind::FoundFood, 0.5, 0.3), 1);
        m.add(make_entry(2, MemoryKind::NearDeath, 1.0, -1.0), 2);

        let json = serde_json::to_string(&m).unwrap();
        let restored: Memory = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.capacity, 5);
        assert_eq!(restored.len(), 2);
        assert_eq!(restored.entries[0].kind, MemoryKind::FoundFood);
        assert_eq!(restored.entries[1].kind, MemoryKind::NearDeath);
    }

    #[test]
    fn serialization_roundtrip_eviction_weights() {
        let w = EvictionWeights {
            recency_weight: 0.5,
            importance_weight: 0.2,
            emotional_weight: 0.1,
            variety_weight: 0.2,
        };
        let json = serde_json::to_string(&w).unwrap();
        let restored: EvictionWeights = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.recency_weight, 0.5);
        assert_eq!(restored.importance_weight, 0.2);
        assert_eq!(restored.emotional_weight, 0.1);
        assert_eq!(restored.variety_weight, 0.2);
    }

    #[test]
    fn eviction_prefers_keeping_recent_important_emotional_entries() {
        let mut m = Memory::new(2, EvictionWeights::default());

        // Entry A: old, low importance, no emotion.
        m.add(make_entry(1, MemoryKind::FoundFood, 0.1, 0.0), 100);
        // Entry B: recent, high importance, strong emotion.
        m.add(make_entry(99, MemoryKind::NearDeath, 1.0, -1.0), 100);

        // Add a third entry -- entry A should be evicted.
        m.add(make_entry(100, MemoryKind::Reproduced, 0.5, 0.5), 100);

        assert_eq!(m.len(), 2);
        let kinds: Vec<MemoryKind> = m.entries.iter().map(|e| e.kind).collect();
        assert!(!kinds.contains(&MemoryKind::FoundFood), "old low-scored entry should be evicted");
        assert!(kinds.contains(&MemoryKind::NearDeath));
        assert!(kinds.contains(&MemoryKind::Reproduced));
    }

    #[test]
    fn variety_weight_protects_unique_kinds() {
        // Use weights that heavily favor variety.
        let weights = EvictionWeights {
            recency_weight: 0.0,
            importance_weight: 0.0,
            emotional_weight: 0.0,
            variety_weight: 1.0,
        };
        let mut m = Memory::new(3, weights);

        // Two FoundFood entries and one NearDeath.
        m.add(make_entry(1, MemoryKind::FoundFood, 0.5, 0.5), 10);
        m.add(make_entry(2, MemoryKind::FoundFood, 0.5, 0.5), 10);
        m.add(make_entry(3, MemoryKind::NearDeath, 0.5, 0.5), 10);

        // Adding a 4th: one of the duplicated FoundFood should be evicted,
        // not the unique NearDeath.
        m.add(make_entry(4, MemoryKind::Migrated, 0.5, 0.5), 10);

        assert_eq!(m.len(), 3);
        let food_count = m.entries.iter().filter(|e| e.kind == MemoryKind::FoundFood).count();
        assert!(
            food_count <= 1,
            "variety weighting should evict duplicated kind; found {} FoundFood entries",
            food_count
        );
    }

    #[test]
    fn empty_recall_returns_empty() {
        let m = Memory::new(10, EvictionWeights::default());
        let results = m.recall(MemoryKind::FoundFood, 100, 50);
        assert!(results.is_empty());
        let all = m.recall_all(100, 50);
        assert!(all.is_empty());
    }

    #[test]
    fn add_to_zero_capacity_still_works() {
        // Edge case: capacity of 1.
        let mut m = Memory::new(1, EvictionWeights::default());
        m.add(make_entry(1, MemoryKind::FoundFood, 0.5, 0.5), 1);
        assert_eq!(m.len(), 1);
        m.add(make_entry(2, MemoryKind::NearDeath, 1.0, -1.0), 2);
        assert_eq!(m.len(), 1);
        assert_eq!(m.entries[0].kind, MemoryKind::NearDeath);
    }

    #[test]
    fn location_stored_in_memory_entry() {
        let entry = MemoryEntry {
            tick: 1,
            kind: MemoryKind::Migrated,
            importance: 0.5,
            emotional_valence: 0.0,
            x: 123.45,
            y: 678.90,
            associated_entity_id: None,
        };
        assert_eq!(entry.x, 123.45);
        assert_eq!(entry.y, 678.90);
    }
}
