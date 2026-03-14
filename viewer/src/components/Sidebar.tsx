import { useState } from 'react';
import { PopulationChart } from './PopulationChart.tsx';
import { SpeciesChart } from './SpeciesChart.tsx';

/**
 * Collapsible right sidebar containing population and species charts.
 */
export function Sidebar() {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <div className={`sidebar ${collapsed ? 'sidebar-collapsed' : ''}`}>
      <button
        className="sidebar-toggle"
        onClick={() => setCollapsed(!collapsed)}
        title={collapsed ? 'Show charts' : 'Hide charts'}
      >
        {collapsed ? '\u25C0' : '\u25B6'}
      </button>

      {!collapsed && (
        <div className="sidebar-content">
          <PopulationChart />
          <SpeciesChart />
        </div>
      )}
    </div>
  );
}
