# Status

## Current phase

All phases implemented — full pipeline verified end-to-end

## Last verified state

All quality gates pass:
- `cargo fmt --check`: clean
- `cargo clippy -D warnings`: 0 warnings
- `cargo test --workspace`: 295 passed (29 test suites incl. BRD-010 node resize/lock/collapse/z-order)
- `pnpm typecheck`: 0 errors
- `pnpm test`: 207 React tests passed (24 suites) + 15 contracts-ts passed
- `playwright test`: 20/20 E2E passed (against Vite dev server; UI shell verified via screenshot)
- **Total: 517 tests + 20 E2E passed | 102 commands | nmh + explorer bridge + explorer COM DLL + browser extension**

## End-to-end pipeline verified ✓

```
✓ capture text → FTS5 search → find by keyword
✓ capture URL → search by domain
✓ capture color → type classification → ColorCard
✓ capture multiple → filter by type/source app
✓ soft delete → excluded from search results
✓ capture session → clipboard listener activation/deactivation
```

## Architecture audit ✓

```
✓ Domain layer: zero tauri/sqlx/win32 imports
✓ Frontend: no direct SQL/filesystem/Win32 access
✓ ARC-001: platform→commands inversion removed (window/region-overlay logic
  moved to platform::window_manager; commands are now thin wrappers); all five
  layer boundaries now enforced by machine-checked fitness tests
✓ Tauri capabilities: no allow-all, minimal permissions
✓ Adapters isolated: canvas/ rich-text/ code/ pdf/ + HtmlToTiptap converter
✓ Domain utilities: color.rs font.rs svg_sanitize.rs
✓ Privacy: monitoring OFF by default, 13 exclusion templates, no content in logs
✓ Security: SSRF protection (literal + DNS-resolved private-address rejection,
  hop-by-hop redirect validation, IPv6 bracket fix), asset path allow-list
  (SEC-002: read/reveal/delete refuse traversal), FTS injection prevention,
  SVG sanitization, HTML sanitization, strict CSP ('self' only), minimal capabilities
✓ Accessibility: session state conveyed by text (not color only), keyboard-first
  flows, prefers-reduced-motion honored (animations/transitions disabled)
✓ Durability: success only after DB commit, atomic asset writes; asset store
  self-heals directory layout on fresh data dirs (first-run capture hardened)
✓ Performance (PERF-001 measured on production query paths):
  20k FTS unicode MATCH p95 26.7ms, trigram MATCH p95 0.3ms (target < 150ms);
  5k FTS MATCH p95 0.3ms; capture 0.9ms; inbox query 0.4ms
✓ Session ↔ Listener: start/pause/resume/finish wired to clipboard state
```

## Feature coverage

### Phase 0 — Foundation ✓
- pnpm + Cargo monorepo, Tauri 2 + React 19 + TS strict + Vite
- 5-layer Rust architecture (domain/application/infrastructure/platform/commands)
- SQLite migration runner (WAL, FK, busy_timeout, checksum integrity)
- System tray + single-instance + close-to-tray
- Global hotkey (Ctrl+Shift+V) with conflict handling
- Typed command contract (CommandErrorDto + Zod schemas)
- Clipboard notification listener (sequence dedup, thread-safe state)

### Phase 1 — Capture & Inbox ✓
- CAP-001 compliance: hotkey captures clipboard TEXT first, then falls back to
  clipboard IMAGE (RGBA→lossless PNG via shared encoder, application-layer
  capture_png with asset staging + dedup advisory); read-image capability added
- Text/URL/Image/Screenshot/Color capture pipeline
- Capture Session (active→paused→finished) with clipboard listener wiring
- CAP-003 Session idle auto-end (configurable idle_timeout, job_runner tick)
- CAP-003/004/005 automatic monitoring pipeline (deep-audit fix): event-driven
  clipboard watcher (message-only window + WM_CLIPBOARDUPDATE — no polling,
  per GOAL.md); privacy gates applied BEFORE content is read: sequence dedup →
  Windows ExcludeClipboardFormat monitor marker → process exclusion list;
  foreground process name now really captured (Win32 FFI, RAII handle close);
  session-linked capture stores capture_session_id (Inbox session grouping)
