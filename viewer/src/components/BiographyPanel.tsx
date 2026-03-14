import { useEffect } from 'react';
import type { EntityState } from '../net/protocol';
import { worldData, useHudStore } from '../net/state-store';

interface BiographyEvent {
  tick: number;
  description: string;
}

// Track entity biography events globally (outside React to avoid re-render cost)
const entityBiographies = new Map<number, BiographyEvent[]>();
const lastEntityEnergies = new Map<number, number>();

/**
 * Called each tick to accumulate biography events for all entities.
 * Detects significant energy gains (feeding) and birth events.
 */
function updateBiographies(tick: number): void {
  for (const [id, entity] of worldData.entities) {
    const lastEnergy = lastEntityEnergies.get(id) ?? entity.energy;

    if (!entityBiographies.has(id)) {
      entityBiographies.set(id, [{ tick, description: 'Entity born' }]);
    }

    // Detect feeding (energy increased significantly)
    if (entity.energy > lastEnergy + 5) {
      const events = entityBiographies.get(id)!;
      if (events.length < 20) {
        events.push({
          tick,
          description: `Fed (+${(entity.energy - lastEnergy).toFixed(1)} energy)`,
        });
      }
    }

    lastEntityEnergies.set(id, entity.energy);
  }
}

interface Props {
  entity: EntityState;
}

export function BiographyPanel({ entity }: Props) {
  const tick = useHudStore((s) => s.tick);

  useEffect(() => {
    updateBiographies(tick);
  }, [tick]);

  const events = entityBiographies.get(entity.id) ?? [];
  const tribeInfo = entity.tribeId ? worldData.tribes.get(entity.tribeId) : null;

  return (
    <div className="entity-stats-group">
      <h4>Biography</h4>
      <div className="biography-info">
        <div className="bio-row">
          <span className="bio-label">Generation</span>
          <span>{entity.generation}</span>
        </div>
        <div className="bio-row">
          <span className="bio-label">Species</span>
          <span>{entity.speciesId}</span>
        </div>
        {tribeInfo && (
          <div className="bio-row">
            <span className="bio-label">Tribe</span>
            <span style={{ color: `hsl(${(entity.tribeId * 137.5) % 360}, 70%, 70%)` }}>
              T{entity.tribeId} ({tribeInfo.memberCount} members)
            </span>
          </div>
        )}
        {entity.isCompositeLeader && (
          <div className="bio-row">
            <span className="bio-label">Role</span>
            <span className="composite-badge">
              Composite Leader ({entity.compositeMemberCount} cells)
            </span>
          </div>
        )}
      </div>
      <div className="biography-timeline">
        <div className="bio-timeline-header">Event Log</div>
        {events.length === 0 && (
          <div className="bio-empty">No events recorded</div>
        )}
        {[...events].reverse().slice(0, 10).map((ev, i) => (
          <div key={i} className="bio-event">
            <span className="bio-event-tick">T{ev.tick}</span>
            <span>{ev.description}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
