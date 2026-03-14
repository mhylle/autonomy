# ADR-0005: Browser Visualization Stack

**Status**: Accepted
**Date**: 2026-03-14
**Decision**: Use PixiJS v8 for 2D rendering in a React application, communicating with the engine via WebSocket

## Context

The viewer must:
- Render thousands (eventually 100k+) of moving entities at interactive frame rates
- Run in a standard web browser (no native app required)
- Support pan, zoom, entity selection, and interactive overlays
- Display UI panels (entity details, statistics, narrative, controls)
- Start with 2D, with a path to 3D in later phases

## Decision

- **Rendering**: PixiJS v8 for 2D WebGL rendering
- **UI framework**: React 19 with Zustand for state management
- **Communication**: WebSocket with protobuf binary messages
- **Build tool**: Vite

## Rationale

### PixiJS v8

Benchmarked at 100k+ sprites at 60fps on modern desktop hardware. Key advantages:
- WebGL-backed with WebGPU backend available (future-proof)
- v8's reactive renderer only updates changed sprites -- massive improvement for simulations where most entities are stationary
- Sprite batching (up to 16 textures per draw call)
- Well-documented performance optimization patterns

Benchmark data (10,000 sprites):
| Engine | FPS |
|--------|-----|
| PixiJS | 47 |
| Phaser | 43 |
| Canvas API | <30 |

### Three.js Migration Path

When 3D is needed (Phase 8+), Three.js with `InstancedMesh` handles 100k+ instances in a single draw call. PixiJS → Three.js migration is feasible because:
- Rendering logic is isolated in dedicated renderer classes
- Entity state store is renderer-agnostic
- Camera/zoom logic abstracts over the rendering backend

### React + Zustand

- React for UI panels, controls, and overlays
- Zustand for lightweight state management of world state from WebSocket deltas
- PixiJS canvas managed outside React's render cycle for performance (React renders UI, PixiJS renders the world)

### WebSocket over alternatives

| Transport | Verdict |
|-----------|---------|
| WebSocket | Full-duplex, universal browser support, binary frames. Correct choice. |
| WebTransport | Lower latency via HTTP/3, but no Safari/iOS support as of 2026. Not viable for broad compatibility. |
| Server-Sent Events | Unidirectional only. We need bidirectional (viewer sends commands). |

## Consequences

- Two rendering contexts in the browser: React DOM for UI, PixiJS WebGL canvas for world
- Entity interpolation between ticks is the viewer's responsibility (engine sends discrete tick states)
- Zoom level determines detail level: engine streams less data when viewer is zoomed out
- PixiJS dependency locks us to 2D initially; Three.js migration requires replacing renderer classes but not state management or UI
