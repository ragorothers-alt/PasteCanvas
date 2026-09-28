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

## 95 Tauri Commands (measured 2026-08-21, round 16–18)

```
boards (33) | inbox (17) | settings (9) | tags (7) | sessions (5) |
backup (4) | capture (4) | search (4) | window (4) | explorer (3) |
browser (2) | explorer_menu (2) | health (1)
```

Dead shells removed in rounds 16–18: capture_clipboard, capture_image,
capture_screenshot, start_region_capture, toggle/get_always_on_top,
show/hide/toggle_main_window, hide_quick_capture (platform functions
get_always_on_top/hide_main_window removed with them).

## Remaining work (future iterations)

- (None blocking) Full GUI E2E against a built Tauri binary requires a Windows desktop session
  (WebView2 + display). UI shell is verified via Playwright against the dev server.

## 2026-08-21 architecture optimization pass

### Round 25 — audit coverage complete; convergence verified

- packages/config was an empty vestigial directory (no package.json, nothing
  tracked) — removed; pnpm workspace glob now matches only real packages.
- design-qa.md verified consistent with the implementation (Traditional
  Chinese UI, themes, viewports). SCAN_REPORT.md (2026-08-16) confirmed as
  the historical input artifact; its High findings are all addressed in the
  rounds above (SVG sanitize wiring, SEC-001 SSRF/DNS-rebinding, fail-closed
  exclusions, open_source scheme validation, WIN-006 NMH protocol).
- No code changes this round; consolidated gate re-run on the current tree:
  fmt 0, Rust 341/29 suites, dead-command scan OK (95). FE/E2E unchanged
  since their last green runs (217 Vitest, 22/22 E2E).
- Audit state: every code surface in the workspace has been read and
  audited; every finding fixed and gate-verified; continuity mechanized in
  CI (rust-gate, frontend-gate, arch-gate, e2e). Remaining items are
  reasoned and documented, not overlooked: move_folder UI (allowlisted
  product gap), desktop-session startup timing (environment-blocked),
  committing the work (user decision).

### Round 24 — COM DLL lifetime correctness (last unread code)

- pastecanvas-explorer-dll audited (final unread surface). Found a real COM
  lifetime bug: MODULE_REFCOUNT was only touched by LockServer, so
  DllCanUnloadNow could report S_OK while Explorer still held IExplorerCommand
  pointers — an unload in that window would crash the shell. Fixed with the
  standard pattern: Drop impls on PasteCanvasCommand/PasteCanvasFactory
  decrement, creation sites increment; DllCanUnloadNow is now truthful.
- Removed the E_NOINTERFACE import-suppression hack (dead const).
- Verification (round 24): cargo fmt clean; clippy -D warnings 0 (DLL
  compiles on Windows); cargo test --workspace 341 passed.
- With this, every code surface in the workspace has been read and audited:
  all Rust layers/jobs/platform/commands/DLL, all TS dirs, the browser
  extension, and both packages' docs.

### Round 23 — workspace-level doc truth + repo hygiene

- Browser extension audited: MV3, minimal permissions (contextMenus +
  nativeMessaging), WIN-006 protocol fields present — clean, no changes.
- Removed apps/desktop/bs_grep.txt (leftover scratch file from round-15
  sweep; untracked junk).
- pastecanvas-blueprint/STATUS.md corrected: it still claimed "Not started —
  no implementation exists". Now points to the implementation's measured
  record (phase completion, release perf numbers, migration v3, test gates).

### Round 22 — last blocking OS work off the async runtime

- Final sweep of the commands layer for subprocess/blocking OS calls on async
  threads; every registration path spawns reg.exe (50–150ms each) or
  PowerShell (seconds):
  - explorer_menu register/unregister (≈12 reg spawns + PowerShell)
  - browser host register/unregister (manifest write + reg spawns)
  - explorer integration register/unregister (6 reg spawns)
  - set_autostart (1 reg spawn)
  - empty_trash orphan file deletions (N remove_file calls)
  All moved to spawn_blocking; async runtime no longer stalls on OS work.
