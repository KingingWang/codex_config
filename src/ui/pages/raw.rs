//! 「源文件编辑」— edit the raw TOML / JSON when you know exactly what you want.

use egui::{Context, Margin, RichText, Ui};
use toml_edit::DocumentMut;

use crate::app::App;
use crate::doc::pretty_json;
use crate::ui::icons;
use crate::ui::theme;
use crate::ui::widgets;

pub fn show(app: &mut App, ui: &mut Ui, _ctx: &Context) {
    let Some(doc) = &app.doc else { return };
    let is_catalog = app.raw_tab_is_catalog;
    let current = if is_catalog {
        doc.catalog_text()
    } else {
        doc.config_text()
    };
    let path_text = if is_catalog {
        doc.catalog_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "（还没有模型目录文件）".to_string())
    } else {
        doc.config_path.display().to_string()
    };

    // Load the buffer the first time, or when the tab changes.
    let tab_key = format!(
        "{}{}",
        if is_catalog { "catalog" } else { "config" },
        path_text
    );
    if app.raw_source != tab_key {
        app.raw_source = tab_key;
        app.raw_buffer = current.clone();
        app.raw_origin = current.clone();
    }
    let origin = app.raw_origin.clone();
    let has_draft = app.raw_buffer != origin;

    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let config_selected = !app.raw_tab_is_catalog;
            if ui
                .add_enabled(
                    config_selected || !has_draft,
                    egui::Button::selectable(config_selected, "config.toml"),
                )
                .on_hover_text("切换前请先应用或放弃当前草稿")
                .clicked()
            {
                app.raw_tab_is_catalog = false;
            }
            if ui
                .add_enabled(
                    !config_selected || !has_draft,
                    egui::Button::selectable(!config_selected, "模型目录 JSON"),
                )
                .on_hover_text("切换前请先应用或放弃当前草稿")
                .clicked()
            {
                app.raw_tab_is_catalog = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(&path_text)
                            .monospace()
                            .size(11.0)
                            .color(theme::TEXT_MUTED),
                    )
                    .truncate(),
                )
                .on_hover_text(&path_text);
            });
        });

        ui.add_space(6.0);

        widgets::hint(
            ui,
            "直接编辑配置文件。改完先「应用到编辑器」，再点右上角「保存配置」。",
        );
    });
    ui.add_space(6.0);

    if origin != current {
        widgets::note(
            ui,
            "文件内容已在其他页面修改。请先「同步最新内容」再继续，避免覆盖。",
            theme::WARN,
        );
        ui.add_space(4.0);
        if widgets::ghost_button(ui, "\u{21BB} 同步最新内容").clicked() {
            app.raw_buffer = current.clone();
            app.raw_origin = current.clone();
        }
        ui.add_space(4.0);
    }

    if is_catalog && doc.catalog.is_none() {
        widgets::note(
            ui,
            "当前没有可用的模型目录。可以直接粘贴完整 JSON。",
            theme::WARN,
        );
        ui.add_space(4.0);
    }

    let validity = if is_catalog {
        match serde_json::from_str::<serde_json::Value>(&app.raw_buffer) {
            Ok(value) if value.get("models").is_some_and(serde_json::Value::is_array) => Ok(()),
            Ok(_) => Err("模型目录需要包含 models 数组".to_string()),
            Err(err) => Err(format!("JSON 解析失败：{err}")),
        }
    } else {
        match app.raw_buffer.parse::<DocumentMut>() {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("TOML 解析失败：{err}")),
        }
    };

    let lines = app.raw_buffer.lines().count();
    let chars = app.raw_buffer.chars().count();
    let footer_height = if validity.is_err() { 174.0 } else { 104.0 };
    let height = (ui.available_height() - footer_height).max(120.0);

    // Reuse the existing memoized highlighter. Its built-in fallback supports
    // TOML; JSON remains plain text without adding a syntax dependency.
    let code_theme = egui_extras::syntax_highlighting::CodeTheme::from_memory(ui.ctx(), ui.style());
    let language = if is_catalog { "json" } else { "toml" };
    let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
        let mut job = egui_extras::syntax_highlighting::highlight(
            ui.ctx(),
            ui.style(),
            &code_theme,
            text.as_str(),
            language,
        );
        job.wrap.max_width = wrap_width;
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    };
    let edit = egui::TextEdit::multiline(&mut app.raw_buffer)
        .font(egui::TextStyle::Monospace)
        .desired_width(f32::INFINITY)
        .desired_rows((height / 15.0).max(10.0) as usize)
        .lock_focus(true)
        .margin(Margin::same(12))
        .layouter(&mut layouter)
        .id(egui::Id::new(if is_catalog {
            "raw-catalog"
        } else {
            "raw-config"
        }));

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .max_height(height)
        .show(ui, |ui| {
            ui.add(edit);
        });

    ui.add_space(6.0);

    if let Err(err) = &validity {
        egui::ScrollArea::vertical()
            .id_salt("raw-validation-error")
            .max_height(60.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                widgets::note(ui, err, theme::DANGER);
            });
        ui.add_space(4.0);
    }

    ui.horizontal_wrapped(|ui| {
        if validity.is_ok() {
            ui.label(
                RichText::new(format!("{} 格式正确", icons::CHECK))
                    .size(12.0)
                    .color(theme::OK),
            );
        }

        ui.label(
            RichText::new(format!("{lines} 行 · {chars} 字符"))
                .size(11.0)
                .color(theme::TEXT_MUTED),
        );
    });
    ui.horizontal_wrapped(|ui| {
        let can_apply = validity.is_ok() && app.raw_buffer != origin && origin == current;
        ui.add_enabled_ui(can_apply, |ui| {
            if widgets::primary_button(ui, &format!("{} 应用到编辑器", icons::CHECK)).clicked()
            {
                app.apply_raw_source();
            }
        });
        if widgets::ghost_button(ui, "\u{21BB} 放弃这里的修改").clicked() {
            app.raw_buffer = origin.clone();
        }
        if is_catalog
            && widgets::ghost_button(ui, "格式化 JSON").clicked()
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&app.raw_buffer)
        {
            app.raw_buffer = pretty_json(&value);
        }
    });

    ui.add_space(12.0);
}
