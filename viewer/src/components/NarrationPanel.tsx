import { useEffect, useState } from 'react';
import { worldData, useHudStore } from '../net/state-store';

// Module-level narration log and counters (persist across re-renders)
const narrationLog: string[] = [];
let prevEntityCount = 0;
let prevWarCount = 0;
let prevSettlementCount = 0;

/**
 * Generates narration text entries based on simulation state changes.
 * Called each tick; entries are prepended so newest is at the top.
 */
function generateNarration(tick: number, entityCount: number): void {
  const warCount = worldData.activeWars.length;
  const tribeCount = worldData.tribes.size;
  const settlementCount = worldData.settlements.size;

  // Periodic world status
  if (tick % 100 === 0 && entityCount > 0) {
    narrationLog.unshift(
      `Tick ${tick}: The world holds ${entityCount} living creatures across ${tribeCount} tribes.`
    );
  }

  // War events
  if (warCount > prevWarCount && worldData.activeWars.length > 0) {
    const war = worldData.activeWars[0];
    narrationLog.unshift(
      `A war has broken out between Tribe ${war.tribeAId} and Tribe ${war.tribeBId}!`
    );
  }

  // Settlement events
  if (settlementCount > prevSettlementCount) {
    narrationLog.unshift(
      `A new settlement emerges from the growing tribe clusters.`
    );
  }

  // Population milestones
  if (prevEntityCount > 0) {
    const change = entityCount - prevEntityCount;
    if (change < -20) {
      narrationLog.unshift(
        `A great dying: population fell by ${Math.abs(change)} souls.`
      );
    }
    if (change > 20) {
      narrationLog.unshift(
        `A flourishing: ${change} new creatures joined the world.`
      );
    }
  }

  // Keep only the last 50 entries
  while (narrationLog.length > 50) narrationLog.pop();

  prevEntityCount = entityCount;
  prevWarCount = warCount;
  prevSettlementCount = settlementCount;
}

export function NarrationPanel() {
  const tick = useHudStore((s) => s.tick);
  const entityCount = useHudStore((s) => s.entityCount);
  const [log, setLog] = useState<string[]>([]);

  useEffect(() => {
    generateNarration(tick, entityCount);
    setLog([...narrationLog]);
  }, [tick, entityCount]);

  return (
    <div className="panel-section">
      <h4>Narration</h4>
      <div className="narration-log">
        {log.length === 0 && (
          <div className="narration-empty">Waiting for events...</div>
        )}
        {log.map((entry, i) => (
          <div key={i} className="narration-entry">
            {entry}
          </div>
        ))}
      </div>
    </div>
  );
}
