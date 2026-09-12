//! Reusable UI building blocks. Everything that needs to *explain itself* to a
//! beginner lives here: labels always show the real config key plus a plain
//! language description.

use egui::{Align, Color32, CornerRadius, Id, Margin, RichText, Sense, Stroke, Ui, Vec2};

use crate::doc::schema::{Choice, WireApi, UNSET, choice_help, choice_label};
use crate::ui::icons;
use crate::ui::theme;

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------

pub fn page_header(ui: &mut Ui, icon: &str, title: &str, subtitle: &str) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        icon_tile(ui, icon, 38.0, 22.0, theme::ACCENT);
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.label(RichText::new(title).size(23.0).strong().color(theme::TEXT));
            ui.label(RichText::new(subtitle).size(13.0).color(theme::TEXT_DIM));
        });
    });
    ui.add_space(12.0);
}

/// A rounded tile with a single vector icon centred inside — used for page and
/// section headers so the chrome reads as a real product, not glyph soup.
pub fn icon_tile(ui: &mut Ui, glyph: &str, size: f32, icon_size: f32, accent: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same((size * 0.3) as u8),
        accent.gamma_multiply(0.16),
        Stroke::new(1.0, accent.gamma_multiply(0.5)),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(icon_size),
        accent,
    );
}

/// Backwards-compatible alias kept for the sidebar entry points.
pub fn icon_chip(ui: &mut Ui, glyph: &str, size: f32, icon_size: f32) {
    icon_tile(ui, glyph, size, icon_size, theme::ACCENT);
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    theme::card_frame().show(ui, add).inner
}

/// A card with an icon, a title and an optional subtitle.
pub fn section<R>(ui: &mut Ui, icon: &str, title: &str, subtitle: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            if !icon.is_empty() {
                icon_tile(ui, icon, 30.0, 17.0, theme::ACCENT);
                ui.add_space(8.0);
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(title).size(16.0).strong().color(theme::TEXT));
                if !subtitle.is_empty() {
                    ui.label(RichText::new(subtitle).size(12.5).color(theme::TEXT_DIM));
                }
            });
        });
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        add(ui)
    })
}

pub struct FieldSpec<'a> {
    pub label: &'a str,
    pub key: &'a str,
    pub help: &'a str,
}

impl<'a> FieldSpec<'a> {
    pub fn new(label: &'a str, key: &'a str, help: &'a str) -> Self {
        Self { label, key, help }
    }
}

/// The workhorse row: explanation on the left, control on the right.
pub fn field<R>(ui: &mut Ui, spec: FieldSpec<'_>, add: impl FnOnce(&mut Ui) -> R) -> R {
    let total = ui.available_width();
    let label_w = (total * 0.46).clamp(230.0, 430.0);
    let response = ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            wrap(ui);
            ui.set_width(label_w);
            ui.horizontal(|ui| {
                ui.label(RichText::new(spec.label).size(13.5).strong().color(theme::TEXT));
                if !spec.key.is_empty() {
                    code_chip(ui, spec.key);
                }
            });
            if !spec.help.is_empty() {
                ui.label(
                    RichText::new(spec.help)
                        .size(12.0)
                        .color(theme::TEXT_DIM)
                        ,
                );
            }
        });
        ui.vertical(|ui| {
            ui.set_min_width(240.0);
            ui.set_max_width((total - label_w - 24.0).max(200.0));
            add(ui)
        })
        .inner
    });
    ui.add_space(4.0);
    response.inner
}

/// Small monospace chip showing a real config key.
pub fn code_chip(ui: &mut Ui, key: &str) {
    let frame = egui::Frame::new()
        .fill(theme::INPUT_BG)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(Margin::symmetric(5, 1));
    frame
        .show(ui, |ui| {
            ui.label(RichText::new(key).monospace().size(11.0).color(theme::TEXT_MUTED));
        })
        .response
        .on_hover_text(format!("配置文件里的真实字段名：{key}"));
}

pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(12.0)
            .color(theme::TEXT_MUTED)
            ,
    );
}

pub fn note(ui: &mut Ui, text: &str, color: Color32) {
    let frame = egui::Frame::new()
        .fill(color.gamma_multiply(0.13))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.45)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(text).size(12.5).color(color));
    });
}

