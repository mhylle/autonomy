import { useRef, useEffect } from 'react';
import { worldData } from '../net/state-store';
import { useHudStore } from '../net/state-store';

/**
 * Analytics panel showing a small species range map canvas.
 * The canvas is redrawn on every tick.
 */
export function AnalyticsPanel() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const tick = useHudStore((s) => s.tick);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const W = canvas.width;
    const H = canvas.height;

    ctx.fillStyle = '#111';
    ctx.fillRect(0, 0, W, H);

    const ww = worldData.worldWidth || 500;
    const wh = worldData.worldHeight || 500;

    for (const entity of worldData.entities.values()) {
      const px = (entity.position.x / ww) * W;
      const py = (entity.position.y / wh) * H;
      const hue = (entity.speciesId * 137.5) % 360;
      ctx.fillStyle = `hsl(${hue}, 70%, 60%)`;
      ctx.beginPath();
      ctx.arc(px, py, 1.5, 0, Math.PI * 2);
      ctx.fill();
    }
  }, [tick]);

  const structureCount = worldData.structures?.length ?? 0;

  return (
    <div className="panel-section">
      <h4>Species Range Map</h4>
      <canvas
        ref={canvasRef}
        width={200}
        height={200}
        className="range-map-canvas"
      />
      {structureCount > 0 && (
        <div className="panel-stat-row">
          <span className="stat-label">Structures</span>
          <span className="stat-value">{structureCount}</span>
        </div>
      )}
    </div>
  );
}
