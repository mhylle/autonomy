import type { EntityState } from '../net/protocol.ts';
import { useHudStore } from '../net/state-store.ts';

interface EntityPanelProps {
  entity: EntityState;
}

/**
 * Bottom panel showing details for the selected entity.
 * Displays drives, energy, age, species info, and a BT placeholder.
 */
export function EntityPanel({ entity }: EntityPanelProps) {
  const close = () => useHudStore.getState().selectEntity(null);

  const energyPct = ((entity.energy / entity.maxEnergy) * 100).toFixed(0);
  const healthPct = ((entity.health / entity.maxHealth) * 100).toFixed(0);
  const agePct = ((entity.age / entity.maxLifespan) * 100).toFixed(0);

  return (
    <div className="entity-panel-bottom">
      <div className="entity-panel-header">
        <h3>Entity #{entity.id}</h3>
        <span className="entity-species-badge" style={speciesBadgeStyle(entity.speciesId)}>
          Species {entity.speciesId}
        </span>
        <button className="close-btn" onClick={close}>x</button>
      </div>

      <div className="entity-panel-body">
        {/* Core Stats */}
        <div className="entity-stats-group">
          <h4>Stats</h4>
          <StatBar label="Energy" value={entity.energy} max={entity.maxEnergy} pct={energyPct} color="#facc15" />
          <StatBar label="Health" value={entity.health} max={entity.maxHealth} pct={healthPct} color="#4ade80" />
          <StatBar label="Age" value={entity.age} max={entity.maxLifespan} pct={agePct} color="#60a5fa" />
          <div className="stat-row">
            <span className="stat-label">Position</span>
            <span className="stat-value">
              ({entity.position.x.toFixed(1)}, {entity.position.y.toFixed(1)})
            </span>
          </div>
          <div className="stat-row">
            <span className="stat-label">Size</span>
            <span className="stat-value">{entity.size.toFixed(2)}</span>
          </div>
          <div className="stat-row">
            <span className="stat-label">Generation</span>
            <span className="stat-value">{entity.generation}</span>
          </div>
        </div>

        {/* BT Visualizer Placeholder */}
        <div className="entity-stats-group">
          <h4>Behavior Tree</h4>
          <div className="bt-placeholder">
            <div className="bt-placeholder-text">
              BT data not yet available in protocol.
            </div>
            <div className="bt-placeholder-hint">
              Requires EntityDetail message with BT structure.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function StatBar({ label, value, max, pct, color }: {
  label: string;
  value: number;
  max: number;
  pct: string;
  color: string;
}) {
  const ratio = Math.min(value / max, 1);
  return (
    <div className="stat-bar-row">
      <span className="stat-label">{label}</span>
      <div className="stat-bar-track">
        <div
          className="stat-bar-fill"
          style={{ width: `${ratio * 100}%`, backgroundColor: color }}
        />
      </div>
      <span className="stat-bar-value">{pct}%</span>
    </div>
  );
}

function speciesBadgeStyle(speciesId: number): React.CSSProperties {
  const hue = (speciesId * 137.508) % 360;
  return {
    backgroundColor: `hsla(${hue}, 70%, 60%, 0.2)`,
    color: `hsl(${hue}, 70%, 70%)`,
    border: `1px solid hsla(${hue}, 70%, 60%, 0.4)`,
  };
}