pub fn badge(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) {
    let frame = egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, fg.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(20))
        .inner_margin(Margin::symmetric(8, 2));
    frame.show(ui, |ui| {
        ui.label(RichText::new(text).size(11.5).strong().color(fg));
    });
}

/// A tiny rounded pill holding a number, used for sidebar counts.
pub fn count_pill(ui: &mut Ui, count: usize, fg: Color32, bg: Color32) {
    let frame = egui::Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::symmetric(7, 1));
    frame.show(ui, |ui| {
        ui.label(RichText::new(count.to_string()).size(11.0).strong().color(fg));
    });
}

/// Key/value read-only summary row.
pub fn kv_row(ui: &mut Ui, key: &str, value: &str) {
    ui.horizontal_top(|ui| {
        ui.label(RichText::new(key).size(12.5).color(theme::TEXT_MUTED));
        ui.label(
            RichText::new(value)
                .size(12.5)
                .monospace()
                .color(theme::TEXT_DIM)
                ,
        );
    });
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

/// Single line text field with a placeholder. Returns true when changed.
pub fn text_field(ui: &mut Ui, id: &str, value: &mut String, placeholder: &str) -> bool {
    let edit = egui::TextEdit::singleline(value)
        .hint_text(RichText::new(placeholder).color(theme::TEXT_MUTED).size(13.0))
        .vertical_align(Align::Center)
        .id(Id::new(id));
    ui.add_sized(Vec2::new(ui.available_width().max(140.0), 32.0), edit)
        .changed()
}

pub fn mono_field(ui: &mut Ui, id: &str, value: &mut String, placeholder: &str) -> bool {
    let edit = egui::TextEdit::singleline(value)
        .font(egui::TextStyle::Monospace)
        .hint_text(RichText::new(placeholder).color(theme::TEXT_MUTED).size(13.0))
        .id(Id::new(id))
        .vertical_align(Align::Center);
    ui.add_sized(Vec2::new(ui.available_width().max(140.0), 32.0), edit)
        .changed()
}

/// Text field with an explicit width — use inside `horizontal` rows so the row
/// cannot grow past its container (Areas size themselves to their content).
pub fn text_field_w(ui: &mut Ui, id: &str, value: &mut String, placeholder: &str, width: f32) -> bool {
    let edit = egui::TextEdit::singleline(value)
        .hint_text(RichText::new(placeholder).color(theme::TEXT_MUTED).size(13.0))
        .vertical_align(Align::Center)
        .id(Id::new(id));
    ui.add_sized(Vec2::new(width, 32.0), edit).changed()
}

pub fn password_field(ui: &mut Ui, id: &str, value: &mut String, placeholder: &str) -> bool {
    let edit = egui::TextEdit::singleline(value)
        .password(true)
        .font(egui::TextStyle::Monospace)
        .hint_text(RichText::new(placeholder).color(theme::TEXT_MUTED).size(13.0))
        .id(Id::new(id))
        .vertical_align(Align::Center);
    ui.add_sized(Vec2::new(ui.available_width().max(140.0), 32.0), edit)
        .changed()
}

pub fn multiline_field(ui: &mut Ui, id: &str, value: &mut String, rows: usize) -> bool {
    let edit = egui::TextEdit::multiline(value)
        .id(Id::new(id))
        .desired_rows(rows)
        .font(egui::TextStyle::Monospace);
    ui.add(edit).changed()
}

pub fn opt_int_field(ui: &mut Ui, id: &str, buffer: &mut String) -> bool {
    mono_field(ui, id, buffer, "留空 = 不写入配置")
}

pub fn parse_opt_int(buffer: &str) -> Option<i64> {
    let cleaned: String = buffer
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_')
        .collect();
    if cleaned.is_empty() {
        None
    } else {
        cleaned.parse::<i64>().ok()
    }
}

/// Dropdown over a `Choice` list, with an optional "未设置" entry.
/// `current` is the raw config value; an empty string means unset.
pub fn choice_dropdown(
    ui: &mut Ui,
    id: &str,
    current: &mut String,
    choices: &[Choice],
    allow_unset: bool,
) -> bool {
    let selected = if current.is_empty() {
        UNSET.to_string()
    } else {
        choice_label(choices, current)
    };
    let mut changed = false;
    egui::ComboBox::from_id_salt(Id::new(id))
        .selected_text(RichText::new(selected).size(13.5).color(theme::TEXT))
        .width(ui.available_width().max(220.0))
        .show_ui(ui, |ui| {
            if allow_unset {
                let is_selected = current.is_empty();
                let text = RichText::new(UNSET)
                    .size(13.0)
                    .color(if is_selected { theme::ACCENT_TEXT } else { theme::TEXT_DIM });
                if ui.selectable_label(is_selected, text).clicked() {
                    *current = String::new();
                    changed = true;
                }
                ui.separator();
            }
            for choice in choices {
                let is_selected = current == choice.value;
                let label = RichText::new(choice.label)
                    .size(13.0)
                    .color(if is_selected { theme::ACCENT_TEXT } else { theme::TEXT });
                if ui
                    .selectable_label(is_selected, label)
                    .on_hover_text(choice.help)
                    .clicked()
                {
                    *current = choice.value.to_string();
                    changed = true;
                }
            }
        });
    if !current.is_empty() {
        let help = choice_help(choices, current);
        if !help.is_empty() {
            ui.label(
                RichText::new(help)
                    .size(11.5)
                    .color(theme::TEXT_MUTED)
                    ,
            );
        }
    }
    changed
}

/// Dropdown over arbitrary strings (model slugs, provider ids, ...).
pub fn string_dropdown(
    ui: &mut Ui,
    id: &str,
    current: &mut String,
    options: &[(String, String)],
    placeholder: &str,
) -> bool {
    let mut changed = false;
    let selected = if current.is_empty() {
        placeholder.to_string()
    } else {
        options
            .iter()
            .find(|(value, _)| value == current)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| current.clone())
    };
    egui::ComboBox::from_id_salt(Id::new(id))
        .selected_text(RichText::new(selected).size(13.5).color(theme::TEXT))
        .width(ui.available_width().max(220.0))
        .show_ui(ui, |ui| {
            for (value, label) in options {
                let is_selected = *value == *current;
                if ui
                    .selectable_label(
                        is_selected,
                        RichText::new(label.clone())
                            .size(13.0)
                            .color(if is_selected { theme::ACCENT_TEXT } else { theme::TEXT }),
                    )
                    .clicked()
                {
                    *current = value.clone();
                    changed = true;
                }
            }
        });
    changed
}

