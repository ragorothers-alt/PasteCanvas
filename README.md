# PasteCanvas

Windows-first, local-first personal capture and infinite canvas tool.

## Principles

- **Capture first, organize later** — zero-friction collection
- **Local-first** — no account, no server, offline-capable
- **Privacy by default** — manual capture, exclusion lists, no telemetry
- **No AI in V1** — predictable rule-based organization only

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop shell | Tauri 2 |
| Frontend | React 19 + TypeScript strict + Vite |
| Native/Application | Rust |
| Database | SQLite (via sqlx, WAL mode) |
| Canvas | @xyflow/react (behind adapter) |
| State | Zustand (UI) + TanStack Query (async cache) |
| Tests | cargo test, Vitest, React Testing Library |
| Package manager | pnpm |

## Prerequisites

- Node.js >= 22
- pnpm >= 10
- Rust stable (>= 1.85) with `x86_64-pc-windows-msvc` target
- Windows 11 with WebView2

## Development

```bash
# Install dependencies
pnpm install

# Start desktop app in dev mode
pnpm desktop:dev

# Quality checks
pnpm lint
pnpm typecheck
pnpm test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
```

## Repository Layout

```
pastecanvas/
├─ apps/desktop/          # Tauri desktop application
│  ├─ src/                # React frontend
│  └─ src-tauri/          # Rust backend
│     ├─ src/
│     │  ├─ domain/       # Entities, value objects, rules (no framework deps)
│     │  ├─ application/  # Use cases, transactions, services
│     │  ├─ infrastructure/ # SQLite repos, asset store, jobs
│     │  ├─ platform/     # Windows adapters (clipboard, hotkeys, tray)
│     │  ├─ commands/     # Tauri command boundary (thin)
│     │  ├─ events/       # In-process event bus
│     │  ├─ jobs/         # Background job coordination
│     │  └─ bootstrap/    # App initialization
│     ├─ migrations/      # Immutable sequential SQL migrations
│     └─ capabilities/    # Tauri security capabilities
├─ crates/
│  ├─ pastecanvas-contracts/      # Shared DTO types
│  ├─ pastecanvas-test-support/   # Test utilities
│  ├─ pastecanvas-nmh/            # Browser extension Native Messaging host (CAP-008)
│  ├─ pastecanvas-explorer/       # Explorer context-menu capture bridge (CAP-007)
│  └─ pastecanvas-explorer-dll/   # IExplorerCommand COM server (CAP-007)
├─ apps/browser-extension/        # Chrome/Edge MV3 extension (CAP-008)
├─ packages/
│  └─ contracts-ts/      # TypeScript Zod schemas
├─ docs/adr/             # Architecture Decision Records
├─ fixtures/             # Test fixtures (no real user data)
└─ scripts/              # Build/utility scripts
```

## Dependencies & Licenses (FND-001)

All runtime dependencies are permissively licensed; no prerelease crates are
used without an ADR. Lockfiles (`pnpm-lock.yaml`, `Cargo.lock`) are committed.

| Dependency | Purpose | License |
|---|---|---|
| tauri 2 (+ official plugins: global-shortcut, clipboard-manager, single-instance) | Desktop shell, hotkeys, clipboard, tray | MIT / Apache-2.0 |
| sqlx (SQLite, runtime-tokio) | Database access, WAL, migrations | MIT / Apache-2.0 |
| tokio | Async runtime | MIT |
| serde / serde_json | DTO (de)serialization | MIT / Apache-2.0 |
| thiserror | Typed error enums | MIT / Apache-2.0 |
| sha2 | Content-addressed asset hashing | MIT / Apache-2.0 |
| reqwest | SSRF-guarded metadata fetch | MIT / Apache-2.0 |
| url | URL parsing/normalization | MIT / Apache-2.0 |
| base64, chrono, tracing/tracing-subscriber | Encoding, timestamps, structured logs | MIT / Apache-2.0 |
| uuid (v7) | Entity IDs | MIT / Apache-2.0 |
| react / react-dom 19 | UI | MIT |
| @xyflow/react | Infinite canvas (behind adapter) | MIT |
| zustand | UI state | MIT |
| @tiptap/* , @codemirror/* , pdfjs-dist | Rich-text / code / PDF rendering | MIT |
| zod | Command contract schemas | MIT |
| vitest / @testing-library/react / playwright | Tests | MIT / Apache-2.0 |

Windows API access (screenshots, shell, focus) uses raw FFI against the OS —
no extra crates. License of the produced application binaries: MIT (this
project). Third-party notices ship with their respective packages.

## Architecture Rules

- Domain layer has zero dependency on Tauri, Win32, SQLite, or WebView
- Frontend never executes SQL or accesses filesystem directly
- All Windows-specific code lives behind port abstractions in `platform/`
- React Flow types never appear in persistence or Rust DTOs
- Every command returns structured errors with stable codes

## License

MIT
