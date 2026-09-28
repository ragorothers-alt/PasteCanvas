use tauri::Manager;
use tracing_subscriber::EnvFilter;

use crate::bootstrap::app_state::AppState;
use crate::commands;
use crate::infrastructure::database::migrator;
use crate::infrastructure::database::pool::AppDatabase;
use crate::jobs::job_runner;
use crate::platform::{hotkey, tray};

pub fn build_and_run() {
    // Initialize structured logging — no captured content in logs
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    tracing::info!(event = "app_starting", version = env!("CARGO_PKG_VERSION"));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            use tauri::Emitter;
            // CAP-009: an out-of-process integration (Explorer bridge / NMH)
            // committed a capture — refresh the Inbox without stealing focus.
            if crate::platform::notify::is_capture_notify(&args) {
                let _ = app.emit(crate::events::ITEM_CAPTURED, ());
                let _ = crate::platform::window_manager::show_main_window(app);
                return;
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            // Resolve app data directory
            let app_data_dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&app_data_dir).expect("create app data dir");

            let db_path = app_data_dir.join("pastecanvas.sqlite3");

            // DAT-002: apply a pending restore before opening the DB (safe point)
            if let Err(e) =
                crate::application::backup_service::BackupService::apply_pending_restore(
                    &app_data_dir,
                )
            {
                tracing::error!(event = "pending_restore_failed", error = %e);
            }

            // Open database, run migrations and persist the device id in one
            // runtime entry (single block_on instead of several).
            let (db, device_id) = tauri::async_runtime::block_on(async {
                let db = AppDatabase::open(&db_path).await.expect("open database");
                migrator::run_migrations(db.pool())
                    .await
                    .expect("run migrations");
                let device_id = db
                    .load_or_create_current_device()
                    .await
                    .expect("persist device id");
                (db, device_id)
            });

            // Manage shared state
            app.manage(AppState {
                db: db.clone(),
                device_id: device_id.clone(),
                data_dir: app_data_dir.clone(),
                clipboard_listener:
                    crate::platform::clipboard::listener::ClipboardListenerState::new(),
                hotkey_failures: std::sync::Mutex::new(Vec::new()),
            });

            // Spawn background job runner
            job_runner::spawn_job_runner(db.clone(), app.handle().clone());

            // Set up system tray
            tray::setup_tray(app.handle())?;

            // Initialize default settings/exclusions, then register global
            // hotkeys from the stored bindings (defaults were just seeded).
            // Both share one runtime entry; failures are collected and
            // surfaced to the UI (WIN-005 / FND-006).
            {
                let handle = app.handle().clone();
                let settings_db = db.clone();
                let hotkey_db = db.clone();
                let failures_state = &app.state::<AppState>().hotkey_failures;
                tauri::async_runtime::block_on(async {
                    let svc =
                        crate::application::settings_service::SettingsService::new(settings_db);
                    if let Err(e) = svc.initialize_defaults().await {
                        tracing::warn!(event = "settings_init_failed", error = %e);
                    }

                    let svc =
                        crate::application::settings_service::SettingsService::new(hotkey_db);
                    let mut failures: Vec<String> = Vec::new();
                    match svc.list_hotkeys().await {
                        Ok(bindings) => {
                            for b in bindings {
                                if !b.enabled || b.accelerator.trim().is_empty() {
                                    continue;
                                }
                                if let Err(e) = hotkey::register_action_hotkey(
                                    &handle,
                                    &b.action,
                                    &b.accelerator,
                                ) {
                                    tracing::error!(
                                        event = "hotkey_registration_failed",
                                        action = %b.action,
                                        error = %e
                                    );
                                    failures.push(format!("{} ({})", b.action, e));
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(event = "hotkey_bindings_load_failed", error = %e);
                            // Fall back to the two essential defaults.
                            if let Err(e2) = hotkey::register_capture_hotkey(&handle) {
                                failures.push(format!("capture_clipboard ({e2})"));
                            }
                            if let Err(e2) = hotkey::register_screenshot_hotkey(&handle) {
                                failures.push(format!("screenshot ({e2})"));
                            }
                        }
                    }
                    if !failures.is_empty() {
                        if let Ok(mut guard) = failures_state.lock() {
                            *guard = failures;
                        }
                    }
                });
            }

            let window = app
                .get_webview_window("main")
                .expect("main window must exist");

            // WIN-003: restore window position/size from settings
            {
                let db_for_geom = db.clone();
                let win_for_geom = window.clone();
                tauri::async_runtime::spawn(async move {
                    restore_window_geometry(&db_for_geom, &win_for_geom).await;
                });
            }

            // WIN-001: close behavior is user-configurable (ui.close_to_tray,
            // default true). WIN-003: geometry is saved whenever we hide.
            let window_clone = window.clone();
            let db_clone = db.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // Read the setting synchronously — the window event handler
                    // cannot await; the DB pool is always reachable here.
                    let close_to_tray = tauri::async_runtime::block_on(async {
                        let svc = crate::application::settings_service::SettingsService::new(
                            db_clone.clone(),
                        );
                        svc.get_setting("ui.close_to_tray").await.ok().flatten()
                    })
                    .map(|v| v != "false")
                    .unwrap_or(true);

                    if !close_to_tray {
                        tracing::info!(event = "window_close_exit");
                        return; // honor the close: destroy window, app exits
                    }

                    api.prevent_close();
                    // Save geometry before hiding (WIN-003)
                    let db_g = db_clone.clone();
                    let win_g = window_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        save_window_geometry(&db_g, &win_g).await;
                    });
                    let _ = window_clone.hide();
                    tracing::debug!(event = "window_hidden_to_tray");
                }
            });

            // WIN-002: autostart launches with --minimized — the app must run
            // silently in the tray without showing the main window.
            let launched_minimized = std::env::args().any(|a| a == "--minimized");
            if launched_minimized {
                tracing::info!(event = "app_started_minimized");
            } else {
                window.show()?;
            }

            tracing::info!(event = "app_ready", db_path = %db_path.display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health::health_check,
            commands::capture::capture_text,
            commands::capture::get_region_capture_context,
            commands::capture::capture_region_screenshot,
            commands::capture::cancel_region_capture,
            commands::browser::register_browser_host,
            commands::browser::unregister_browser_host,
            commands::explorer::register_explorer_integration,
            commands::explorer::unregister_explorer_integration,
            commands::explorer::capture_files,
            commands::explorer_menu::register_modern_explorer_menu,
            commands::explorer_menu::unregister_modern_explorer_menu,
            commands::inbox::query_inbox,
            commands::inbox::toggle_favorite,
            commands::inbox::toggle_pin,
            commands::inbox::reveal_item_asset,
            commands::inbox::get_trash_stats,
            commands::inbox::query_trash,
            commands::inbox::empty_trash,
            commands::inbox::trash_item,
            commands::inbox::archive_item,
            commands::inbox::batch_archive,
            commands::inbox::batch_trash,
            commands::inbox::restore_item,
            commands::inbox::edit_item,
            commands::inbox::get_item_content,
            commands::inbox::open_source,
            commands::inbox::get_item_asset_base64,
            commands::inbox::get_item_thumbnail,
            commands::search::search_items,
            commands::search::verify_search_index,
            commands::search::rebuild_search_index,
            commands::search::optimize_search_index,
            commands::boards::create_board,
            commands::boards::list_boards,
            commands::boards::get_recent_boards,
            commands::boards::get_board_snapshot,
            commands::boards::place_item_on_board,
            commands::boards::get_item_board_nodes,
            commands::boards::move_nodes,
            commands::boards::resize_nodes,
            commands::boards::set_node_locked,
            commands::boards::set_node_collapsed,
            commands::boards::bring_node_to_front,
            commands::boards::save_viewport,
            commands::boards::rename_board,
            commands::boards::trash_board,
            commands::boards::move_board,
            commands::boards::remove_node,
            commands::boards::group_nodes,
            commands::boards::ungroup_nodes,
            commands::boards::place_board_link,
            commands::boards::create_edge,
            commands::boards::delete_edge,
            commands::boards::restore_board,
            commands::boards::list_trashed_boards,
            commands::boards::list_trashed_folders,
            commands::boards::toggle_board_favorite,
            commands::boards::reorder_boards,
            commands::boards::list_folders,
            commands::boards::create_folder,
            commands::boards::rename_folder,
            commands::boards::move_folder,
            commands::boards::trash_folder,
            commands::boards::restore_folder,
            commands::boards::reorder_folders,
            commands::sessions::start_capture_session,
            commands::sessions::pause_capture_session,
            commands::sessions::resume_capture_session,
            commands::sessions::finish_capture_session,
            commands::sessions::get_capture_session,
            commands::backup::create_backup,
            commands::backup::create_export,
            commands::backup::validate_backup,
            commands::backup::prepare_restore,
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::settings::list_exclusions,
            commands::settings::add_exclusion,
            commands::settings::toggle_exclusion,
            commands::settings::remove_exclusion,
            commands::settings::list_hotkeys,
            commands::settings::get_startup_hotkey_failures,
            commands::settings::update_hotkey,
            commands::tags::create_tag,
            commands::tags::list_tags,
            commands::tags::rename_tag,
            commands::tags::delete_tag,
            commands::tags::add_tag_to_item,
            commands::tags::remove_tag_from_item,
            commands::tags::get_item_tags,
            commands::window::set_autostart,
            commands::window::get_autostart,
            commands::window::show_quick_capture,
            commands::window::close_quick_capture,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PasteCanvas application");
}

