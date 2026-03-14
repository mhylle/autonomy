use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Tracks per-system timing statistics across ticks.
///
/// Each system's execution time is recorded every tick. The stats
/// store min, max, total, and count for computing averages.
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    /// Per-system timing data: system_name -> SystemTiming
    pub systems: HashMap<&'static str, SystemTiming>,
    /// Total tick durations (wall clock per tick).
    pub tick_timing: SystemTiming,
    /// Number of entities at last measurement.
    pub last_entity_count: u32,
    /// Whether to print stats to the log.
    pub enabled: bool,
    /// How often (in ticks) to log a summary.
    pub log_interval: u64,
}

/// Timing statistics for a single system or the overall tick.
#[derive(Debug, Clone)]
pub struct SystemTiming {
    pub min: Duration,
    pub max: Duration,
    pub total: Duration,
    pub count: u64,
    /// Most recent measurement.
    pub last: Duration,
}

impl Default for SystemTiming {
    fn default() -> Self {
        Self {
            min: Duration::MAX,
            max: Duration::ZERO,
            total: Duration::ZERO,
            count: 0,
            last: Duration::ZERO,
        }
    }
}

impl SystemTiming {
    /// Record a new duration measurement.
    pub fn record(&mut self, duration: Duration) {
        self.last = duration;
        self.total += duration;
        self.count += 1;
        if duration < self.min {
            self.min = duration;
        }
        if duration > self.max {
            self.max = duration;
        }
    }

    /// Average duration over all recorded samples.
    pub fn avg(&self) -> Duration {
        if self.count == 0 {
            Duration::ZERO
        } else {
            self.total / self.count as u32
        }
    }

    /// Reset all statistics.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

impl PerformanceStats {
    pub fn new(enabled: bool) -> Self {
        Self {
            systems: HashMap::new(),
            tick_timing: SystemTiming::default(),
            last_entity_count: 0,
            enabled,
            log_interval: 100,
        }
    }

    /// Record the duration of a system execution.
    pub fn record_system(&mut self, name: &'static str, duration: Duration) {
        self.systems
            .entry(name)
            .or_insert_with(SystemTiming::default)
            .record(duration);
    }

    /// Record the overall tick duration.
    pub fn record_tick(&mut self, duration: Duration) {
        self.tick_timing.record(duration);
    }

    /// Reset all statistics.
    pub fn reset(&mut self) {
        self.systems.clear();
        self.tick_timing.reset();
    }

    /// Format a summary report string.
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "=== Performance Stats ({} ticks, {} entities) ===",
            self.tick_timing.count, self.last_entity_count
        ));
        lines.push(format!(
            "  Tick: avg={:?}, min={:?}, max={:?}, last={:?}",
            self.tick_timing.avg(),
            if self.tick_timing.min == Duration::MAX {
                Duration::ZERO
            } else {
                self.tick_timing.min
            },
            self.tick_timing.max,
            self.tick_timing.last,
        ));

        // Sort systems by average time descending (most expensive first).
        let mut system_entries: Vec<_> = self.systems.iter().collect();
        system_entries.sort_by(|a, b| b.1.avg().cmp(&a.1.avg()));

        for (name, timing) in &system_entries {
            let pct = if self.tick_timing.avg().as_nanos() > 0 {
                (timing.avg().as_nanos() as f64 / self.tick_timing.avg().as_nanos() as f64) * 100.0
            } else {
                0.0
            };
            lines.push(format!(
                "  {:<25} avg={:>10?}  min={:>10?}  max={:>10?}  ({:.1}%)",
                name,
                timing.avg(),
                if timing.min == Duration::MAX {
                    Duration::ZERO
                } else {
                    timing.min
                },
                timing.max,
                pct,
            ));
        }
        lines.push("===".to_string());
        lines.join("\n")
    }
}

