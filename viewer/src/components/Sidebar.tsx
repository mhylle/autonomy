import { useState } from 'react';
import { PopulationChart } from './PopulationChart.tsx';
import { SpeciesChart } from './SpeciesChart.tsx';
import { TribePanel } from './TribePanel.tsx';
import { AnalyticsPanel } from './AnalyticsPanel.tsx';
import { TimelinePanel } from './TimelinePanel.tsx';
import { SettlementPanel } from './SettlementPanel.tsx';
import { NarrationPanel } from './NarrationPanel.tsx';

type Tab = 'charts' | 'tribes' | 'analytics' | 'timeline' | 'settlements' | 'narration';

/**
 * Collapsible right sidebar with tabbed panels.
 * Tabs: Charts, Tribes, Map (species range), Events (timeline).
 */
export function Sidebar() {
  const [collapsed, setCollapsed] = useState(false);
  const [tab, setTab] = useState<Tab>('charts');

  return (
    <div className={`sidebar ${collapsed ? 'sidebar-collapsed' : ''}`}>
      <button
        className="sidebar-toggle"
        onClick={() => setCollapsed(!collapsed)}
        title={collapsed ? 'Show sidebar' : 'Hide sidebar'}
      >
        {collapsed ? '\u25C0' : '\u25B6'}
      </button>

      {!collapsed && (
        <>
          <div className="sidebar-tabs">
            <button
              className={`tab-btn ${tab === 'charts' ? 'active' : ''}`}
              onClick={() => setTab('charts')}
            >
              Charts
            </button>
            <button
              className={`tab-btn ${tab === 'tribes' ? 'active' : ''}`}
              onClick={() => setTab('tribes')}
            >
              Tribes
            </button>
            <button
              className={`tab-btn ${tab === 'analytics' ? 'active' : ''}`}
              onClick={() => setTab('analytics')}
            >
              Map
            </button>
            <button
              className={`tab-btn ${tab === 'timeline' ? 'active' : ''}`}
              onClick={() => setTab('timeline')}
            >
              Events
            </button>
            <button
              className={`tab-btn ${tab === 'settlements' ? 'active' : ''}`}
              onClick={() => setTab('settlements')}
            >
              Civs
            </button>
            <button
              className={`tab-btn ${tab === 'narration' ? 'active' : ''}`}
              onClick={() => setTab('narration')}
            >
              Story
            </button>
          </div>

          <div className="sidebar-content">
            {tab === 'charts' && (
              <>
                <PopulationChart />
                <SpeciesChart />
              </>
            )}
            {tab === 'tribes' && <TribePanel />}
            {tab === 'analytics' && <AnalyticsPanel />}
            {tab === 'timeline' && <TimelinePanel />}
            {tab === 'settlements' && <SettlementPanel />}
            {tab === 'narration' && <NarrationPanel />}
          </div>
        </>
      )}
    </div>
  );
}
