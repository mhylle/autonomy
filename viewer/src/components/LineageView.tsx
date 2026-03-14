import type { EntityState } from '../net/protocol.ts';

interface LineageViewProps {
  entity: EntityState;
}

/**
 * Lineage tree display (Phase 3.5).
 *
 * Shows the selected entity's ancestry as a vertical chain of
 * generation markers. Since parentId is not yet in the protocol,
 * we display the generation chain from Gen 0 up to the current
 * generation, highlighting the entity's position.
 */
export function LineageView({ entity }: LineageViewProps) {
  const gen = entity.generation;

  // Build generation chain — show up to 6 ancestors plus current,
  // with ellipsis if the chain is longer.
  const MAX_VISIBLE = 7;
  const showEllipsis = gen >= MAX_VISIBLE;
  const startGen = showEllipsis ? gen - (MAX_VISIBLE - 2) : 0;

  const steps: number[] = [];
  for (let g = startGen; g <= gen; g++) {
    steps.push(g);
  }

  return (
    <div className="entity-stats-group">
      <h4>Lineage</h4>
      <div className="lineage-view">
        <div className="lineage-chain">
          {showEllipsis && (
            <>
              <span className="lineage-node lineage-node-ancestor">
                Gen 0
              </span>
              <span className="lineage-arrow">{'\u22EF'}</span>
            </>
          )}
          {steps.map((g, i) => (
            <span key={g} className="lineage-step">
              <span
                className={
                  `lineage-node ${g === gen ? 'lineage-node-current' : 'lineage-node-ancestor'}`
                }
              >
                Gen {g}
                {g === gen && ' (current)'}
              </span>
              {i < steps.length - 1 && (
                <span className="lineage-arrow">{'\u2192'}</span>
              )}
            </span>
          ))}
        </div>
        {gen === 0 && (
          <div className="lineage-hint">Primordial entity (no ancestors)</div>
        )}
      </div>
    </div>
  );
}