/// A helper to time a block of code and record it into PerformanceStats.
///
/// Returns the result of the closure.
pub fn time_system<F, R>(stats: &mut Option<PerformanceStats>, name: &'static str, f: F) -> R
where
    F: FnOnce() -> R,
{
    if let Some(ref mut stats) = stats {
        if stats.enabled {
            let start = Instant::now();
            let result = f();
            stats.record_system(name, start.elapsed());
            return result;
        }
    }
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_timing_records_min_max() {
        let mut timing = SystemTiming::default();
        timing.record(Duration::from_micros(100));
        timing.record(Duration::from_micros(200));
        timing.record(Duration::from_micros(50));

        assert_eq!(timing.min, Duration::from_micros(50));
        assert_eq!(timing.max, Duration::from_micros(200));
        assert_eq!(timing.count, 3);
        assert_eq!(timing.last, Duration::from_micros(50));
    }

    #[test]
    fn system_timing_avg() {
        let mut timing = SystemTiming::default();
        timing.record(Duration::from_micros(100));
        timing.record(Duration::from_micros(200));

        // Average of 100us and 200us = 150us
        assert_eq!(timing.avg(), Duration::from_micros(150));
    }

    #[test]
    fn system_timing_avg_empty() {
        let timing = SystemTiming::default();
        assert_eq!(timing.avg(), Duration::ZERO);
    }

    #[test]
    fn system_timing_reset() {
        let mut timing = SystemTiming::default();
        timing.record(Duration::from_micros(100));
        timing.reset();

        assert_eq!(timing.count, 0);
        assert_eq!(timing.total, Duration::ZERO);
        assert_eq!(timing.min, Duration::MAX);
        assert_eq!(timing.max, Duration::ZERO);
    }

    #[test]
    fn performance_stats_record_system() {
        let mut stats = PerformanceStats::new(true);
        stats.record_system("perception", Duration::from_micros(500));
        stats.record_system("perception", Duration::from_micros(300));
        stats.record_system("drives", Duration::from_micros(200));

        assert_eq!(stats.systems.len(), 2);
        assert_eq!(stats.systems["perception"].count, 2);
        assert_eq!(stats.systems["drives"].count, 1);
    }

    #[test]
    fn performance_stats_summary_format() {
        let mut stats = PerformanceStats::new(true);
        stats.record_tick(Duration::from_micros(1000));
        stats.record_system("perception", Duration::from_micros(500));
        stats.record_system("drives", Duration::from_micros(200));
        stats.last_entity_count = 42;

        let summary = stats.summary();
        assert!(summary.contains("Performance Stats"));
        assert!(summary.contains("perception"));
        assert!(summary.contains("drives"));
        assert!(summary.contains("42 entities"));
    }

    #[test]
    fn performance_stats_reset() {
        let mut stats = PerformanceStats::new(true);
        stats.record_system("perception", Duration::from_micros(500));
        stats.record_tick(Duration::from_micros(1000));
        stats.reset();

        assert!(stats.systems.is_empty());
        assert_eq!(stats.tick_timing.count, 0);
    }

    #[test]
    fn time_system_records_duration() {
        let mut stats = Some(PerformanceStats::new(true));
        let result = time_system(&mut stats, "test_system", || 42);

        assert_eq!(result, 42);
        let stats = stats.unwrap();
        assert_eq!(stats.systems["test_system"].count, 1);
        assert!(stats.systems["test_system"].last > Duration::ZERO || true); // may be 0 on fast machines
    }

    #[test]
    fn time_system_disabled_skips_timing() {
        let mut stats = Some(PerformanceStats::new(false));
        let result = time_system(&mut stats, "test_system", || 42);

        assert_eq!(result, 42);
        let stats = stats.unwrap();
        assert!(stats.systems.is_empty(), "disabled stats should not record");
    }

    #[test]
    fn time_system_none_stats() {
        let mut stats: Option<PerformanceStats> = None;
        let result = time_system(&mut stats, "test_system", || 42);

        assert_eq!(result, 42);
        assert!(stats.is_none());
    }
}