/// Big three-way selector for the wire protocol.
pub fn protocol_picker(ui: &mut Ui, current: &mut WireApi) -> bool {
    let mut changed = false;
    let width = ((ui.available_width() - 24.0) / 3.0).clamp(150.0, 280.0);
    ui.horizontal_top(|ui| {
        for option in WireApi::ALL {
            let selected = *current == option;
            let (fill, stroke) = if selected {
                (theme::ACCENT_WEAK, Stroke::new(1.6, theme::ACCENT))
            } else {
                (theme::INPUT_BG, Stroke::new(1.0, theme::BORDER))
            };
            let frame = egui::Frame::new()
                .fill(fill)
                .stroke(stroke)
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(12));
            let response = frame
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        wrap(ui);
                        ui.set_width(width - 24.0);
                        ui.label(
                            RichText::new(option.label())
                                .size(13.5)
                                .strong()
                                .color(if selected { theme::ACCENT_TEXT } else { theme::TEXT }),
                        );
                        ui.label(
                            RichText::new(option.endpoint())
                                .monospace()
                                .size(10.5)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(option.help())
                                .size(11.5)
                                .color(if selected { theme::ACCENT_TEXT } else { theme::TEXT_DIM }),
                        );
                    });
                })
.response
                .interact(Sense::click());
            if response.clicked() && !selected {
                *current = option;
                changed = true;
            }
        }
    });
    changed
}

