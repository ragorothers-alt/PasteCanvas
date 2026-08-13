//! Platform / Windows adapters — clipboard listener, hotkeys, foreground process,
//! tray, autostart, graphics capture, monitors/DPI, focus restore, native IPC.
//!
//! All Windows-only code lives here, behind port abstractions.

pub mod clipboard;
pub mod clipboard_watch;
pub mod focus;
pub mod hotkey;
pub mod native_messaging;
pub mod notify;
pub mod tray;
pub mod window_manager;

#[cfg(target_os = "windows")]
pub mod windows;
