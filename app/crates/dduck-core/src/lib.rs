//! D.Duck core: transcript ingestion, storage, analytics and Claude CLI integration.
//! Kept independent of Tauri so it can be reused by a headless daemon later.

pub mod agent_controls;
pub mod agents_catalog;
pub mod analytics;
pub mod artifact_preview;
pub mod artifacts;
pub mod attachment;
pub mod auth_status;
pub mod brief_refine;
pub mod authoring;
pub mod claude_cli;
pub mod cli_path;
pub mod cloud_routines;
pub mod commands;
pub mod db;
pub mod delivery_guard;
pub mod discover;
pub mod evidence;
pub mod ext_trash;
mod error;
pub mod http;
pub mod i18n;
pub mod insights;
pub mod install_link;
pub mod ingest;
pub mod labels;
pub mod parse;
pub mod permissions;
pub mod pricing;
pub mod prompt;
pub mod raw;
pub mod review_inbox;
pub mod routines;
pub mod report_guard;
pub mod rules;
mod process;
pub mod procs;
pub mod projects;
pub mod mcp_config;
pub mod settings;
pub mod skills_catalog;
pub mod task_lifecycle;
pub mod task_meta;
pub mod task_report;
pub mod team;
pub mod team_config;
pub mod terminal_win;
pub mod text;
#[doc(hidden)]
pub mod testkit;
pub mod timeline;
pub mod limit_resume;
pub mod live_agents;
pub mod login_output;
pub mod login_session;
pub mod loadout;
pub mod mcp_list;
pub mod memory;
pub mod model_select;
pub mod news;
pub mod plugin_bulk;
pub mod platform;
pub mod plugins;
pub mod preferred;
pub mod tracker;
pub mod trust;

pub use error::Error;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
