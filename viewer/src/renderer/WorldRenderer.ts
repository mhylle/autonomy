import { worldData } from '../net/state-store';
import { TerrainRenderer } from './TerrainRenderer';
import { MinimapRenderer } from './MinimapRenderer';

/**
 * Canvas 2D renderer for the simulation world.
 *
 * Reads entity/resource data directly from the shared worldData
 * object (bypassing React) to avoid GC pressure from Map copies.
 * Runs its own requestAnimationFrame loop independent of React.
 *
 * Will be replaced by Three.js in Era 10 (3D World) — the
 * interface (init/destroy/setOnEntityClick) stays the same.
 */
export class WorldRenderer {
  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;

  // Camera
  private cameraX = 0;
  private cameraY = 0;
  private zoom = 1;
  private cameraDirty = true;

  // Hit detection
  private entityHitTargets: { id: number; x: number; y: number; r: number }[] = [];
  private onEntityClick: ((id: number) => void) | null = null;

  private initialized = false;
  private animFrameId = 0;
  private renderCount = 0;

  // Sub-renderers
  private terrainRenderer = new TerrainRenderer();
  private minimapRenderer = new MinimapRenderer();

  // Color cache
  private colorCache: Map<number, string> = new Map();

  // Viewport change callback for sending updates to the server
  private onViewportChange: ((bounds: { x: number; y: number; width: number; height: number; zoom: number }) => void) | null = null;

  async init(canvas: HTMLCanvasElement): Promise<void> {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d')!;

    const parent = canvas.parentElement;
    const resize = () => {
      const w = parent?.clientWidth || window.innerWidth;
      const h = parent?.clientHeight || window.innerHeight;
      canvas.width = w;
      canvas.height = h;
    };
    resize();
    window.addEventListener('resize', resize);

    this.setupCamera(canvas);
    this.initialized = true;

    // Center on world
    this.cameraX = worldData.worldWidth / 2;
    this.cameraY = worldData.worldHeight / 2;

    // Expose diagnostics
    (window as any).__renderDiag = () => ({
      renderCount: this.renderCount,
      initialized: this.initialized,
      animFrameId: this.animFrameId,
      entityCount: worldData.entities.size,
      resourceCount: worldData.resources.size,
    });

    // Independent render loop — not tied to React
    const renderLoop = () => {
      this.render();
      this.animFrameId = requestAnimationFrame(renderLoop);
    };
    this.animFrameId = requestAnimationFrame(renderLoop);
  }

  /** Initialize the minimap with its canvas element. */
  initMinimap(canvas: HTMLCanvasElement): void {
    this.minimapRenderer.init(canvas);
  }

  setOnEntityClick(callback: (id: number) => void): void {
    this.onEntityClick = callback;
  }

  /** Register a callback to be notified when the viewport changes (pan/zoom). */
  setOnViewportChange(callback: (bounds: { x: number; y: number; width: number; height: number; zoom: number }) => void): void {
    this.onViewportChange = callback;
  }

  /** Get the current viewport bounds in world coordinates. */
  getViewportBounds(): { x: number; y: number; width: number; height: number; zoom: number } {
    const canvas = this.canvas;
    if (!canvas) {
      return { x: 0, y: 0, width: 10000, height: 10000, zoom: 1 };
    }
    const w = canvas.width / this.zoom;
    const h = canvas.height / this.zoom;
    return {
      x: this.cameraX - w / 2,
      y: this.cameraY - h / 2,
      width: w,
      height: h,
      zoom: this.zoom,
    };
  }

  /** Notify that the viewport changed. Called from camera event handlers. */
  private notifyViewportChange(): void {
    if (this.onViewportChange) {
      this.onViewportChange(this.getViewportBounds());
    }
  }

  private render(): void {
    const ctx = this.ctx;
    const canvas = this.canvas;
    if (!ctx || !canvas || !this.initialized) return;

    const w = canvas.width;
    const h = canvas.height;
    this.renderCount++;
    const { entities, resources, worldWidth, worldHeight, terrain } = worldData;

    // Update terrain texture if data changed
    this.terrainRenderer.update(terrain);

    // Update camera center if world size changed
    if (this.cameraDirty) {
      this.cameraX = worldWidth / 2;
      this.cameraY = worldHeight / 2;
      this.cameraDirty = false;
    }

    // Clear
    ctx.fillStyle = '#1a1a2e';
    ctx.fillRect(0, 0, w, h);

    // Camera transform
    ctx.save();
    ctx.translate(w / 2 - this.cameraX * this.zoom, h / 2 - this.cameraY * this.zoom);
    ctx.scale(this.zoom, this.zoom);

    // Draw terrain background
    this.terrainRenderer.draw(ctx);

    // World boundary
    ctx.strokeStyle = 'rgba(255,255,255,0.08)';
    ctx.lineWidth = 1 / this.zoom;
    ctx.strokeRect(0, 0, worldWidth, worldHeight);

    // Draw resources — batch by color to reduce state changes
    ctx.fillStyle = 'rgba(74, 222, 128, 0.5)';
    for (const resource of resources.values()) {
      if (resource.amount <= 0) continue;
      const ratio = resource.amount / resource.maxAmount;
      const radius = 2 + ratio * 3;
      ctx.globalAlpha = 0.3 + ratio * 0.5;
      ctx.beginPath();
      ctx.arc(resource.position.x, resource.position.y, radius, 0, Math.PI * 2);
      ctx.fill();
    }

    // Draw entities
    ctx.globalAlpha = 1;
    this.entityHitTargets.length = 0; // reuse array
    for (const [id, entity] of entities) {
      const color = this.getSpeciesColor(entity.speciesId);
      const radius = Math.max(2, entity.size * 0.5);
      const energyRatio = entity.energy / entity.maxEnergy;

      ctx.globalAlpha = 0.3 + energyRatio * 0.7;
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.arc(entity.position.x, entity.position.y, radius, 0, Math.PI * 2);
      ctx.fill();

      this.entityHitTargets.push({ id, x: entity.position.x, y: entity.position.y, r: radius });
    }

    // Draw kill indicators (Phase 3.7)
    this.drawKillIndicators(ctx);

    ctx.globalAlpha = 1;
    ctx.restore();

    // Draw minimap
    this.minimapRenderer.draw(worldWidth, worldHeight, entities, this.terrainRenderer);
  }

