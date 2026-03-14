# ADR-0002: Wire Protocol

**Status**: Accepted
**Date**: 2026-03-14
**Decision**: Use Protocol Buffers (protobuf) via `prost` for the client-server wire protocol

## Context

The simulation engine (Rust) streams entity state to browser viewers (TypeScript). The protocol must:
- Be language-neutral (Rust server, TypeScript client)
- Support binary encoding (JSON is too slow for thousands of entities per tick)
- Have a schema that serves as the contract between engine and viewer
- Support backward-compatible schema evolution as features are added

## Decision

Use Protocol Buffers with `prost` (Rust) and `ts-proto` (TypeScript). Schema files live in `shared/proto/`.

Use `serde` + `bincode` for internal snapshots (save/load simulation state) where cross-language support is not needed.

## Rationale

- **Language-neutral schema**: `.proto` files are the single source of truth. Both Rust and TypeScript types are generated from the same schema.
- **Battle-tested**: Protobuf is the industry standard for binary serialization. `prost` is mature (v0.14).
- **Schema evolution**: Adding fields is backward-compatible. Critical as we add features across 8+ phases.
- **Tooling**: `prost-build` generates Rust types at compile time. `ts-proto` generates TypeScript types with full type safety.

## Alternatives Considered

| Format | Rejected Because |
|--------|-----------------|
| FlatBuffers | Zero-copy reads are attractive for high-frequency streaming, but Rust support is experimental ("APIs may change between minor versions" per their docs). Schema evolution story is weaker. |
| MessagePack | Smallest wire size, but no schema -- both sides must agree on structure implicitly. |
| JSON | Too slow and verbose for streaming thousands of entity states per tick. |
| Cap'n Proto | Good performance but smaller ecosystem and more complex setup than protobuf. |

## Consequences

- `shared/proto/` directory contains all `.proto` files
- Build scripts generate Rust types (`prost-build`) and TypeScript types (`ts-proto`)
- Adding a new message type requires updating the `.proto` schema and regenerating
- Internal snapshots use `bincode` (faster, no schema overhead, Rust-only)
