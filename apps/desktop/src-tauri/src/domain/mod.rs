//! Domain layer — entities, value objects, and business rules.
//!
//! This module MUST NOT depend on Tauri, Win32, SQLite, or WebView.
//! All domain logic must be unit-testable in isolation.

pub mod color;
pub mod entities;
pub mod error;
pub mod font;
pub mod svg_sanitize;
pub mod value_objects;
