//! Infrastructure layer — SQLite repositories, asset store, metadata fetcher,
//! thumbnail generator, backup/export, logging, job queue.
//!
//! Implements ports defined by the application layer.

pub mod asset_store;
pub mod database;
pub mod thumbnail;
