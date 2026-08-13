pub mod application;
pub mod bootstrap;
pub mod commands;
pub mod domain;
pub mod events;
pub mod infrastructure;
pub mod jobs;
pub mod platform;

use bootstrap::app_builder;

pub fn run() {
    app_builder::build_and_run();
}
