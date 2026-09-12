//! Centralised icon glyphs, powered by the Phosphor icon font.
//!
//! Keeping every glyph in one place means the rest of the UI just refers to
//! `icons::MODELS` etc. and never has to know the raw Phosphor constant names.

pub use egui_phosphor::regular as ph;

// -- App / branding ---------------------------------------------------------
pub const BRAND: &str = ph::CUBE_TRANSPARENT;

// -- Pages ------------------------------------------------------------------
pub const OVERVIEW: &str = ph::HOUSE;
pub const MODELS: &str = ph::ROBOT;
pub const PROVIDERS: &str = ph::PLUGS_CONNECTED;
pub const PROFILES: &str = ph::CARDS;
pub const ADVANCED: &str = ph::SLIDERS_HORIZONTAL;
pub const RAW: &str = ph::FILE_CODE;

// -- Section headers --------------------------------------------------------
pub const ACTIVE_MODEL: &str = ph::SPARKLE;
pub const SECURITY: &str = ph::SHIELD_CHECK;
pub const FILES: &str = ph::FOLDER_OPEN;
pub const OUTPUT: &str = ph::LIST_BULLETS;
pub const STEPS: &str = ph::MAP_TRIFOLD;
pub const BASICS: &str = ph::IDENTIFICATION_CARD;
pub const REASONING: &str = ph::BRAIN;
pub const CONTEXT: &str = ph::STACK;
pub const CAPABILITIES: &str = ph::WRENCH;
pub const GEAR: &str = ph::GEAR_SIX;
pub const PROTOCOL: &str = ph::TREE_STRUCTURE;
pub const ADDRESS: &str = ph::GLOBE;
pub const AUTH: &str = ph::KEY;
pub const TEST: &str = ph::PULSE;
pub const FLASK: &str = ph::FLASK;
pub const TERMINAL: &str = ph::TERMINAL_WINDOW;
pub const MCP: &str = ph::PLUG;

// -- Actions ----------------------------------------------------------------
pub const SAVE: &str = ph::FLOPPY_DISK;
pub const REVERT: &str = ph::ARROW_COUNTER_CLOCKWISE;
pub const DIFF: &str = ph::GIT_DIFF;
pub const ADD: &str = ph::PLUS;
pub const DELETE: &str = ph::TRASH;
pub const DUPLICATE: &str = ph::COPY;
pub const RENAME: &str = ph::PENCIL_SIMPLE;
pub const DOWNLOAD: &str = ph::DOWNLOAD_SIMPLE;
pub const SEARCH: &str = ph::MAGNIFYING_GLASS;
pub const OPEN_FOLDER: &str = ph::FOLDER_OPEN;
pub const INFO: &str = ph::INFO;
pub const CARET_RIGHT: &str = ph::CARET_RIGHT;
pub const CARET_DOWN: &str = ph::CARET_DOWN;
pub const CARET_LEFT: &str = ph::CARET_LEFT;
pub const CHECK: &str = ph::CHECK_CIRCLE;
pub const WARNING: &str = ph::WARNING;
pub const ERROR: &str = ph::WARNING_OCTAGON;
pub const DOT: &str = ph::CIRCLE;
pub const CLOSE: &str = ph::X;

// -- Restart app-server -----------------------------------------------------
pub const RESTART: &str = ph::ARROWS_CLOCKWISE;
pub const POWER: &str = ph::POWER;
pub const SERVER: &str = ph::HARD_DRIVES;
pub const SHIELD: &str = ph::SHIELD_CHECK;