- Verification (round 22): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 341 passed (29 suites). Frontend untouched.

### Round 21 — last render-path memoization + consolidated final gate

- App.tsx sidebar tree and useInbox grouping were recomputed on every
  render; both now useMemo'd ([folders, boards] / [items, groupMode]).
- Consolidated final gate, all green in one pass:
  cargo fmt --check 0 · clippy -D warnings 0 · cargo test 341/29 suites ·
  ESLint 0 · tsc 0 · Vitest 217 + contracts 15 · dead-command scan OK (95) ·
  Playwright E2E 22/22.

### Round 20 — release-mode benchmarks + join-column index audit

- PERF-001 re-measured in RELEASE mode (prior numbers were debug builds):
  - capture_text: 0.80ms avg (target <10ms)
  - inbox query 1k: 0.30ms avg
  - FTS unicode MATCH 20k: p95 9.33ms (target <150ms, 16x headroom)
  - FTS trigram MATCH 20k: p95 0.18ms; FTS 5k: p95 0.17ms
  - 6/6 bench assertions pass in 3.9s
- query_inbox join columns audited: search_documents.item_id UNIQUE,
  item_assets PK(item_id, asset_id, role), source_contexts PK — every LEFT
  JOIN is index-backed. No change needed.

### Round 19 — dead-IPC methodology mechanized into CI

- scripts/check-dead-commands.mjs: every registered Tauri command must have a
  TS invoke caller or a Rust-internal code reference; string literals are
  stripped from the Rust side so tracing/log text never counts as a caller
  (the negative test proved the naive version counted `event = "health_check"`
  as a caller — fixed before shipping).
- Exceptions require a written reason in ALLOWLIST; stale allowlist entries
  (unregistered names) fail the check, so the allowlist cannot rot silently.
- Wired as `pnpm check:dead-commands` and as CI job `arch-gate` (ubuntu,
  node-only, seconds to run). Rounds 16–18's manual sweep is now machine
  discipline on every push/PR.
- Verified both directions: exit 0 on the current tree (95 commands, 2
  reasoned allowlist entries); exit 1 when an allowlist entry is removed.

### Round 18 — post-deletion orphan sweep + doc truth

