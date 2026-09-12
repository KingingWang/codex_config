//! Small glue between a `config.toml` key and a form control.
//!
//! Every helper reads the current value from the document, renders a control and
//! writes straight back when the user changes it — so there is no separate
//! "apply" step to remember.

use egui::{RichText, Ui};
use toml_edit::Value as TomlValue;

use crate::app::App;
use crate::doc::schema::Choice;
use crate::doc::toml_ext::TomlPathExt;
use crate::ui::theme;
use crate::ui::widgets::{self, FieldSpec};

fn widget_id(path: &[&str], suffix: &str) -> String {
    format!("{}{}", path.join("."), suffix)
}

fn write_str(app: &mut App, path: &[&str], value: &str) {
    if let Some(doc) = &mut app.doc {
        if value.trim().is_empty() {
            doc.config.remove_at(path);
        } else {
            doc.config
                .set_value_at(path, TomlValue::from(value.trim().to_string()));
        }
    }
}

pub fn read_str(app: &App, path: &[&str]) -> String {
    app.doc
        .as_ref()
        .and_then(|doc| doc.config.str_at(path))
        .unwrap_or_default()
}

/// Free text setting (model_catalog_json, notify path, ...).
pub fn str_field(
    app: &mut App,
    ui: &mut Ui,
    path: &[&str],
    label: &str,
    help: &str,
    placeholder: &str,
    mono: bool,
) {
    let mut value = read_str(app, path);
    let id = widget_id(path, "-s");
    let changed = widgets::field(
        ui,
        FieldSpec::new(label, path.last().unwrap_or(&""), help),
        |ui| {
            if mono {
                widgets::mono_field(ui, &id, &mut value, placeholder)
            } else {
                widgets::text_field(ui, &id, &mut value, placeholder)
            }
        },
    );
    if changed {
        write_str(app, path, &value);
    }
}

/// Integer setting; empty field removes the key.
pub fn int_field(app: &mut App, ui: &mut Ui, path: &[&str], label: &str, help: &str) {
    let mut buffer = app
        .doc
        .as_ref()
        .and_then(|doc| doc.config.int_at(path))
        .map(|v| v.to_string())
        .unwrap_or_default();
    let id = widget_id(path, "-i");
    let changed = widgets::field(
        ui,
        FieldSpec::new(label, path.last().unwrap_or(&""), help),
        |ui| widgets::opt_int_field(ui, &id, &mut buffer),
    );
    if changed && let Some(doc) = &mut app.doc {
        match widgets::parse_opt_int(&buffer) {
            Some(value) => doc.config.set_value_at(path, TomlValue::from(value)),
            None => {
                doc.config.remove_at(path);
            }
        }
    }
    if !buffer.is_empty() && widgets::parse_opt_int(&buffer).is_none() {
        widgets::note(ui, "这里只能填数字，或者留空表示不写入配置。", theme::WARN);
    }
}

/// Enum setting rendered as a dropdown of `Choice`s.
pub fn choice_field(
    app: &mut App,
    ui: &mut Ui,
    path: &[&str],
    choices: &[Choice],
    label: &str,
    help: &str,
) {
    let mut value = read_str(app, path);
    let id = widget_id(path, "-c");
    let changed = widgets::field(
        ui,
        FieldSpec::new(label, path.last().unwrap_or(&""), help),
        |ui| widgets::choice_dropdown(ui, &id, &mut value, choices, true),
    );
    if changed {
        write_str(app, path, &value);
    }
}

/// Three state boolean: unset / true / false.
pub fn bool_field(app: &mut App, ui: &mut Ui, path: &[&str], label: &str, help: &str) {
    const BOOL_CHOICES: &[Choice] = &[
        Choice {
            value: "true",
            label: "开启 (true)",
            help: "明确写入 true。",
        },
        Choice {
            value: "false",
            label: "关闭 (false)",
            help: "明确写入 false。",
        },
    ];
    let mut value = app
        .doc
        .as_ref()
        .and_then(|doc| doc.config.bool_at(path))
        .map(|v| v.to_string())
        .unwrap_or_default();
    let id = widget_id(path, "-b");
    let changed = widgets::field(
        ui,
        FieldSpec::new(label, path.last().unwrap_or(&""), help),
        |ui| widgets::choice_dropdown(ui, &id, &mut value, BOOL_CHOICES, true),
    );
    if changed && let Some(doc) = &mut app.doc {
        match value.as_str() {
            "true" => doc.config.set_value_at(path, TomlValue::from(true)),
            "false" => doc.config.set_value_at(path, TomlValue::from(false)),
            _ => {
                doc.config.remove_at(path);
            }
        }
    }
}

/// A dropdown whose options come from the document (models, providers, profiles).
pub fn dynamic_choice_field(
    app: &mut App,
    ui: &mut Ui,
    path: &[&str],
    mut options: Vec<(String, String)>,
    label: &str,
    help: &str,
    placeholder: &str,
) {
    let mut value = read_str(app, path);
    let id = widget_id(path, "-d");
    if !options.iter().any(|(value, _)| value.is_empty()) {
        options.insert(0, (String::new(), placeholder.to_string()));
    }
    let changed = widgets::field(
        ui,
        FieldSpec::new(label, path.last().unwrap_or(&""), help),
        |ui| widgets::string_dropdown(ui, &id, &mut value, &options, placeholder),
    );
    if changed {
        write_str(app, path, &value);
    }
}

/// Dense tri-state row used by the advanced page (features, tui, notice, ...).
pub fn tri_row(app: &mut App, ui: &mut Ui, path: &[&str], label: &str, help: &str) {
    let mut value = app.doc.as_ref().and_then(|doc| doc.config.bool_at(path));
    let id = widget_id(path, "-t");
    let changed = widgets::compact_row(ui, label, path.last().unwrap_or(&""), |ui| {
        widgets::tri_state(ui, &id, &mut value)
    });
    if changed && let Some(doc) = &mut app.doc {
        match value {
            Some(true) => doc.config.set_value_at(path, TomlValue::from(true)),
            Some(false) => doc.config.set_value_at(path, TomlValue::from(false)),
            None => {
                doc.config.remove_at(path);
            }
        }
    }
    if !help.is_empty() {
        ui.label(RichText::new(help).size(11.5).color(theme::TEXT_MUTED));
    }
    ui.add_space(3.0);
}

/// String array stored in the config, edited as one entry per line.
pub fn lines_field(
    app: &mut App,
    ui: &mut Ui,
    path: &[&str],
    label: &str,
    help: &str,
    rows: usize,
) {
    let current: Vec<String> = app
        .doc
        .as_ref()
        .and_then(|doc| doc.config.str_array_at(path))
        .unwrap_or_default();
    let mut buffer = current.join("\n");
    let id = widget_id(path, "-l");
    let changed = widgets::field(
        ui,
        FieldSpec::new(label, path.last().unwrap_or(&""), help),
        |ui| widgets::multiline_field(ui, &id, &mut buffer, rows),
    );
    if changed && let Some(doc) = &mut app.doc {
        let items: Vec<String> = buffer
            .lines()
            .map(|line| line.trim().trim_matches(',').trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        if items.is_empty() {
            doc.config.remove_at(path);
        } else {
            doc.config
                .set_value_at(path, crate::doc::toml_ext::value_str_array(&items));
        }
    }
}