- URL metadata enrichment (background jobs, SSRF-safe fetch)
- Inbox Smart Views (Today/Yesterday/Last-7-days/Earlier + Source/Domain/Type/Session grouping)
- Item Detail Panel (title edit, CodeMirror, ColorCard, PdfCard, FontCard)
- Batch operations (archive, trash, restore)
- Tag management (CRUD, item-tag association, search integration, TagSelector UI)
- Add to Board, favorite, pin, trash, pagination, reveal-asset-in-explorer (INB-003)
- INB-003 drag-to-board gesture (PRD "拖到 Board"): Inbox items are draggable
  (HTML5 DnD, shared payload contract dragPayload.ts); dropping on a board
  canvas places the item at the pointer's flow coordinates (cross-window in
  Tauri, keyboard/menu paths still available)
- INB-003 PRD triage rule (compliance fix): FIRST placement onto any Board
  transitions a 'new' item to 'triaged' on every placement path (canvas +
  explorer); guarded so user-changed statuses are never clobbered
- Backup (VACUUM INTO) + Export (JSON+Markdown)
- DAT-002 Restore (validate schema/files, pending-restore applied at startup)
- DAT-003 Trash view (restore, empty with impact-stats confirmation, orphan-asset purge)
- Settings + Capture Exclusions (13 built-in password managers)
- CAP-001 compliance (deep-audit fix): hotkey & tray capture outcomes are no
  longer silent — success shows a light toast, failure shows an understandable
  reason + retry hint (captureEvents module + Tauri→window event bridge)
- CAP-002 Quick Capture window (text + file drag-in via OS paths + optional
  target Board + Enter/Esc; floating always-on-top window); PRD focus rule now
  enforced: floating window records the previous foreground app on open and
  restores it after save/Esc (Win32 AttachThreadInput+SetForegroundWindow,
  behind platform::focus — never force-focuses ourselves, safe no-ops)
- Asset store (content-addressed, SHA-256 dedup, atomic write)
- P1-CAP-007 Bounded thumbnails: server-side PNG downscale (png crate only,
  box filter, RGBA normalize), cached under variants/<sha>_thumb_<max>.png
  (SEC-002-checked variant paths), served via get_item_thumbnail; ItemDetail
  shows bounded image/screenshot previews (never the full-size asset)
- INB-004 Dedup notice (hash + normalized URL, toast with "open existing")

