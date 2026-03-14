import { create } from 'zustand';
import type { EntityState, ResourceState, TerrainGrid, WorldSnapshot, TickDelta } from './protocol';
import { recordHistorySample } from './history';

/**
 * A kill event records where an entity died, used for rendering
 * transient death indicators on the canvas.
 */
export interface KillEvent {
  x: number;
  y: number;
  timestamp: number;
}

/**
 * World data lives OUTSIDE React/Zustand to avoid GC pressure.
 *
 * The entities and resources Maps are mutated in place — no copies.
 * The renderer reads directly from these. Zustand only stores
 * lightweight HUD values (tick, count, fps) that update the DOM.
 */
export const worldData = {
  entities: new Map<number, EntityState>(),
  resources: new Map<number, ResourceState>(),
  worldWidth: 500,
  worldHeight: 500,
  terrain: null as TerrainGrid | null,
  /** Recent kill events for rendering death indicators. */
  killEvents: [] as KillEvent[],
};

export function applySnapshotToWorld(snapshot: WorldSnapshot): void {
  worldData.entities.clear();
  for (const entity of snapshot.entities) {
    worldData.entities.set(entity.id, entity);
  }
  worldData.resources.clear();
  for (const resource of snapshot.resources) {
    worldData.resources.set(resource.id, resource);
  }
  worldData.worldWidth = snapshot.worldWidth;
  worldData.worldHeight = snapshot.worldHeight;
  worldData.terrain = snapshot.terrain;

  // Record history sample on snapshot
  recordHistorySample(snapshot.tick, worldData.entities);
}

export function applyDeltaToWorld(delta: TickDelta): void {
  // Apply incremental changes: spawned, updated, died.
  for (const entity of delta.spawned) {
    worldData.entities.set(entity.id, entity);
  }
  for (const entity of delta.updated) {
    worldData.entities.set(entity.id, entity);
  }

  // Record kill events before removing entities so we can capture
  // their last known position for death indicators.
  const now = performance.now();
  for (const id of delta.died) {
    const entity = worldData.entities.get(id);
    if (entity) {
      worldData.killEvents.push({
        x: entity.position.x,
        y: entity.position.y,
        timestamp: now,
      });
    }
    worldData.entities.delete(id);
  }

  // Prune old kill events (older than 2 seconds)
  const cutoff = now - 2000;
  if (worldData.killEvents.length > 0 && worldData.killEvents[0].timestamp < cutoff) {
    const firstValid = worldData.killEvents.findIndex((e) => e.timestamp >= cutoff);
    if (firstValid === -1) {
      worldData.killEvents.length = 0;
    } else {
      worldData.killEvents.splice(0, firstValid);
    }
  }

  for (const resource of delta.resourceChanges) {
    worldData.resources.set(resource.id, resource);
  }

  // Record history sample after delta applied
  recordHistorySample(delta.tick, worldData.entities);
}

/** Lightweight Zustand store — only HUD values, no entity data. */
export interface HudState {
  tick: number;
  entityCount: number;
  connected: boolean;
  fps: number;
  selectedEntityId: number | null;
  paused: boolean;
  speedMultiplier: number;

  setHud: (tick: number, entityCount: number) => void;
  setConnected: (connected: boolean) => void;
  setFps: (fps: number) => void;
  selectEntity: (id: number | null) => void;
  togglePause: () => void;
  setSpeedMultiplier: (speed: number) => void;
}

export const useHudStore = create<HudState>((set) => ({
  tick: 0,
  entityCount: 0,
  connected: false,
  fps: 0,
  selectedEntityId: null,
  paused: false,
  speedMultiplier: 1,

  setHud: (tick, entityCount) => set({ tick, entityCount }),
  setConnected: (connected) => set({ connected }),
  setFps: (fps) => set({ fps }),
  selectEntity: (id) => set({ selectedEntityId: id }),
  togglePause: () => set((state) => ({ paused: !state.paused })),
  setSpeedMultiplier: (speed) => set({ speedMultiplier: speed }),
}));
