import type { TerrainGrid } from '../net/protocol';
import { TERRAIN_COLORS, DEFAULT_TERRAIN_COLOR } from './terrain-colors';

/**
 * Renders terrain as an off-screen canvas texture.
 *
 * The texture is generated once when terrain data arrives and drawn
 * as a single drawImage call each frame. This avoids per-cell fillRect
 * calls in the hot render loop.
 */
export class TerrainRenderer {
  private texture: HTMLCanvasElement | null = null;
  private currentTerrain: TerrainGrid | null = null;

  /**
   * Generate the terrain texture if the terrain data has changed.
   * Returns true if the texture was regenerated.
   */
  update(terrain: TerrainGrid | null): boolean {
    if (!terrain) return false;
    if (terrain === this.currentTerrain) return false;

    this.currentTerrain = terrain;
    this.texture = this.buildTexture(terrain);
    return true;
  }

  /**
   * Draw the terrain texture onto the main canvas context.
   *
   * The caller is responsible for setting up the camera transform
   * before calling this method.
   */
  draw(ctx: CanvasRenderingContext2D): void {
    if (!this.texture) return;
    ctx.drawImage(this.texture, 0, 0);
  }

  /** Returns the off-screen texture canvas (for minimap use). */
  getTexture(): HTMLCanvasElement | null {
    return this.texture;
  }

  private buildTexture(terrain: TerrainGrid): HTMLCanvasElement {
    const canvas = document.createElement('canvas');
    const pixelWidth = terrain.cols * terrain.cellSize;
    const pixelHeight = terrain.rows * terrain.cellSize;
    canvas.width = pixelWidth;
    canvas.height = pixelHeight;

    const ctx = canvas.getContext('2d')!;
    const { cols, rows, cellSize, types } = terrain;

    for (let row = 0; row < rows; row++) {
      for (let col = 0; col < cols; col++) {
        const terrainType = types[row * cols + col];
        ctx.fillStyle = TERRAIN_COLORS[terrainType] ?? DEFAULT_TERRAIN_COLOR;
        ctx.fillRect(col * cellSize, row * cellSize, cellSize, cellSize);
      }
    }

    return canvas;
  }

  destroy(): void {
    this.texture = null;
    this.currentTerrain = null;
  }
}
