import { useRef, useEffect, useCallback } from 'react';
import { populationHistory } from '../net/history.ts';

const CHART_WIDTH = 280;
const CHART_HEIGHT = 120;
const PADDING = { top: 20, right: 10, bottom: 20, left: 40 };
const LINE_COLOR = '#60a5fa';
const GRID_COLOR = 'rgba(255,255,255,0.08)';
const TEXT_COLOR = 'rgba(255,255,255,0.5)';
const BG_COLOR = 'rgba(0,0,0,0)';

/**
 * Canvas-based line chart showing total population over the last N ticks.
 * Redraws at ~4 Hz via setInterval to avoid per-frame overhead.
 */
export function PopulationChart() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const timerRef = useRef<ReturnType<typeof setInterval>>(0 as unknown as ReturnType<typeof setInterval>);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const data = populationHistory.getAll();
    const w = CHART_WIDTH;
    const h = CHART_HEIGHT;
    const plotW = w - PADDING.left - PADDING.right;
    const plotH = h - PADDING.top - PADDING.bottom;

    // Clear
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = BG_COLOR;
    ctx.fillRect(0, 0, w, h);

    if (data.length < 2) {
      ctx.fillStyle = TEXT_COLOR;
      ctx.font = '11px monospace';
      ctx.fillText('Waiting for data...', PADDING.left, h / 2);
      return;
    }

    // Compute range
    let maxVal = 0;
    for (const sample of data) {
      if (sample.totalCount > maxVal) maxVal = sample.totalCount;
    }
    if (maxVal === 0) maxVal = 1;

    // Round up max for nice grid lines
    const gridStep = niceStep(maxVal, 4);
    const yMax = Math.ceil(maxVal / gridStep) * gridStep;

    // Title
    ctx.fillStyle = TEXT_COLOR;
    ctx.font = '11px monospace';
    ctx.fillText('Population', PADDING.left, 13);

    // Draw grid lines
    ctx.strokeStyle = GRID_COLOR;
    ctx.lineWidth = 1;
    const gridCount = Math.round(yMax / gridStep);
    for (let i = 0; i <= gridCount; i++) {
      const val = i * gridStep;
      const y = PADDING.top + plotH - (val / yMax) * plotH;
      ctx.beginPath();
      ctx.moveTo(PADDING.left, y);
      ctx.lineTo(PADDING.left + plotW, y);
      ctx.stroke();

      // Label
      ctx.fillStyle = TEXT_COLOR;
      ctx.font = '9px monospace';
      ctx.textAlign = 'right';
      ctx.fillText(formatNum(val), PADDING.left - 4, y + 3);
    }

    // Draw line
    ctx.strokeStyle = LINE_COLOR;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    for (let i = 0; i < data.length; i++) {
      const x = PADDING.left + (i / (data.length - 1)) * plotW;
      const y = PADDING.top + plotH - (data[i].totalCount / yMax) * plotH;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();

    // Current value label
    const last = data[data.length - 1];
    ctx.fillStyle = LINE_COLOR;
    ctx.font = '10px monospace';
    ctx.textAlign = 'right';
    ctx.fillText(String(last.totalCount), PADDING.left + plotW, 13);
    ctx.textAlign = 'left';
  }, []);

  useEffect(() => {
    draw();
    timerRef.current = setInterval(draw, 250);
    return () => clearInterval(timerRef.current);
  }, [draw]);

  return (
    <canvas
      ref={canvasRef}
      width={CHART_WIDTH}
      height={CHART_HEIGHT}
      className="chart-canvas"
    />
  );
}

/** Pick a "nice" step size for grid lines */
function niceStep(max: number, targetLines: number): number {
  const rough = max / targetLines;
  const mag = Math.pow(10, Math.floor(Math.log10(rough)));
  const norm = rough / mag;
  if (norm <= 1) return mag;
  if (norm <= 2) return 2 * mag;
  if (norm <= 5) return 5 * mag;
  return 10 * mag;
}

function formatNum(n: number): string {
  if (n >= 1000) return (n / 1000).toFixed(1) + 'k';
  return String(Math.round(n));
}
