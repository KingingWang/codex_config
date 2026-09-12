//! Codex 配置助手 — a GUI for editing a Codex installation's `config.toml`
//! and the model catalog JSON it points at.
//!
//! The crate is split so that the GUI can be driven from tests:
//! [`doc`] owns all file parsing/writing, [`ui`] owns rendering, and [`app`]
//! glues the two together.

pub mod app;
pub mod dialogs;
pub mod doc;
pub mod editors;
pub mod net;
pub mod page;
pub mod remote;
pub mod remote_browser;
pub mod remote_ui;
pub mod server;
pub mod ssh_config;
pub mod ui;

pub use app::App;
