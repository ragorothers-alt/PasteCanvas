//! Jobs — durable background job coordination.
//! Job types: fetch_link_metadata, generate_image_thumbnail, render_pdf_thumbnail,
//! extract_pdf_metadata, rebuild_search_document, cleanup_orphan_asset, build_backup.

pub mod job_runner;
pub mod link_metadata;