/// Multi select chips, used for input modalities and reasoning levels.
pub fn chip_toggles(
    ui: &mut Ui,
    options: &[(String, String, String)],
    selected: &mut Vec<String>,
) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for (value, label, help) in options {
            let is_selected = selected.iter().any(|s| s == value);
            let (fill, stroke, fg) = if is_selected {
                (theme::ACCENT_WEAK, Stroke::new(1.4, theme::ACCENT), theme::ACCENT_TEXT)
            } else {
                (theme::INPUT_BG, Stroke::new(1.0, theme::BORDER), theme::TEXT_DIM)
            };
            let text = if is_selected {
                format!("{} {label}", icons::CHECK)
            } else {
                label.clone()
            };
            let frame = egui::Frame::new()
                .fill(fill)
                .stroke(stroke)
                .corner_radius(CornerRadius::same(16))
                .inner_margin(Margin::symmetric(10, 4));
            let response = frame
                .show(ui, |ui| {
                    ui.label(RichText::new(text).size(12.5).color(fg));
                })
                .response
                .interact(Sense::click())
                .on_hover_text(help.clone());
            if response.clicked() {
                if is_selected {
                    selected.retain(|s| s != value);
                } else {
                    selected.push(value.clone());
                }
                changed = true;
            }
        }
    });
    changed
}

/// Editable key/value table (HTTP headers, query params, env vars).
pub fn kv_editor(
    ui: &mut Ui,
    id: &str,
    entries: &mut Vec<(String, String)>,
    key_hint: &str,
    value_hint: &str,
) -> bool {
    let mut changed = false;
    let mut to_remove: Option<usize> = None;
    for (index, entry) in entries.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let key_resp = ui.add_sized(
                Vec2::new(190.0, 30.0),
                egui::TextEdit::singleline(&mut entry.0)
                    .hint_text(key_hint)
                    .font(egui::TextStyle::Monospace)
                    .id(Id::new((id, "k", index))),
            );
            let value_resp = ui.add_sized(
                Vec2::new((ui.available_width() - 40.0).max(120.0), 30.0),
                egui::TextEdit::singleline(&mut entry.1)
                    .hint_text(value_hint)
                    .font(egui::TextStyle::Monospace)
                    .id(Id::new((id, "v", index))),
            );
            if key_resp.changed() || value_resp.changed() {
                changed = true;
            }
            if icon_button(ui, icons::DELETE, theme::DANGER, "删除这一项").clicked() {
                to_remove = Some(index);
            }
        });
    }
    if let Some(index) = to_remove {
        entries.remove(index);
        changed = true;
    }
    if ghost_button(ui, &format!("{} 添加一项", icons::ADD)).clicked() {
        entries.push((String::new(), String::new()));
        changed = true;
    }
    changed
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

pub fn primary_button(ui: &mut Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(RichText::new(text).size(13.5).strong().color(Color32::WHITE))
        .fill(theme::ACCENT)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(8))
        .min_size(Vec2::new(0.0, 32.0));
    ui.add(button)
}

pub fn ghost_button(ui: &mut Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(RichText::new(text).size(13.0).color(theme::TEXT))
        .fill(theme::CARD_ALT)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(CornerRadius::same(8))
        .min_size(Vec2::new(0.0, 30.0));
    ui.add(button)
}

pub fn danger_button(ui: &mut Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(RichText::new(text).size(13.0).strong().color(theme::DANGER))
        .fill(theme::DANGER_WEAK)
        .stroke(Stroke::new(1.0, theme::DANGER.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(8))
        .min_size(Vec2::new(0.0, 30.0));
    ui.add(button)
}

pub fn link_button(ui: &mut Ui, text: &str, color: Color32) -> egui::Response {
    let button = egui::Button::new(RichText::new(text).size(12.0).color(color))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(6))
        .min_size(Vec2::new(0.0, 22.0));
    ui.add(button)
}

pub fn icon_button(ui: &mut Ui, icon: &str, color: Color32, tooltip: &str) -> egui::Response {
    let button = egui::Button::new(RichText::new(icon).size(13.0).color(color))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(6))
        .min_size(Vec2::new(26.0, 26.0));
    ui.add(button).on_hover_text(tooltip.to_string())
}