/// Persisted main-window geometry (WIN-003).
#[derive(serde::Serialize, serde::Deserialize)]
struct WindowGeometry {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

const WINDOW_GEOMETRY_KEY: &str = "app.window_geometry";

/// FND-005/WIN-003: corrects saved window bounds that no longer fit any
/// current monitor (monitor unplugged, resolution changed). The window is
/// clamped so at least a usable portion stays visible on some monitor; when
/// nothing fits, the geometry is discarded (OS default placement).
fn fit_geometry_to_monitors(
    geom: WindowGeometry,
    monitors: &[(i32, i32, u32, u32)], // (x, y, width, height) physical
) -> Option<WindowGeometry> {
    // Degenerate saved sizes are never restored.
    if geom.width < 200 || geom.height < 200 {
        return None;
    }
    if monitors.is_empty() {
        return None;
    }

    // Does any monitor contain the saved top-left corner?
    let anchored = monitors.iter().any(|&(mx, my, mw, mh)| {
        geom.x >= mx && geom.x < mx + mw as i32 && geom.y >= my && geom.y < my + mh as i32
    });
    if anchored {
        return Some(geom);
    }

    // Offscreen: clamp onto the monitor with the largest area.
    let (mx, my, mw, mh) = monitors
        .iter()
        .max_by_key(|&&(_, _, w, h)| w as u64 * h as u64)?;
    let mw = *mw as i32;
    let mh = *mh as i32;

    // Size clamped to the monitor.
    let width = geom.width.min(mw);
    let height = geom.height.min(mh);
    // Position clamped so the whole window stays inside the monitor.
    let x = geom.x.clamp(*mx, *mx + (mw - width).max(0));
    let y = geom.y.clamp(*my, *my + (mh - height).max(0));

    Some(WindowGeometry {
        x,
        y,
        width,
        height,
    })
}

/// Saves the main window's position + size to settings (WIN-003).
async fn save_window_geometry(
    db: &crate::infrastructure::database::pool::AppDatabase,
    window: &tauri::WebviewWindow,
) {
    let pos = match window.outer_position() {
        Ok(p) => p,
        Err(_) => return,
    };
    let size = match window.inner_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    let geom = WindowGeometry {
        x: pos.x,
        y: pos.y,
        width: size.width as i32,
        height: size.height as i32,
    };
    if let Ok(json) = serde_json::to_string(&geom) {
        let svc = crate::application::settings_service::SettingsService::new(db.clone());
        let _ = svc.set_setting(WINDOW_GEOMETRY_KEY, &json).await;
        tracing::debug!(event = "window_geometry_saved", x = geom.x, y = geom.y);
    }
}

/// Restores the main window's position + size from settings (WIN-003),
/// correcting offscreen bounds against the current monitor set (FND-005).
async fn restore_window_geometry(
    db: &crate::infrastructure::database::pool::AppDatabase,
    window: &tauri::WebviewWindow,
) {
    let svc = crate::application::settings_service::SettingsService::new(db.clone());
    let stored = match svc.get_setting(WINDOW_GEOMETRY_KEY).await {
        Ok(Some(v)) => v,
        _ => return,
    };
    let geom: WindowGeometry = match serde_json::from_str(&stored) {
        Ok(g) => g,
        Err(_) => return,
    };

    // FND-005: clamp/validate against the monitors present right now.
    let monitors: Vec<(i32, i32, u32, u32)> = window
        .available_monitors()
        .map(|ms| {
            ms.into_iter()
                .map(|m| {
                    let p = m.position();
                    let s = m.size();
                    (p.x, p.y, s.width, s.height)
                })
                .collect()
        })
        .unwrap_or_default();
    let geom = match fit_geometry_to_monitors(geom, &monitors) {
        Some(g) => g,
        None => return, // degenerate or no monitors — keep OS default placement
    };

    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
        x: geom.x,
        y: geom.y,
    }));
    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
        width: geom.width as u32,
        height: geom.height as u32,
    }));
    tracing::debug!(event = "window_geometry_restored", x = geom.x, y = geom.y);
}

