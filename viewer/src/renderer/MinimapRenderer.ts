import type { EntityState } from '../net/protocol';
import type { TerrainRenderer } from './TerrainRenderer';

/**
 * Renders a small minimap showing the entire world with entity dots.
 *
 * Uses a plain HTML canvas (not PixiJS) for simplicity.
 * The minimap is 200x200 and shows terrain background plus entity dots.
 */
export class MinimapRenderer {
  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private size = 200;

  init(canvas: HTMLCanvasElement): void {
    this.canvas = canvas;
    canvas.width = this.size;
    canvas.height = this.size;
    this.ctx = canvas.getContext('2d')!;
  }

  /**
   * Redraw the minimap with current world state.
   *
   * Called from the main render loop. Draws terrain background
   * scaled to fit, then entity dots on top.
   */
  draw(
    worldWidth: number,
    worldHeight: number,
    entities: Map<number, EntityState>,
    terrainRenderer: TerrainRenderer,
  ): void {
    const ctx = this.ctx;
    if (!ctx) return;

    const s = this.size;
    const scaleX = s / worldWidth;
    const scaleY = s / worldHeight;

    // Clear
    ctx.fillStyle = '#1a1a2e';
    ctx.fillRect(0, 0, s, s);

    // Draw terrain texture scaled to minimap
    const terrainTexture = terrainRenderer.getTexture();
    if (terrainTexture) {
      ctx.drawImage(terrainTexture, 0, 0, s, s);
    }

    // Draw entities as small dots
    ctx.fillStyle = '#ffffff';
    for (const entity of entities.values()) {
      const mx = entity.position.x * scaleX;
      const my = entity.position.y * scaleY;
      ctx.fillRect(mx - 1, my - 1, 2, 2);
    }

    // Border
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
    ctx.lineWidth = 1;
    ctx.strokeRect(0, 0, s, s);
  }

  destroy(): void {
    this.canvas = null;
    this.ctx = null;
  }
}
