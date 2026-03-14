import { useHudStore, worldData } from '../net/state-store';

export function SettlementPanel() {
  // Subscribe to tick so the panel re-renders when world data updates
  useHudStore((s) => s.tick);

  const settlements = Array.from(worldData.settlements.values());
  const tradeRoutes = worldData.tradeRoutes;
  const culturalProfiles = Array.from(worldData.culturalProfiles.values());

  if (settlements.length === 0) {
    return (
      <div className="panel-section">
        <h4>Settlements</h4>
        <div className="panel-empty">
          No settlements detected yet. Settlements form when tribes persistently occupy a region.
        </div>
      </div>
    );
  }

  return (
    <div className="panel-section">
      <h4>Settlements ({settlements.length})</h4>
      <div className="settlement-list">
        {settlements.map((s) => {
          const culture = culturalProfiles.find((c) => c.tribeId === s.tribeId);
          return (
            <div key={s.id} className="settlement-card">
              <div
                className="settlement-name"
                style={{ color: `hsl(${(s.tribeId * 137.5) % 360}, 70%, 70%)` }}
              >
                {s.name}
              </div>
              <div className="settlement-details">
                <span>Pop: {s.population}</span>
                <span>Def: {s.defenseScore.toFixed(1)}</span>
                <span>Founded: T{s.foundingTick}</span>
              </div>
              {culture && (
                <div className="settlement-culture">
                  Culture complexity: {culture.complexity.toFixed(2)}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {tradeRoutes.length > 0 && (
        <>
          <h4>Trade Routes ({tradeRoutes.length})</h4>
          <div className="trade-route-list">
            {tradeRoutes.map((r, i) => (
              <div key={i} className="trade-route-item">
                S{r.fromSettlement} &rarr; S{r.toSettlement}
                {r.resourceType && (
                  <span className="trade-resource"> ({r.resourceType})</span>
                )}
                <span className="trade-volume"> vol: {r.volume}</span>
              </div>
            ))}
          </div>
        </>
      )}

      {culturalProfiles.length > 0 && (
        <>
          <h4>Cultural Profiles</h4>
          <div className="culture-list">
            {culturalProfiles.map((c) => (
              <div key={c.tribeId} className="culture-item">
                <span style={{ color: `hsl(${(c.tribeId * 137.5) % 360}, 70%, 70%)` }}>
                  T{c.tribeId}
                </span>
                <span> complexity: {c.complexity.toFixed(2)}</span>
                {c.signalSummary && (
                  <span className="signal-summary"> signals: {c.signalSummary}</span>
                )}
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
