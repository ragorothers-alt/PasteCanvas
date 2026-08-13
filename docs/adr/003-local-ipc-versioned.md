# ADR-003: Versioned Local IPC — Activation Handoff and Native Messaging

## Status

Accepted (2026-08-02)

## Context

Phase 4 requires local IPC between PasteCanvas and its integration surfaces:

- Windows Explorer context menu (CAP-007) must capture files without blocking Explorer and without running a WebView inside Explorer.
- The browser extension (CAP-008) must deliver captures to the desktop app.
- The running app must learn about out-of-process captures (CAP-009) and be activated when not running.
- All channels must be current-user scoped (privacy-by-default), versioned, bounded, and testable.

## Decision

No named pipe server is introduced. Each channel uses the lightest mechanism that satisfies its constraints:

1. **Browser → Native Messaging Host (stdio, Chrome/Edge-managed)**
   - Length-prefixed JSON frames (`[u32 LE length][payload]`), hard cap **1 MB** (`MAX_MESSAGE_BYTES`).
   - **Protocol version** field (`protocolVersion`, currently 1); the host rejects higher versions explicitly and defaults missing fields to 1 for older extensions.
   - **Request ID** correlation (`requestId` echoed in every response).
   - **Strict schema validation**: serde typed structs + capture-kind whitelist (`page|selection|link|image|video`); unknown kinds rejected before any work.
   - **Extension-ID allowlist**: the NMH manifest's `allowed_origins` contains exactly the origin the user registers (Settings UI); unknown extensions are denied by the browser itself.

2. **Explorer → bridge binary (detached process, shared DB)**
   - `IExplorerCommand` COM DLL launches `pastecanvas-explorer.exe` detached; Explorer returns immediately.
   - The bridge receives file **paths** as CLI arguments (no binary payload over IPC), stages assets into the content-addressed store, and commits to the shared SQLite DB (WAL permits safe concurrent access).

3. **Integration → running app (single-instance activation handoff)**
   - After committing, the bridge/NMH launches the main executable with the marker argument `--pastecanvas-capture-notify`.
   - `tauri-plugin-single-instance` forwards arguments to the running instance (current-user socket) which emits `pastecanvas:item-captured`; the launcher exits immediately. When no instance runs, the launch activates the app (Phase-4 "app-not-running activation").
   - Notification is advisory: the capture is already durable in the DB before notification is attempted.

### Security posture

- **Current-user ACL everywhere**: HKCU registry entries, per-user APPDATA manifest/DB/assets, user-scoped single-instance socket. Nothing requires elevation.
- **Message caps**: 1 MB frame limit enforced on both read and write paths.
- **Replay/rate**: requests are idempotent captures; a replayed message yields at most an advisory duplicate notice (INB-004 dedup). No bearer tokens or privileged actions exist on these channels.
- **Cleanup**: bridge and NMH processes are short-lived per invocation; no persistent listeners to leak.
- **Binary handoff**: large payloads travel as files through the content-addressed asset store (temp → hash → atomic rename), never inline over IPC.

## Consequences

- No long-running privileged listener process; smaller attack surface than a named pipe server.
- Browser channel security inherits Chrome/Edge's Native Messaging enforcement (origin allowlist, stdio isolation).
- Capture paths share one durable commit pipeline; UI refresh is event-driven via the activation handoff.
- Future protocol changes bump `protocolVersion`; the host's explicit rejection makes version mismatch actionable (Phase-4 checklist).

## Alternatives Considered

- **Named pipe server**: persistent listener requiring explicit DACL management and lifetime handling; rejected — neither real channel needs bidirectional streaming.
- **Deep-link URI activation (`pastecanvas://`)**: URL-handler registration is a global surface and URL length/encoding limits complicate multi-file payloads; rejected for capture transport, acceptable later for simple "open board" links.
- **File-watch polling for cross-process refresh**: continuous I/O and latency; rejected in favor of explicit activation handoff with polling-free semantics.

## Test Harness

- `native_messaging.rs` framing + schema unit tests (round-trip, endianness, oversize rejection on read/write, version/kind validation, defaults).
- `explorer_capture_cap007.rs` integration tests (file capture pipeline, board placement).
- `notify.rs` unit tests (marker detection, graceful failure, self-launch guard).
- CAP-008 NMH integration tests (message processing, error responses).
