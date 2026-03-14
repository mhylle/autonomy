import { useRef, useEffect, useCallback } from 'react';
import { speciesHistory, getActiveSpeciesIds } from '../net/history.ts';

const CHART_WIDTH = 280;
const CHART_HEIGHT = 140;
const PADDING = { top: 20, right: 10, bottom: 20, left: 40 };
const GRID_COLOR = 'rgba(255,255,255,0.08)';
const TEXT_COLOR = 'rgba(255,255,255,0.5)';

/** Maximum species to display (top N by recent count) */
const MAX_DISPLAYED_SPECIES = 12;

/**
 * Canvas-based stacked area chart showing species diversity over time.
 * Each species gets a unique color derived from its speciesId.
 * Redraws at ~4 Hz.
 */
export function SpeciesChart() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const timerRef = useRef<ReturnType<typeof setInterval>>(0 as unknown as ReturnType<typeof setInterval>);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const data = speciesHistory.getAll();
    const w = CHART_WIDTH;
    const h = CHART_HEIGHT;
    const plotW = w - PADDING.left - PADDING.right;
    const plotH = h - PADDING.top - PADDING.bottom;

    ctx.clearRect(0, 0, w, h);

    if (data.length < 2) {
      ctx.fillStyle = TEXT_COLOR;
      ctx.font = '11px monospace';
      ctx.fillText('Waiting for data...', PADDING.left, h / 2);
      return;
    }

    // Determine top species by most recent sample
    const allIds = getActiveSpeciesIds();
    const lastSample = data[data.length - 1].species;
    const sorted = allIds
      .map((id) => ({ id, count: lastSample.get(id) ?? 0 }))
      .sort((a, b) => b.count - a.count);
    const topSpecies = sorted.slice(0, MAX_DISPLAYED_SPECIES).map((s) => s.id);

    // Compute max stacked height
    let maxTotal = 0;
    for (const sample of data) {
      let total = 0;
      for (const sid of topSpecies) {
        total += sample.species.get(sid) ?? 0;
      }
      if (total > maxTotal) maxTotal = total;
    }
    if (maxTotal === 0) maxTotal = 1;

    const gridStep = niceStep(maxTotal, 4);
    const yMax = Math.ceil(maxTotal / gridStep) * gridStep;

    // Title
    ctx.fillStyle = TEXT_COLOR;
    ctx.font = '11px monospace';
    ctx.fillText('Species', PADDING.left, 13);

    // Species count label
    ctx.textAlign = 'right';
    ctx.fillText(`${allIds.length} spp`, PADDING.left + plotW, 13);
    ctx.textAlign = 'left';

    // Grid lines
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

      ctx.fillStyle = TEXT_COLOR;
      ctx.font = '9px monospace';
      ctx.textAlign = 'right';
      ctx.fillText(formatNum(val), PADDING.left - 4, y + 3);
    }
    ctx.textAlign = 'left';

    // Draw stacked areas (bottom-up)
    // Build cumulative arrays
    const n = data.length;
    const cumulative: number[][] = new Array(topSpecies.length + 1);
    cumulative[0] = new Array(n).fill(0);
    for (let s = 0; s < topSpecies.length; s++) {
      cumulative[s + 1] = new Array(n);
      const sid = topSpecies[s];
      for (let i = 0; i < n; i++) {
        cumulative[s + 1][i] = cumulative[s][i] + (data[i].species.get(sid) ?? 0);
      }
    }

    // Draw from top layer down so bottom layers overlap properly
    for (let s = topSpecies.length - 1; s >= 0; s--) {
      const color = speciesColor(topSpecies[s]);
      ctx.fillStyle = color;
      ctx.globalAlpha = 0.6;
      ctx.beginPath();

      // Top edge (cumulative[s+1])
      for (let i = 0; i < n; i++) {
        const x = PADDING.left + (i / (n - 1)) * plotW;
        const y = PADDING.top + plotH - (cumulative[s + 1][i] / yMax) * plotH;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }

      // Bottom edge (cumulative[s]) reversed
      for (let i = n - 1; i >= 0; i--) {
        const x = PADDING.left + (i / (n - 1)) * plotW;
        const y = PADDING.top + plotH - (cumulative[s][i] / yMax) * plotH;
        ctx.lineTo(x, y);
      }

      ctx.closePath();
      ctx.fill();
    }
    ctx.globalAlpha = 1;

    // Draw top-line for each species
    ctx.lineWidth = 1;
    for (let s = 0; s < topSpecies.length; s++) {
      ctx.strokeStyle = speciesColor(topSpecies[s]);
      ctx.globalAlpha = 0.9;
      ctx.beginPath();
      for (let i = 0; i < n; i++) {
        const x = PADDING.left + (i / (n - 1)) * plotW;
        const y = PADDING.top + plotH - (cumulative[s + 1][i] / yMax) * plotH;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
    }
    ctx.globalAlpha = 1;
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

/** Deterministic HSL color from speciesId (matches WorldRenderer logic) */
function speciesColor(speciesId: number): string {
  const hue = (speciesId * 137.508) % 360;
  return `hsl(${hue.toFixed(0)}, 70%, 60%)`;
}

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