  /**
   * Draw fading red X marks at positions where entities recently died.
   * Kill events fade out over 1.5 seconds.
   */
  private drawKillIndicators(ctx: CanvasRenderingContext2D): void {
    const events = worldData.killEvents;
    if (events.length === 0) return;

    const now = performance.now();
    const fadeMs = 1500; // duration of the fade effect
    const xSize = 6 / this.zoom; // X mark arm length, constant screen size

    ctx.lineCap = 'round';

    for (const event of events) {
      const age = now - event.timestamp;
      if (age > fadeMs) continue;

      const alpha = 1 - age / fadeMs;
      // Scale slightly larger at start, shrinking as it fades
      const scale = 1 + (1 - alpha) * 0.5;
      const armLen = xSize * scale;

      ctx.strokeStyle = `rgba(239, 68, 68, ${alpha})`;
      ctx.lineWidth = Math.max(2 / this.zoom, 1);

      // Draw X
      ctx.beginPath();
      ctx.moveTo(event.x - armLen, event.y - armLen);
      ctx.lineTo(event.x + armLen, event.y + armLen);
      ctx.moveTo(event.x + armLen, event.y - armLen);
      ctx.lineTo(event.x - armLen, event.y + armLen);
      ctx.stroke();
    }
  }

  private getSpeciesColor(speciesId: number): string {
    let color = this.colorCache.get(speciesId);
    if (!color) {
      const hue = (speciesId * 137.508) % 360;
      color = hslToHex(hue, 70, 60);
      this.colorCache.set(speciesId, color);
    }
    return color;
  }

  private setupCamera(canvas: HTMLCanvasElement): void {
    let isDragging = false;
    let lastX = 0;
    let lastY = 0;

    canvas.addEventListener('mousedown', (e) => {
      isDragging = true;
      lastX = e.clientX;
      lastY = e.clientY;
    });

    canvas.addEventListener('mousemove', (e) => {
      if (!isDragging) return;
      const dx = e.clientX - lastX;
      const dy = e.clientY - lastY;
      this.cameraX -= dx / this.zoom;
      this.cameraY -= dy / this.zoom;
      lastX = e.clientX;
      lastY = e.clientY;
      this.notifyViewportChange();
    });

    canvas.addEventListener('mouseup', () => {
      if (isDragging) {
        isDragging = false;
        this.notifyViewportChange();
      }
    });
    canvas.addEventListener('mouseleave', () => { isDragging = false; });

    canvas.addEventListener('wheel', (e) => {
      e.preventDefault();
      const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
      this.zoom = Math.max(0.1, Math.min(10, this.zoom * zoomFactor));
      this.notifyViewportChange();
    }, { passive: false });

    canvas.addEventListener('click', (e) => {
      if (!this.onEntityClick) return;
      const rect = canvas.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;
      const worldX = (screenX - canvas.width / 2) / this.zoom + this.cameraX;
      const worldY = (screenY - canvas.height / 2) / this.zoom + this.cameraY;
      const hit = this.findEntityAt(worldX, worldY);
      if (hit !== null) this.onEntityClick(hit);
    });
  }

  private findEntityAt(x: number, y: number): number | null {
    let closest: number | null = null;
    let closestDist = Infinity;
    for (const t of this.entityHitTargets) {
      const dx = t.x - x;
      const dy = t.y - y;
      const dist = dx * dx + dy * dy;
      const hitRadius = Math.max(t.r, 8 / this.zoom);
      if (dist < hitRadius * hitRadius && dist < closestDist) {
        closestDist = dist;
        closest = t.id;
      }
    }
    return closest;
  }

  destroy(): void {
    this.initialized = false;
    if (this.animFrameId) {
      cancelAnimationFrame(this.animFrameId);
    }
    this.terrainRenderer.destroy();
    this.minimapRenderer.destroy();
  }
}

function hslToHex(h: number, s: number, l: number): string {
  s /= 100;
  l /= 100;
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  const r = Math.round(f(0) * 255);
  const g = Math.round(f(8) * 255);
  const b = Math.round(f(4) * 255);
  return `rgb(${r},${g},${b})`;
}
