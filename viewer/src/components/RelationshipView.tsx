import { useRef, useEffect } from 'react';
import type { EntityState } from '../net/protocol.ts';
import { worldData } from '../net/state-store.ts';

interface RelationshipViewProps {
  entity: EntityState;
}

/** Maximum radius (in world units) to consider an entity a neighbor. */
const NEIGHBOR_RADIUS = 80;
/** Maximum neighbors to display. */
const MAX_NEIGHBORS = 12;
/** Canvas dimensions. */
const CANVAS_SIZE = 140;

/**
 * Relationship graph visualization (Phase 3.5).
 *
 * Draws a small dot diagram with the selected entity at center
 * and its perceived neighbors around it. Kin (same species) are
 * highlighted with a ring; other species use their species color.
 *
 * Since explicit relationship data isn't in the protocol yet,
 * neighbors are determined by proximity in world space.
 */
export function RelationshipView({ entity }: RelationshipViewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    drawRelationships(ctx, entity);
  });

  return (
    <div className="entity-stats-group">
      <h4>Neighbors</h4>
      <canvas
        ref={canvasRef}
        className="relationship-canvas"
        width={CANVAS_SIZE}
        height={CANVAS_SIZE}
      />
    </div>
  );
}

interface Neighbor {
  id: number;
  distance: number;
  angle: number;
  isKin: boolean;
  speciesId: number;
}

function drawRelationships(ctx: CanvasRenderingContext2D, entity: EntityState): void {
  const w = CANVAS_SIZE;
  const h = CANVAS_SIZE;
  const cx = w / 2;
  const cy = h / 2;

  // Clear
  ctx.clearRect(0, 0, w, h);

  // Background circle
  ctx.fillStyle = 'rgba(255, 255, 255, 0.03)';
  ctx.beginPath();
  ctx.arc(cx, cy, 60, 0, Math.PI * 2);
  ctx.fill();

  // Gather neighbors
  const neighbors = findNeighbors(entity);

  // Draw connection lines
  for (const n of neighbors) {
    const normDist = Math.min(n.distance / NEIGHBOR_RADIUS, 1);
    const px = cx + Math.cos(n.angle) * normDist * 55;
    const py = cy + Math.sin(n.angle) * normDist * 55;

    ctx.strokeStyle = n.isKin
      ? 'rgba(96, 165, 250, 0.3)'
      : 'rgba(255, 255, 255, 0.1)';
    ctx.lineWidth = n.isKin ? 1.5 : 0.5;
    ctx.beginPath();
    ctx.moveTo(cx, cy);
    ctx.lineTo(px, py);
    ctx.stroke();
  }

  // Draw neighbor dots
  for (const n of neighbors) {
    const normDist = Math.min(n.distance / NEIGHBOR_RADIUS, 1);
    const px = cx + Math.cos(n.angle) * normDist * 55;
    const py = cy + Math.sin(n.angle) * normDist * 55;

    const hue = (n.speciesId * 137.508) % 360;
    const dotRadius = n.isKin ? 4 : 3;

    // Dot
    ctx.fillStyle = `hsl(${hue}, 70%, 60%)`;
    ctx.beginPath();
    ctx.arc(px, py, dotRadius, 0, Math.PI * 2);
    ctx.fill();

    // Kin ring
    if (n.isKin) {
      ctx.strokeStyle = `hsl(${hue}, 70%, 75%)`;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(px, py, dotRadius + 2, 0, Math.PI * 2);
      ctx.stroke();
    }
  }

  // Draw center entity (self)
  const selfHue = (entity.speciesId * 137.508) % 360;
  ctx.fillStyle = `hsl(${selfHue}, 70%, 70%)`;
  ctx.beginPath();
  ctx.arc(cx, cy, 5, 0, Math.PI * 2);
  ctx.fill();

  ctx.strokeStyle = '#fff';
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.arc(cx, cy, 7, 0, Math.PI * 2);
  ctx.stroke();

  // Label
  ctx.fillStyle = 'rgba(255, 255, 255, 0.4)';
  ctx.font = '10px monospace';
  ctx.textAlign = 'center';
  ctx.fillText(
    `${neighbors.length} nearby (${neighbors.filter((n) => n.isKin).length} kin)`,
    cx,
    h - 6,
  );
}

function findNeighbors(entity: EntityState): Neighbor[] {
  const neighbors: Neighbor[] = [];
  const ex = entity.position.x;
  const ey = entity.position.y;
  const r2 = NEIGHBOR_RADIUS * NEIGHBOR_RADIUS;

  for (const [id, other] of worldData.entities) {
    if (id === entity.id) continue;
    const dx = other.position.x - ex;
    const dy = other.position.y - ey;
    const dist2 = dx * dx + dy * dy;
    if (dist2 > r2) continue;

    neighbors.push({
      id,
      distance: Math.sqrt(dist2),
      angle: Math.atan2(dy, dx),
      isKin: other.speciesId === entity.speciesId,
      speciesId: other.speciesId,
    });
  }

  // Sort by distance, take closest
  neighbors.sort((a, b) => a.distance - b.distance);
  return neighbors.slice(0, MAX_NEIGHBORS);
}