/// A full width frame that behaves like a button; used in list rows.
pub fn clickable_frame<R>(ui: &mut Ui, selected: bool, add: impl FnOnce(&mut Ui) -> R) -> (egui::Response, R) {
    let (fill, stroke) = if selected {
        (theme::ACCENT_WEAK, Stroke::new(1.2, theme::ACCENT.gamma_multiply(0.7)))
    } else {
        (theme::INPUT_BG, Stroke::new(1.0, theme::BORDER))
    };
    let frame = egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12));
    let mut inner = None;
    let response = frame
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            inner = Some(add(ui));
        })
        .response;
    (response.interact(Sense::click()), inner.unwrap())
}

pub fn empty_state(ui: &mut Ui, icon: &str, title: &str, body: &str) {
    ui.add_space(30.0);
    ui.vertical_centered(|ui| {
        wrap(ui);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(58.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 29.0, theme::ACCENT.gamma_multiply(0.14));
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(30.0),
            theme::ACCENT_HI,
        );
        ui.add_space(10.0);
        ui.label(RichText::new(title).size(17.0).strong().color(theme::TEXT));
        ui.add_space(4.0);
        ui.set_max_width(520.0);
        ui.label(
            RichText::new(body)
                .size(13.0)
                .color(theme::TEXT_DIM)
                ,
        );
    });
    ui.add_space(14.0);
}

pub fn search_field(ui: &mut Ui, query: &mut String) -> bool {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icons::SEARCH).size(15.0).color(theme::TEXT_MUTED));
        ui.add_sized(
            Vec2::new(ui.available_width().max(160.0), 32.0),
            egui::TextEdit::singleline(query)
                .hint_text(RichText::new("搜索…").color(theme::TEXT_MUTED).size(13.0))
                .id(Id::new("search-box")),
        )
        .changed()
    })
    .inner
}

/// Simple centred modal dialog. Returns the closure result while open.
pub fn modal<R>(ctx: &egui::Context, id: &str, title: &str, width: f32, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let mut result = None;
    egui::Modal::new(Id::new(id)).show(ctx, |ui| {
        wrap(ui);
        ui.set_width(width);
        ui.label(RichText::new(title).size(17.0).strong().color(theme::TEXT));
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(10.0);
        result = Some(add(ui));
    });
    result
}

/// Compact three state switch: 默认 / 开 / 关.
/// `None` means "not written to the config file" (Codex uses its own default).
pub fn tri_state(ui: &mut Ui, _id: &str, value: &mut Option<bool>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (label, option, help) in [
            ("默认", None, "不写进配置文件，使用 Codex 自己的默认值"),
            ("开", Some(true), "写入 true"),
            ("关", Some(false), "写入 false"),
        ] {
            let selected = *value == option;
            let (fill, stroke, fg) = if selected {
                match option {
                    Some(true) => (theme::OK_WEAK, Stroke::new(1.2, theme::OK), theme::OK),
                    Some(false) => (theme::DANGER_WEAK, Stroke::new(1.2, theme::DANGER.gamma_multiply(0.7)), theme::DANGER),
                    None => (theme::CARD_ALT, Stroke::new(1.2, theme::BORDER_STRONG), theme::TEXT),
                }
            } else {
                (theme::INPUT_BG, Stroke::new(1.0, theme::BORDER), theme::TEXT_MUTED)
            };
            let frame = egui::Frame::new()
                .fill(fill)
                .stroke(stroke)
                .corner_radius(CornerRadius::same(7))
                .inner_margin(Margin::symmetric(11, 3));
            let response = frame
                .show(ui, |ui| {
                    ui.label(RichText::new(label).size(12.0).color(fg));
                })
                .response
                .interact(Sense::click())
                .on_hover_text(help);
            if response.clicked() && !selected {
                *value = option;
                changed = true;
            }
        }
    });
    changed
}

/// A dense row: label + optional key chip on the left, a control on the right.
pub fn compact_row<R>(ui: &mut Ui, label: &str, key: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let total = ui.available_width();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width((total * 0.5).clamp(180.0, 380.0));
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).size(13.0).color(theme::TEXT));
                if !key.is_empty() {
                    code_chip(ui, key);
                }
            });
        });
        ui.vertical(|ui| add(ui)).inner
    })
    .inner
}

/// Force text wrapping inside this `Ui`. Needed wherever the layout would
/// otherwise give labels an unbounded width (rows, areas, modals).
pub fn wrap(ui: &mut Ui) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
}
