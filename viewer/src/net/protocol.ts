import * as protobuf from 'protobufjs';

// Types matching our proto definitions

export interface Vec2 {
  x: number;
  y: number;
}

export interface EntityState {
  id: number;
  position: Vec2;
  energy: number;
  maxEnergy: number;
  health: number;
  maxHealth: number;
  age: number;
  maxLifespan: number;
  size: number;
  speciesId: number;
  generation: number;
  tribeId: number;              // 0 = no tribe
  isCompositeLeader: boolean;
  compositeMemberCount: number;
}

export interface ResourceState {
  id: number;
  position: Vec2;
  resourceType: string;
  amount: number;
  maxAmount: number;
}

export interface TerrainGrid {
  cols: number;
  rows: number;
  cellSize: number;
  types: number[];
}

export interface SignalState {
  id: number;
  x: number;
  y: number;
  signalType: number;
  strength: number;
}

export interface TribeInfo {
  id: number;
  memberCount: number;
  centroidX: number;
  centroidY: number;
}

export interface ActiveWar {
  tribeAId: number;
  tribeBId: number;
  declaredTick: number;
}

export interface StructureState {
  id: number;
  x: number;
  y: number;
  structureType: string;
  health: number;
  maxHealth: number;
  progress: number;
}

export interface SettlementState {
  id: number;
  name: string;
  x: number;
  y: number;
  population: number;
  tribeId: number;
  foundingTick: number;
  defenseScore: number;
}

export interface TradeRouteState {
  fromSettlement: number;
  toSettlement: number;
  resourceType: string;
  volume: number;
  tripCount: number;
}

export interface CulturalProfile {
  tribeId: number;
  complexity: number;
  signalSummary: string;
}

export interface ObjectState {
  id: number;
  x: number;
  y: number;
  material: string;
  mass: number;
}

export interface WorldSnapshot {
  tick: number;
  entities: EntityState[];
  resources: ResourceState[];
  worldWidth: number;
  worldHeight: number;
  terrain: TerrainGrid | null;
  signals: SignalState[];
  tribes: TribeInfo[];
  activeWars: ActiveWar[];
  structures: StructureState[];
  settlements: SettlementState[];
  tradeRoutes: TradeRouteState[];
  culturalProfiles: CulturalProfile[];
  objectsInWorld: ObjectState[];
}

export interface TickDelta {
  tick: number;
  spawned: EntityState[];
  updated: EntityState[];
  died: number[];
  resourceChanges: ResourceState[];
  entityCount: number;
  warChanges: ActiveWar[];
  settlementChanges: SettlementState[];
}

let worldSnapshotType: protobuf.Type | null = null;
let tickDeltaType: protobuf.Type | null = null;

export async function loadProtoDefinitions(): Promise<void> {
  const root = await protobuf.load('/proto/world.proto');
  worldSnapshotType = root.lookupType('autonomy.WorldSnapshot');
  tickDeltaType = root.lookupType('autonomy.TickDelta');
}

export function decodeWorldSnapshot(data: Uint8Array): WorldSnapshot {
  if (!worldSnapshotType) throw new Error('Proto definitions not loaded');
  const msg = worldSnapshotType.decode(data);
  return worldSnapshotType.toObject(msg, {
    longs: Number,
    defaults: true,
  }) as unknown as WorldSnapshot;
}

export function decodeTickDelta(data: Uint8Array): TickDelta {
  if (!tickDeltaType) throw new Error('Proto definitions not loaded');
  const msg = tickDeltaType.decode(data);
  return tickDeltaType.toObject(msg, {
    longs: Number,
    defaults: true,
  }) as unknown as TickDelta;
}
