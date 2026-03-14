/**
 * Memory Inspector panel (Phase 3.3).
 *
 * Displays a placeholder for entity memory data. The memory payload
 * is not yet included in the EntityState protocol message, so this
 * shows a ready-to-populate list layout that will be wired up once
 * the EntityDetail protocol extension is available.
 */
export function MemoryInspector() {
  // Placeholder memory entries showing the intended layout.
  const placeholderEntries = [
    { label: 'Food sources', icon: '\u25CF' },
    { label: 'Threats', icon: '\u25B2' },
    { label: 'Kin encounters', icon: '\u2666' },
    { label: 'Territory', icon: '\u25A0' },
  ];

  return (
    <div className="entity-stats-group">
      <h4>Memory</h4>
      <div className="memory-inspector">
        <div className="memory-placeholder-notice">
          Requires EntityDetail protocol
        </div>
        <ul className="memory-list">
          {placeholderEntries.map((entry) => (
            <li key={entry.label} className="memory-item memory-item-disabled">
              <span className="memory-icon">{entry.icon}</span>
              <span className="memory-label">{entry.label}</span>
              <span className="memory-count">--</span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
