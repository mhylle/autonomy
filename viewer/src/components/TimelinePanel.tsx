import { useEffect } from 'react';
import { worldData } from '../net/state-store';
import { useHudStore } from '../net/state-store';

interface TimelineEvent {
  tick: number;
  description: string;
  type: 'war' | 'population' | 'extinction';
}

// Module-level event log — persists across renders
const timelineEvents: TimelineEvent[] = [];
let lastPopulation = 0;
let lastWarCount = 0;

/**
 * Shows a scrollable log of significant simulation events.
 * Tracks population crashes and war declarations.
 */
export function TimelinePanel() {
  const tick = useHudStore((s) => s.tick);
  const entityCount = useHudStore((s) => s.entityCount);

  useEffect(() => {
    // Track population crashes
    if (
      lastPopulation > 0 &&
      entityCount < lastPopulation * 0.5 &&
      entityCount < lastPopulation - 10
    ) {
      timelineEvents.push({
        tick,
        description: `Population crash: ${lastPopulation} → ${entityCount}`,
        type: 'population',
      });
      if (timelineEvents.length > 50) timelineEvents.shift();
    }
    lastPopulation = entityCount;

    // Track war declarations
    const warCount = worldData.activeWars.length;
    if (warCount > lastWarCount) {
      const newWar = worldData.activeWars[warCount - 1];
      if (newWar) {
        timelineEvents.push({
          tick,
          description: `War declared: T${newWar.tribeAId} vs T${newWar.tribeBId}`,
          type: 'war',
        });
        if (timelineEvents.length > 50) timelineEvents.shift();
      }
    }
    lastWarCount = warCount;
  }, [tick, entityCount]);

  return (
    <div className="panel-section">
      <h4>Timeline</h4>
      <div className="timeline-list">
        {timelineEvents.length === 0 && (
          <div className="timeline-empty">No events yet</div>
        )}
        {[...timelineEvents].reverse().map((ev, i) => (
          <div key={i} className={`timeline-event timeline-event-${ev.type}`}>
            <span className="timeline-tick">T{ev.tick}</span>
            <span className="timeline-desc">{ev.description}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