### Phase 2 — Canvas ✓
- Board CRUD + ReactFlow infinite canvas
- CanvasMapper (DTO → React Flow, isolated from persistence)
- ItemCardNode (memo'd custom node)
- Group/Ungroup with coordinate conversion (tested)
- Nested boards (board_link nodes, self-reference prevention, double-click enter, breadcrumb back, sub-board creation)
- Undo/Redo (Zustand command history, Ctrl+Z/Ctrl+Shift+Z, bounded 50)
- Viewport + drag persistence (batch on stop, no per-frame writes)
- Canvas LOD (minimal/summary/full based on zoom, editor gating)
- Canvas image/screenshot cards render bounded thumbnails (240px lazy load,
  P1-CAP-007 pipeline; icon fallback, pointer-events preserved for dragging)
- BRD-002 Canvas paste (Ctrl+V captures clipboard at mouse position)
- BRD-005 Edges (create/delete, self-loop rejection, cascade on node removal)
- BRD-006 Board hierarchy organization (Phase-2 Workstream A): folder CRUD
  (create/rename/reorder/trash/restore), cycle-safe folder moves, board
  favorite toggle + restore-from-trash, board/folder reorder sort-keys;
  sidebar tree (recursive folders→boards grouping, cycle-proof renderer)
- BRD-008 Sidebar drag-reorder (PRD BRD-001 "側邊欄可拖曳排序"): boards are
  draggable in the sidebar; drop takes the target's slot and persists via
  reorder_boards sort-key rewrite (pure computeBoardReorder, payload key
  isolated from inbox-item drags)
- BRD-009 Folder drag-reorder: folders draggable onto folders, persisted via
  reorder_folders; generic computeListReorder shared by boards & folders
  (reparenting stays a distinct move_folder action)
- BRD-007 Workstream A completion: Recent Boards (opening touches updated_at,
  sidebar 🕘 Recent section, limit-clamped command) + inline folder rename UI
  (✏ button / double-click, Enter save / Esc cancel)
- BRD-010 Node controls (Workstream C/D gaps closed): batch resize with 40px
  floor + non-finite rejection, lock toggle (locked cards not draggable/
  resizable, 🔒 affordance), bring-to-front on drag start (MAX(z)+1 in tx),
  group collapse (custom GroupNode frame + ▾ toggle; collapsed children are
  filtered from render while flags persist server-side); 4 new commands
  (resize_nodes/set_node_locked/set_node_collapsed/bring_node_to_front) with
  7 integration tests
- Workstream G keyboard controls: arrow-key movement of selected nodes
  (Shift = 1px fine step, locked nodes stay put, 400ms debounced batch
  persist — no DB write per keystroke), L toggles lock on selection, F fits
  view (prefers-reduced-motion honored); pure computeKeyboardMoves with 6
  unit tests
- Workstream F resize undo: NodeResizer gestures now push a createResizeCommand
  (pre-resize size captured on resize start, one history entry per completed
  interaction); Ctrl+Z/Ctrl+Shift+Z restores/re-applies sizes via resize_nodes
  (2 new unit tests)

### Phase 3 — Search ✓
- FTS5 unicode61 + trigram dual indexes
- SRC-002 Full filters: type/source/domain/board/after/before/favorite/inbox/session
- Filter-only browse (no text query), inline `key:value` syntax parser
- Ranking boosts (favorite, recency, exact title match)
- Trigram fallback for short/CJK queries (≤4 chars)
- Safe query building (parameterized SQL — injection prevention)
- Debounced search UI with filter chips
- Tag-based search (tags_text in search_documents)
- SRC-003 result actions: open detail, copy, favorite, open source, locate-on-board, add-to-board
- SRC-004 Search index maintenance (verify/rebuild/optimize commands; canonical data
  always recoverable if the derived FTS projection breaks; Settings UI section)
- source_app persistence fix (was hardcoded empty)
- SRC-003 Search actions: open detail, copy, favorite, open source (scheme-validated ShellExecute)
- SRC-005 Correctness fixtures (Phase-3 checklist): FTS injection payloads +
  control characters neutralized on the real MATCH path; Traditional Chinese
  no-space text, Japanese mixed scripts, camelCase/snake_case; trashed-item
  visibility policy; same-item-two-boards dedup. PLUS production fix: 1–2 char
  queries now work via wildcard-escaped LIKE fallback (FTS5 trigram needs ≥3 chars)

### Phase 4 — Windows ✓
- Screenshot (GDI BitBlt, multi-monitor virtual screen)
- CAP-006 Region screenshot (overlay selection, Esc cancel, multi-monitor/DPI, Ctrl+Shift+S)
- PNG encoding fix (screenshots now real lossless PNG, was raw BGRA)
- CAP-008 Browser extension (MV3 + Native Messaging host, Chrome/Edge registry)
- CAP-009 Cross-process capture notification: Explorer bridge & NMH notify the
  running app after commit (single-instance arg handoff → inbox auto-refresh
  without stealing focus); app-not-running activation (launch opens the app)
- CAP-007 Explorer right-click (file capture pipeline + bridge binary + registry, Inbox + last Board)
- CAP-007 IExplorerCommand COM DLL (Win10/11 classic menu) + Win11 sparse-package manifest/script
- File capture pipeline (content-addressed asset store, type detection, batch, DAT-001)
- Always-on-top toggle, show/hide/toggle window
- WIN-001 Tray menu (Open/QuickCapture/SaveClipboard/Pause/AlwaysOnTop/Settings/Quit);
  close-button behavior now honors ui.close_to_tray setting (Settings toggle;
  false = exit on close) per PRD; Pause/Resume menu item now fully wired
  (sessionToggle module: active→pause / paused→resume / never auto-starts
  monitoring — privacy-by-default)
- WIN-003 Window position+size memory (persist/restore via settings); FND-005
  offscreen bounds correction: restored geometry is validated against the
  monitors present now — removed-monitor positions clamp onto the largest
  display, degenerate sizes fall back to OS placement (7 unit tests)
- Autostart (Windows Registry HKCU\...\Run); WIN-002 compliance: --minimized
  autostart launch stays silent in the tray (no main window)
- WIN-004 Sleep/resume handling: deterministic wall-clock-gap suspend detector
  in job_runner pauses active capture sessions on system sleep; privacy-by-default
  — capture NEVER auto-resumes on wake (explicit user resume + toast notice)
- WIN-005 Hotkey re-registration: startup registers ALL 5 stored bindings
  (capture/screenshot/quick-capture/toggle-window/global-search); update_hotkey
  validates → persists → live re-registers without restart (empty unbinds);
  Settings UI editable hotkeys; Tauri→window event bridge fixes tray event delivery
- WIN-006 Versioned local IPC: NMH protocol gains protocolVersion (host rejects
  future versions explicitly, defaults v1 for old extensions), requestId
  correlation echoed in responses, capture-kind whitelist enforced before work;
  extension sends version+requestId; decision recorded in ADR-003
- SEC-001 SSRF hardening (deep-audit finding): metadata fetch now resolves DNS
  and rejects ANY private record (closes rebinding), validates every redirect
  hop (scheme + destination; Policy::none), classifies IPv4 broadcast/link-local
  and IPv6 ULA/link-local, fixes bracketed-IPv6-literal bypass
- SEC-002 Asset path hardening (deep-audit finding): resolve_path_checked
  allow-lists canonical content-addressed paths; reveal/preview/empty-trash
  (file DELETION) all refuse traversal/absolute/drive/ADS paths — hostile DB
  values can no longer read, expose, or delete files outside the store
- ARC-001 Layer-boundary enforcement (deep-audit finding): platform adapters
  (tray/hotkeys) no longer call the commands layer — window show/hide/toggle,
  always-on-top, quick-capture window and region overlay live in
  platform::window_manager; five architecture fitness tests scan the sources
  and fail CI on any reintroduced forbidden dependency
- Quick Capture floating window
- All Win32 code behind platform abstraction

### Content Type Cards ✓ (all integrated in InboxItem)
- plain_text, link, image, screenshot, color, code, pdf, font, rich_text, svg
- PDF first-page thumbnail (pdf.js render in ItemDetail, asset read via base64)

### Keyboard-first ✓
- Ctrl+Shift+V (capture), Ctrl+Shift+S (region screenshot), Ctrl+Shift+C (quick capture), Ctrl+Shift+P (toggle window), Ctrl+Shift+F (global search), Ctrl+V (canvas paste), Ctrl+K, Ctrl+Shift+B, Ctrl+Z/Ctrl+Shift+Z, Escape — all re-bindable in Settings (WIN-005)

### Testing ✓
- 288 Rust tests (28 suites incl. CAP-001 capture_png integration + 1 domain capture-method stability, 7 CAP-007 thumbnail pipeline (target size, box downscale bounds, PNG validity, refusal paths, variant path security), CAP-003 session-linked capture + 2 foreground-process-name extraction, 7 FND-005 geometry correction, CAP-002 focus holder semantics, 3 INB-003 triage compliance, 9 SRC-005 search correctness fixtures, 5 ARC-001 architecture fitness (source-scanned layer boundaries), PERF-001 FTS MATCH p95 benchmarks (20k/5k, real production query path), 5 SEC-002 asset path hardening, 5 SEC-001 SSRF hardening, 4 BRD-007 recent boards, 3 WIN-006 protocol validation, 5 CAP-009 notify, 2 WIN-005 hotkey validation/registry, 3 WIN-004 suspend detector + 1 WIN-004 session integration, 8 BRD-006 folders, 6 SRC-004 unit + 2 SRC-004 integration, 7 INB-004 + 6 BRD-005 + 11 SRC-002 + 5 SRC-003 + 3 CAP-006 PNG + 8 CAP-008 NMH + 14 CAP-007/002 explorer+files + 1 INB-003 pin + 1 INB-002 domain + 1 DAT-003 trash + 1 CAP-007 sparse pkg + 1 WIN-003 geom + 2 CAP-003 idle + 5 DAT-002 restore)
- 199 React tests (23 suites incl. 3 BRD-009 folder reorder + 5 FND-006 hotkey failure feedback + 7 BRD-008 drag-reorder + 8 WIN-001/CAP-004 session toggle + 5 INB-003 drag payload contract + 5 CAP-001 capture feedback + CAP-009 event bridge + BRD-006 boardTree + SRC-004 api contract + 500-node perf + toast + edge/node mapper + query parser + search actions + region select + PDF renderer + INB-002 grouping)
- 15 Zod contract tests
- 20 Playwright E2E specs (20/20 passing; app shell, inbox, search, navigation, trash, settings search-index, board sidebar controls, close-to-tray toggle)
- 6 performance benchmarks (PERF-001: FTS MATCH on the real query path with
  p95 statistics — all GOAL.md targets exceeded with headroom)
- 5 end-to-end pipeline tests (capture → search → find)

### Documentation ✓
- ADR-001: Modular Monolith with Strict Layer Boundaries
- ADR-002: SQLite with WAL Mode and Content-Addressed Assets
- ADR-003: Versioned Local IPC — activation handoff + Native Messaging (no named pipes; current-user ACL; version/request-id/message caps)
- README.md (incl. FND-001 dependencies & licenses table), STATUS.md
- FND-006 compliance: hotkey registration failures at startup are collected in
  managed state, drained via get_startup_hotkey_failures, and surfaced as an
  error toast with rebind guidance (visible + recoverable)

## 98 Tauri Commands

```
Capture (8) | Inbox (18: +get_item_thumbnail) | Search (4: +verify/rebuild/optimize index) | Board (26: +recent boards, folders CRUD/move/reorder, board favorite/restore/reorder) | Session (5)
Backup (4) | Settings (9: +get_startup_hotkey_failures) | Tags (7) | Window (10: +close_quick_capture focus-restore) | Health (1) | Browser (2) | Explorer (5)
```

## Remaining work (future iterations)

- (None blocking) Full GUI E2E against a built Tauri binary requires a Windows desktop session
  (WebView2 + display). UI shell is verified via Playwright against the dev server.

## 2026-08-13 product-completion pass

- Rebuilt the main window around the UX specification: 248px workspace navigation,
  fixed command/search bar, central Inbox/Board workspace and a responsive 320px
  item inspector. Added dark/light system themes, visible keyboard focus and a
  compact navigation rail for narrow windows.
- Routed the `quick-capture` Tauri label to the dedicated Quick Capture surface
  instead of rendering the full application. The native window is now 460x280,
  DPI-aware, cursor-adjacent, edge-flipping and monitor-bounds-clamped.
- Made shell actions truthful: Favorites executes `favorite:true` search, Capture
  opens the native Quick Capture window (and focuses the inline composer in browser
  previews), and the command bar reflects real capture-session state.
- Completed Inbox organization controls in the UI: explicit item selection,
  deterministic batch placement, batch Archive/Trash, individual Archive, and
  visible failure feedback.
- Added `revision` to Inbox/Trash item summaries and use it for title edits, removing
  the stale hard-coded optimistic-concurrency token.
- Restored reproducible frontend quality gates with an ESLint 9 flat configuration;
  a forced frozen pnpm reinstall repaired the relocated virtual-store links.
- Current evidence: ESLint clean; TypeScript clean; 207 desktop Vitest + 15 contract
  tests pass; production Vite build passes; Playwright 22/22 passes; Rust fmt and
  workspace Clippy pass; 295 Rust tests pass. Browser visual checks cover 1280x800,
  800x600 and 640x700. A built Tauri/WebView2 manual interaction pass is still
  required for OS-level focus restoration, cursor placement and native drag/drop.

## Final verification record (audit round)

- Fresh full-gate run: cargo fmt --check clean; clippy -D warnings 0 errors;
  cargo test --workspace 278 passed; tsc 0 errors (both packages); 196 React
  tests passed; 15 contract tests passed; 20/20 Playwright E2E passed.
- Browser-based verification: app served at localhost:1420; DOM snapshot shows
  the full Inbox shell (sidebar with Boards section, New folder/New board
  buttons, Start Session, Trash, Settings; search bar with filter chips; Quick
  Capture panel; Inbox grouping tabs Date/Source/Domain/Type/Session; empty
  state with Ctrl+Shift+V hint). Playwright E2E performs real interactive
  checks (navigation clicks, search input, settings toggles).
