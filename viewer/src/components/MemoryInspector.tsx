/**
 * Memory Inspector panel (Phase 3.3).
 *
 * Shows a polished memory journal layout. Each section is ready to
 * receive real data once the EntityDetail protocol extension is available.
 * Until then, all sections display a "no data" placeholder.
 */

interface MemorySection {
  label: string;
  icon: string;
  color: string;
}

const MEMORY_SECTIONS: MemorySection[] = [
  { label: 'Food Events',     icon: '\u25CF', color: '#4ade80' },
  { label: 'Combat Events',   icon: '\u25B2', color: '#ef4444' },
  { label: 'Social Events',   icon: '\u2666', color: '#60a5fa' },
  { label: 'Territory Events',icon: '\u25A0', color: '#facc15' },
];

export function MemoryInspector() {
  return (
    <div className="entity-stats-group">
      <h4>Memory Journal</h4>
      <div className="memory-inspector">
        <div className="memory-placeholder-notice">
          Requires EntityDetail protocol (not yet implemented)
        </div>
        <div className="memory-journal">
          {MEMORY_SECTIONS.map((section) => (
            <div key={section.label} className="memory-journal-section">
              <div className="memory-journal-section-header">
                <span className="memory-icon" style={{ color: section.color }}>{section.icon}</span>
                <span className="memory-journal-section-label">{section.label}</span>
              </div>
              <div className="memory-journal-section-body">
                <span className="memory-no-data">No data — EntityDetail protocol required</span>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
