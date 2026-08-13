# ADR-001: Modular Monolith with Strict Layer Boundaries

## Status

Accepted (2026-08-02)

## Context

PasteCanvas needs a desktop application architecture that:
- Keeps domain logic testable without framework dependencies
- Prevents frontend from accessing SQLite/filesystem/Win32 directly
- Allows future sync without redesigning the data model
- Supports Windows platform features behind abstractions

## Decision

Adopt a modular monolith with five Rust layers:

1. **Domain** (`src/domain/`): Entities, value objects, business rules. Zero dependencies on Tauri, sqlx, Win32, or WebView.
2. **Application** (`src/application/`): Use-case services, transaction boundaries, validation. Depends on domain + ports.
3. **Infrastructure** (`src/infrastructure/`): SQLite repositories, asset store, job queue. Implements ports.
4. **Platform** (`src/platform/`): Windows-specific adapters (clipboard, hotkeys, screenshot, tray). Behind `#[cfg(target_os = "windows")]`.
5. **Commands** (`src/commands/`): Thin Tauri command boundary. Maps DTOs, delegates to services, returns structured errors.

Frontend communicates exclusively through typed Tauri commands. React Flow types are isolated in `src/adapters/canvas/`.

## Consequences

- Domain layer is fully unit-testable without database or OS
- Adding a new platform (macOS/Linux) requires only new platform adapters
- Frontend cannot accidentally corrupt data or bypass validation
- Slightly more boilerplate for simple CRUD (command → service → repository)
- Sync can be added at the infrastructure layer without touching domain rules

## Alternatives Considered

- **Single-layer Tauri app**: Simpler but violates testability and security boundaries
- **Microservices**: Over-engineered for a single-user desktop app
- **Electron**: Rejected for bundle size, memory usage, and native API access quality
