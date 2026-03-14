import { useEffect, useRef, useCallback } from 'react';
import { WorldRenderer } from './renderer/WorldRenderer';
import { SimulationClient } from './net/websocket';
import { useHudStore, worldData } from './net/state-store';
import { Sidebar } from './components/Sidebar.tsx';
import { EntityPanel } from './components/EntityPanel.tsx';
import './App.css';

const SPEED_OPTIONS = [0.5, 1, 2, 5, 10];

function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const minimapRef = useRef<HTMLCanvasElement>(null);
  const rendererRef = useRef<WorldRenderer | null>(null);
  const clientRef = useRef<SimulationClient | null>(null);

  // Only subscribe to HUD values — no entity data in React
  const tick = useHudStore((s) => s.tick);
  const entityCount = useHudStore((s) => s.entityCount);
  const connected = useHudStore((s) => s.connected);
  const fps = useHudStore((s) => s.fps);
  const paused = useHudStore((s) => s.paused);
  const speedMultiplier = useHudStore((s) => s.speedMultiplier);
  const selectedEntityId = useHudStore((s) => s.selectedEntityId);

  // Initialize renderer and WebSocket — runs once
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const renderer = new WorldRenderer();
    rendererRef.current = renderer;

    const wsUrl = `ws://${window.location.hostname}:9001/ws`;
    const client = new SimulationClient(wsUrl);
    clientRef.current = client;
    client.connect();

    renderer.init(canvas).then(() => {
      renderer.setOnEntityClick((id) => {
        useHudStore.getState().selectEntity(id);
      });
      renderer.setOnViewportChange((bounds) => {
        client.sendViewportUpdate(bounds);
      });
      if (minimapRef.current) {
        renderer.initMinimap(minimapRef.current);
      }
      // Send initial viewport so server knows our bounds from the start.
      client.sendViewportUpdate(renderer.getViewportBounds());
    });

    return () => {
      client.disconnect();
      renderer.destroy();
    };
  }, []);

  const handleTogglePause = useCallback(() => {
    const client = clientRef.current;
    if (!client) return;
    const newPaused = !useHudStore.getState().paused;
    useHudStore.getState().togglePause();
    client.sendCommand({ type: newPaused ? 'pause' : 'resume' });
  }, []);

  const handleSetSpeed = useCallback((speed: number) => {
    const client = clientRef.current;
    if (!client) return;
    useHudStore.getState().setSpeedMultiplier(speed);
    client.sendCommand({ type: 'set_speed', speed });
  }, []);

  // Look up selected entity from worldData (not from store)
  const selectedEntity = selectedEntityId ? worldData.entities.get(selectedEntityId) : null;

  return (
    <div className="app">
      <canvas ref={canvasRef} className="world-canvas" />

      {/* HUD */}
      <div className="hud">
        <div className="hud-item">Tick: {tick}</div>
        <div className="hud-item">Entities: {entityCount}</div>
        <div className="hud-item">FPS: {fps}</div>
        <div className={`hud-item ${connected ? 'connected' : 'disconnected'}`}>
          {connected ? 'Connected' : 'Disconnected'}
        </div>
      </div>

      {/* Simulation Controls */}
      <div className="sim-controls">
        <button
          className={`ctrl-btn ${paused ? 'paused' : ''}`}
          onClick={handleTogglePause}
          title={paused ? 'Resume' : 'Pause'}
        >
          {paused ? '\u25B6' : '\u23F8'}
        </button>
        <div className="speed-buttons">
          {SPEED_OPTIONS.map((speed) => (
            <button
              key={speed}
              className={`ctrl-btn speed-btn ${speedMultiplier === speed ? 'active' : ''}`}
              onClick={() => handleSetSpeed(speed)}
            >
              {speed}x
            </button>
          ))}
        </div>
      </div>

      {/* Charts Sidebar */}
      <Sidebar />

      {/* Minimap */}
      <canvas ref={minimapRef} className="minimap" width={200} height={200} />

      {/* Entity Panel (bottom) */}
      {selectedEntity && <EntityPanel entity={selectedEntity} />}
    </div>
  );
}

export default App;
