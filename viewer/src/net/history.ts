/**
 * Ring buffer for tracking simulation history data.
 *
 * Stores population and per-species counts over time.
 * Lives outside React to avoid GC pressure, similar to worldData.
 * Components poll this data via requestAnimationFrame or timers.
 */

const MAX_HISTORY = 1000;

export interface PopulationSample {
  tick: number;
  totalCount: number;
}

export interface SpeciesSample {
  tick: number;
  /** Map from speciesId to count */
  species: Map<number, number>;
}

class HistoryBuffer<T> {
  private buffer: T[] = [];
  private maxSize: number;

  constructor(maxSize: number) {
    this.maxSize = maxSize;
  }

  push(item: T): void {
    this.buffer.push(item);
    if (this.buffer.length > this.maxSize) {
      // Drop oldest entries in bulk (every 100 overflows) to avoid shifting every tick
      this.buffer = this.buffer.slice(-this.maxSize);
    }
  }

  getAll(): readonly T[] {
    return this.buffer;
  }

  get length(): number {
    return this.buffer.length;
  }

  clear(): void {
    this.buffer = [];
  }
}

export const populationHistory = new HistoryBuffer<PopulationSample>(MAX_HISTORY);
export const speciesHistory = new HistoryBuffer<SpeciesSample>(MAX_HISTORY);

/**
 * Called each tick (or on snapshot) to record current population data.
 * Reads directly from worldData to compute species breakdown.
 */
export function recordHistorySample(
  tick: number,
  entities: Map<number, { speciesId: number }>,
): void {
  // Total population
  populationHistory.push({ tick, totalCount: entities.size });

  // Species breakdown
  const speciesMap = new Map<number, number>();
  for (const entity of entities.values()) {
    const count = speciesMap.get(entity.speciesId) ?? 0;
    speciesMap.set(entity.speciesId, count + 1);
  }
  speciesHistory.push({ tick, species: speciesMap });
}

/**
 * Returns the set of all species IDs seen in the history window.
 */
export function getActiveSpeciesIds(): number[] {
  const ids = new Set<number>();
  for (const sample of speciesHistory.getAll()) {
    for (const id of sample.species.keys()) {
      ids.add(id);
    }
  }
  return Array.from(ids).sort((a, b) => a - b);
}
