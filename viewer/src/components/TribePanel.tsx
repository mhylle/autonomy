import { worldData } from '../net/state-store';
import { useHudStore } from '../net/state-store';

/**
 * Panel showing active tribes and currently active wars.
 * Re-renders on each tick by subscribing to the tick counter.
 */
export function TribePanel() {
  // Subscribe to tick so the panel refreshes as data changes
  useHudStore((s) => s.tick);

  const tribes = Array.from(worldData.tribes.values());
  const wars = worldData.activeWars;

  return (
    <div className="panel-section">
      <h4>Tribes ({tribes.length})</h4>
      {tribes.length === 0 ? (
        <div className="timeline-empty">No tribes active</div>
      ) : (
        <table className="tribe-table">
          <thead>
            <tr>
              <th>ID</th>
              <th>Members</th>
              <th>Centroid</th>
            </tr>
          </thead>
          <tbody>
            {tribes.map((t) => (
              <tr
                key={t.id}
                style={{ color: `hsl(${(t.id * 137.5) % 360}, 70%, 70%)` }}
              >
                <td>T{t.id}</td>
                <td>{t.memberCount}</td>
                <td>
                  ({t.centroidX.toFixed(0)}, {t.centroidY.toFixed(0)})
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {wars.length > 0 && (
        <div className="war-list">
          <h4>Active Wars ({wars.length})</h4>
          {wars.map((w, i) => (
            <div key={i} className="war-item">
              T{w.tribeAId} vs T{w.tribeBId} &mdash; since tick {w.declaredTick}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