- Round-16 command deletions left two platform-layer orphans (pub fns are
  invisible to clippy's dead_code): window_manager::get_always_on_top and
  ::hide_main_window — removed after verifying zero callers.
- STATUS command inventory replaced with measured per-module counts (95);
  the old "98 commands" block was stale documentation.
- Verification (round 18): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 341 passed (29 suites).

### Round 17 — application-layer audit complete, export unblocked

- Audited the last unreviewed application services:
  - session_service: clean (validated state machine, privacy-by-default,
    single-session invariant)
  - settings_service: clean (idempotent defaults, ON CONFLICT upserts backed
    by real UNIQUE constraints — verified against schema)
  - job_runner: clean (lease reclaim uses the partial index; FTS stays in
    sync with title updates via the AFTER UPDATE trigger — verified)
- One real find: create_export wrote one blocking std::fs::write per item on
  the async runtime (5k items = 5k runtime stalls). All file writes moved to
  spawn_blocking; DB reads stay async.
- Verification (round 17): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 341 passed (29 suites).

### Round 16 — dead IPC surface removed

- Systematic sweep: every registered Tauri command checked for TS invoke
  callers AND Rust-internal callers. ARC-001 (platform calls platform,
  never commands) left 11 command shells with zero callers anywhere —
  pure IPC attack surface with no function:
  - capture_clipboard, capture_image (frontend paths refactored away in R11)
  - capture_screenshot, start_region_capture (hotkeys call window_manager
    directly; also removed the now-orphaned platform::capture_screen)
  - toggle/get_always_on_top, show/hide/toggle_main_window, hide_quick_capture
    (tray/hotkeys call window_manager directly)
  - Underlying capabilities all remain live via their real paths.
- Kept: health_check (diagnostic), move_folder (documented product gap).
- Registered command count now measured at 95 (the old "98 commands"
  doc figure was never reconciled; treat measured counts as authoritative).
- Verification (round 16): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 341 passed; 22/22 Playwright E2E.

### Round 15 — last N+1: sidebar board summaries

- query_board_summaries (list_boards + get_recent_boards) ran a per-board
  COUNT(*) round trip inside a loop — N queries per sidebar refresh.
  Rewritten as one aggregated LEFT JOIN (node_count in the same SELECT);
  soft-deleted nodes still excluded by the aggregate's predicate.
- Regression test list_boards_reports_node_count_from_aggregate: 2 live +
  1 soft-deleted node → node_count == 2.
- Verification (round 15): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 341 passed (29 suites).

### Round 14 — test coverage for the new UI + browser verification

- Rounds 12–13 shipped UI with zero test coverage; closed that gap:
  - TrashView.test.tsx (5): restore section renders boards+folders, restore
    calls the command / hides the row / dispatches boards-changed, empty
    state, graceful backend failure.
  - TagSelector.test.tsx (5): dropdown management actions render, rename
    saves via Enter, Escape cancels, delete honors the confirm (and reports
    affected item count), cancelled confirm deletes nothing.
- Browser verification against the Vite dev server: inbox shell and the
  lazy-loaded TrashView render correctly (screenshots in output/);
  Suspense fallback observed and resolved without errors.
- Verification (round 14): 217/217 Vitest (26 suites); ESLint 0; tsc 0.

### Round 13 — tag management wired (rename/delete)

- Second product gap from the round-11 sweep closed: rename_tag / delete_tag
  backend commands (tested, registered since Phase 1) had no UI.
  - TagSelector dropdown rows gained management actions: inline rename
    (✏ → input, Enter saves / Esc cancels) and delete (🗑 with a confirm
    that reports how many items will lose the tag).
  - tags/api wrappers restored with real callers; delete_tag runs the
    existing transactional path that also rebuilds search tags_text.
- move_folder (folder reparenting) evaluated and intentionally left unwired:
  low-frequency operation needing a drag-across-parents interaction design;
  backend stays tested and registered. Recorded as the only remaining gap.
- Verification (round 13): tsc/ESLint 0; 207/207 Vitest; 22/22 Playwright
  E2E. Rust untouched this round (prior 340/340 still authoritative).

### Round 12 — wired the board/folder restore gap closed

- Round 11's dead-wrapper sweep exposed a product gap: restore_board /
  restore_folder backend commands had no UI path (STATUS claimed BRD-006
  restore since Phase 2). Closed it end-to-end instead of deleting working
  backend code:
  - Backend: TrashedEntryDto + list_trashed_boards / list_trashed_folders
    (lightweight deleted_at-ordered listings), two new commands registered.
  - Frontend: TrashView gained a “看板與資料夾” restore section (boards +
    folders, restore buttons, graceful no-backend fallback); restore dispatches
    paste-canvas://boards-changed which App listens to and refreshes the
    sidebar tree.
  - boards/api wrappers restored WITH real callers this time
    (restoreBoard/restoreFolder/listTrashed*) + TrashedEntry type +
    BOARDS_CHANGED_EVENT constant.
- Remaining documented gaps: move_folder (reparenting) — intentionally
  unwired, backend tested and registered.
- Verification (round 12): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 340 passed (2 new trash/restore round-trip tests);
  tsc/ESLint 0; 207/207 Vitest; 22/22 Playwright E2E.

### Round 11 — export-level dead code sweep

- Full sweep of exported functions/consts across features/stores/lib/adapters
  against every import site (app + tests). Results:
  - Deleted dead API wrappers (zero call sites, backend commands untouched):
    boards moveFolder/restoreFolder/restoreBoard, inbox captureClipboard/
    captureImage, tags renameTag/deleteTag, searchActions addToBoard.
    Note: folder/board restore remains a product gap — backend commands
    exist but no UI path reaches them.
  - Deleted CanvasMapper.extractMoves (+ its now-unused XYPosition import).
  - Demoted to module-private: THEME_STORAGE_KEY, CAPTURE_FAILURE_EVENTS,
    CAPTURE_SUCCESS_EVENTS (used only inside their own files).
  - Deleted RichTextEditor.tsx (zero mount points since Phase 1) and removed
    the @tiptap/pm + @tiptap/react + @tiptap/starter-kit dependencies it
    carried (round-1 deferral resolved: the repo is under git, deletion is
    recoverable). Pure rich-text adapters + their tests remain.
- Verification (round 11): pnpm install clean; tsc 0; ESLint 0; 207/207
  Vitest + 15/15 contracts; 22/22 Playwright E2E; production build clean.

### Round 10 — search path audit & shared helper bootstrap

- Search path audited: FTS unicode + trigram MATCH paths are index-driven;
  the 1–2 char LIKE fallback is a physical necessity (leading-wildcard LIKE
  cannot use a B-tree index) and remains as designed.
- Frontend runtime audited: useInbox is event-driven (no polling), mutations
  update local state optimistically; SessionControl's 5s session poll is a
  single local IPC — acceptable.
- Duplicated out-of-process bootstrap eliminated: pastecanvas-nmh and
  pastecanvas-explorer each carried their own copy of the data-dir layout
  (APP_IDENTIFIER + DB_FILENAME + resolve_data_dir) and open/migrate
  sequence. Extracted to bootstrap::external (resolve_data_dir +
  open_migrated_db); both binaries now consume it (~80 duplicate lines
  removed, layout defined exactly once).
- Verification (round 10): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 338 passed (29 suites).

### Round 9 — measured query plans, corrected round-8 finding

- Added hot_path_queries_use_their_indexes: EXPLAIN QUERY PLAN regression
  guard — an index that exists but is never chosen is the same as no index.
  Verified plans: URL dedup → ix_items_normalized_url, hash dedup →
  ix_items_content_hash, asset lookup → sqlite_autoindex_assets (implicit
  UNIQUE), inbox list → ix_items_inbox.
- Measurement corrected a round-8 finding: assets.sha256 was NEVER missing an
  index — the UNIQUE constraint in 001 already carries an implicit unique
  index. The ix_assets_sha256 added in 003 was redundant and has been removed
  from migration 003 (only ix_items_normalized_url remains).
- query_inbox default path rewritten: `inbox_status != 'archived'` (cannot use
  the partial index) → `inbox_status IN ('new','triaged')` (index-driven,
  semantically identical — status has exactly three values).
- Verification (round 9): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 338 passed (29 suites).

### Round 8 — index audit of the capture hot path

- Audited all 26 schema indexes against production query paths. One capture
  hot path was doing a full-table scan on every capture:
  - URL dedup (`find_item_by_normalized_url`) — ran per captured link with no
    index on items.normalized_url
  - (The assets.sha256 lookup flagged here turned out to be covered by the
    UNIQUE constraint's implicit index — see Round 9 correction.)
- Fixed via migration 003_capture_dedup_indexes (additive partial index;
  001/002 untouched per migration contract).
- New regression test capture_dedup_indexes_exist_after_migration; migrator
  version assertions bumped to 3.
- NMH per-invocation migration cost verified negligible (checksum lookups
  short-circuit; no writes on an up-to-date DB).
- Verification (round 8): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 337 passed (29 suites).

### Round 7 — gates frozen into CI

- Added .github/workflows/ci.yml (was: none — every gate was run by hand,
  which is why 13 stale E2E specs survived undetected for a week):
  - rust-gate: cargo fmt --check, clippy --workspace --all-targets
    -D warnings, cargo test --workspace (windows-latest — Win32 code only
    compiles there)
  - frontend-gate: pnpm install --frozen-lockfile, lint, typecheck,
    workspace vitest (desktop 207 + contracts-ts 15)
  - e2e: Playwright Chromium against the Vite dev server (22 specs),
    report uploaded as artifact on failure
- Every CI command was run locally this session and passes: frozen-lockfile
  install, root lint/typecheck/test, all Rust gates, E2E 22/22.

### Round 6 — measured release builds & last blocking paths

- First measured release build (no profile existed — shipped binaries were
  built with raw cargo defaults). Baseline vs optimized profile
  (`lto = "thin"`, `codegen-units = 1`, `strip = "symbols"` in workspace
  Cargo.toml):
  - pastecanvas-desktop.exe: 20.98MB → 18.81MB (-10%)
  - pastecanvas-nmh.exe: 4.03MB → 3.19MB (-21%)
  - pastecanvas-explorer.exe: 3.79MB → 2.91MB (-23%)
  - explorer COM DLL: 0.22MB → 0.21MB
- Last two blocking-on-async paths moved to the blocking pool:
  - backup_service create_backup: DB sha256 + recursive asset copy
  - link_metadata: reqwest Client rebuilt per job → shared OnceLock client
    (connection pool + TLS session reuse)
- PERF-001 benchmarks re-run: 6/6 pass (including 20k-item FTS p95).
- Verification (round 6): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 336 passed (29 suites); release build clean.
  Frontend untouched this round (prior gates still authoritative).

### Round 5 — startup consolidation, dead dep, detail-panel split

- app_builder setup consolidated: 4 sequential `block_on` runtime entries
  merged into 2 (DB open + migrations + device id in one; settings defaults +
  hotkey registration in one). Job runner / tray setup order preserved.
- Removed `zod` from @pastecanvas/desktop dependencies — zero references in
  app code (the contract schemas own their zod dependency in contracts-ts).
- ItemDetail (inspector + previews) is now a React.lazy chunk mounted only
  when an item is selected; main bundle 342KB → 335KB.
- Cumulative main-bundle weight across rounds: ~1.4MB eager → 335KB (-76%).
- Verification (round 5): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 336 passed (29 suites); pnpm lint/tsc clean;
  207/207 Vitest; 22/22 Playwright E2E; production build clean.

### Round 4 — blocking work off the async runtime & view code-splitting

- Audit found CPU/IO-heavy synchronous work running directly on tokio async
  tasks (would stall UI events, job runner and hotkeys during large captures).
  All of it moved to `tokio::task::spawn_blocking`:
  - New `AssetStore::stage_bytes_async` (SHA-256 + SVG sanitize + disk write +
    atomic rename); migrated all 5 staging call sites (clipboard PNG, file
    capture, explorer capture, capture_image command, both screenshot paths)
  - `capture_screen` / `capture_region` (GDI grab + PNG encode) wrapped at the
    command layer
  - New `snapshot_clipboard_async` for the tray and session-watcher paths
    (Win32 clipboard read + RGBA→PNG encode were inline in async tasks)
  - `get_item_thumbnail`: PNG decode + box downscale + encode + cache write
    (hottest path — inbox card first render); `get_item_asset_base64`: file
    read + base64 encode
- Frontend view code-splitting: BoardView (ReactFlow ~209KB), SettingsView
  (22KB) and TrashView (5KB) are now React.lazy chunks mounted on navigation.
  Main bundle 578KB → 342KB (-41%).
- Verification (round 4): cargo fmt clean; clippy -D warnings 0;
  cargo test --workspace 336 passed; pnpm lint/tsc clean; 207/207 Vitest;
  22/22 Playwright E2E; production build clean.

### Round 3 — false dynamic-import removal

- `@tauri-apps/api/event` was dynamically imported in tauriEvents.ts and
  SessionControl.tsx as a supposed browser-compat optimization, but
  lib/runtime.ts already statically imports @tauri-apps/api/window (and
  window.js statically imports event.js) — so event.js was always in the main
  chunk and the dynamic imports only added a Vite "dynamically imported but
  also statically imported" warning plus promise-chain complexity. Both sites
  converted to static imports (isTauri() guard preserved); build warning gone.
- Verification (round 3): pnpm lint clean; tsc 0 errors; 207/207 Vitest;
  production Vite build clean (no chunking warnings).

### Round 2 — N+1 query elimination & dead code sweep

- SQL N+1 hot paths rewritten set-based:
  - board_service group_nodes: N SELECTs + N UPDATEs → 1 IN select + 1 IN update
  - commands/inbox batch_archive_in_pool / batch_trash_in_pool: per-item UPDATE
    loop + transaction → single IN UPDATE (rows_affected straight from SQLite)
  - pool empty_trash orphan scan: 3 queries per candidate asset → 1 NOT EXISTS
    select + 1 IN delete
  - search_index_service rebuild: per-item tags_text_for query → single
    GROUP_CONCAT grouped lookup (helper removed)
- Deleted dead module stores/useKeyboardShortcuts.ts (zero imports; superseded
  by the paste-canvas:// event listeners); removed empty [dev-dependencies]
  section in pastecanvas-explorer-dll.
- New regression test: rebuild_restores_tags_text (covers the grouped
  tags path incl. alphabetical ordering and untagged items).
- Verification record (round 2): cargo fmt --check clean; clippy --workspace
  --all-targets -D warnings 0; cargo test --workspace 336 passed;
  pnpm lint clean; tsc 0 errors; 207/207 Vitest.

### Round 1 — dependencies & bundle

Dependency & bundle audit (zero-reference deps removed, heavy code split on demand):

- Removed dead dependencies verified by full-repo grep (0 usages each):
  Rust `anyhow` + `async-trait`; TS `@tanstack/react-query` (-2 packages installed).
- `tokio` workspace features trimmed from `full` to `rt + time + macros`
  (the only tokio APIs actually used: `spawn_blocking`, `time::sleep`,
  `#[tokio::test]`, current-thread runtime builder in NMH).
- Bundle code-splitting: pdf.js (~1MB) now loads via dynamic import inside
  PdfRenderer; CodeEditor (CodeMirror core + 3 language grammars, ~516KB) and
  PdfPreview are React.lazy chunks mounted only when a code/PDF item is opened.
  Main bundle 578KB; previously all of it shipped eagerly (~1.4MB).
- Pure language-detection heuristics extracted to adapters/code/languageDetect.ts
  (no CodeMirror dependency); CodeMirrorAdapter re-exports them for compatibility.
- Clippy 1.97 findings fixed: collapsible_if (svg_sanitize), unnecessary
  lazy evaluation (clipboard CF_HTML decode — the no-op fallback claimed a
  latin1 path that was never implemented), too_many_arguments
  (insert_captured_asset_item 14 params → CapturedAssetInsert struct, 6 call sites).
- E2E repair: 13 of 22 Playwright specs had been stale since the 2026-08-13
  UX rebuild (asserting English labels/flat settings against the shipped
  Traditional-Chinese UI + settings category rail). All selectors updated to
  the real UI incl. focus-within filter chips; 22/22 pass in ~28s.
- Verification record: cargo fmt clean; clippy --workspace --all-targets
  -D warnings 0; cargo test --workspace 335 passed; pnpm lint clean;
  pnpm typecheck 0 errors; 207/207 Vitest; 22/22 Playwright E2E;
  production Vite build passes with code-split chunks.
- Known dead weight (removed in Round 11 once git history existed):
  @tiptap/* deps + RichTextEditor.tsx had no production mount point.

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