#[cfg(test)]
mod tests {
    use super::{WindowGeometry, fit_geometry_to_monitors};

    fn geom(x: i32, y: i32, w: i32, h: i32) -> WindowGeometry {
        WindowGeometry {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn window_geometry_serde_round_trip() {
        let g = geom(120, 80, 1280, 800);
        let json = serde_json::to_string(&g).expect("serialize");
        let back: WindowGeometry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.x, 120);
        assert_eq!(back.y, 80);
        assert_eq!(back.width, 1280);
        assert_eq!(back.height, 800);
    }

    // --- FND-005 offscreen bounds correction ---

    #[test]
    fn onscreen_geometry_is_restored_unchanged() {
        let monitors = [(0, 0, 1920, 1080)];
        let g = geom(100, 100, 1280, 800);
        let out = fit_geometry_to_monitors(g, &monitors).expect("fit");
        assert_eq!((out.x, out.y, out.width, out.height), (100, 100, 1280, 800));
    }

    #[test]
    fn negative_coordinates_on_left_monitor_are_anchored() {
        // Second monitor to the left of primary
        let monitors = [(-1920, 0, 1920, 1080), (0, 0, 1920, 1080)];
        let g = geom(-1600, 200, 1280, 800);
        let out = fit_geometry_to_monitors(g, &monitors).expect("fit");
        assert_eq!((out.x, out.y), (-1600, 200));
    }

    #[test]
    fn fully_offscreen_geometry_is_clamped_to_largest_monitor() {
        // Saved on a monitor that no longer exists
        let monitors = [(0, 0, 1920, 1080), (1920, 0, 1280, 720)];
        let g = geom(3000, 2000, 1000, 700);
        let out = fit_geometry_to_monitors(g, &monitors).expect("fit");
        // Largest monitor is the 1920x1080 primary
        assert!(out.x >= 0 && out.x + out.width <= 1920);
        assert!(out.y >= 0 && out.y + out.height <= 1080);
    }

    #[test]
    fn oversized_geometry_with_onscreen_corner_is_kept_usable() {
        // Corner visible → the user can move/resize it; the window may
        // legitimately straddle monitors, so size is left untouched.
        let monitors = [(0, 0, 1280, 720)];
        let g = geom(500, 500, 2000, 1500);
        let out = fit_geometry_to_monitors(g, &monitors).expect("fit");
        assert_eq!((out.x, out.y), (500, 500));
        assert_eq!((out.width, out.height), (2000, 1500));
    }

    #[test]
    fn oversized_offscreen_geometry_is_clamped_size_and_position() {
        let monitors = [(0, 0, 1280, 720)];
        let g = geom(5000, 5000, 2000, 1500);
        let out = fit_geometry_to_monitors(g, &monitors).expect("fit");
        assert!(out.width <= 1280);
        assert!(out.height <= 720);
        assert!(out.x >= 0 && out.x + out.width <= 1280);
        assert!(out.y >= 0 && out.y + out.height <= 720);
    }

    #[test]
    fn degenerate_sizes_are_discarded() {
        let monitors = [(0, 0, 1920, 1080)];
        assert!(fit_geometry_to_monitors(geom(0, 0, 100, 800), &monitors).is_none());
        assert!(fit_geometry_to_monitors(geom(0, 0, 800, 50), &monitors).is_none());
    }

    #[test]
    fn no_monitors_discards_geometry() {
        assert!(fit_geometry_to_monitors(geom(0, 0, 800, 600), &[]).is_none());
    }
}
