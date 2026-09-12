//! 「源文件编辑」— edit the raw TOML / JSON when you know exactly what you want.

use egui::{Context, RichText, Ui};
use toml_edit::DocumentMut;

use crate::app::App;
use crate::doc::pretty_json;
use crate::ui::icons;
use crate::ui::theme;
use crate::ui::widgets;

pub fn show(app: &mut App, ui: &mut Ui, _ctx: &Context) {
    let Some(doc) = &app.doc else { return };
    let is_catalog = app.raw_tab_is_catalog;
    let current = if is_catalog { doc.catalog_text() } else { doc.config_text() };
    let path_text = if is_catalog {
        doc.catalog_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "（还没有模型目录文件）".to_string())
    } else {
        doc.config_path.display().to_string()
    };

    // Load the buffer the first time, or when the tab changes.
    let tab_key = format!("{}{}", if is_catalog { "catalog" } else { "config" }, path_text);
    if app.raw_source != tab_key {
        app.raw_source = tab_key;
        app.raw_buffer = current.clone();
        app.raw_origin = current.clone();
    }
    let origin = app.raw_origin.clone();

    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let config_selected = !app.raw_tab_is_catalog;
            if ui
                .selectable_label(config_selected, RichText::new("config.toml").size(13.0))
                .clicked()
            {
                app.raw_tab_is_catalog = false;
            }
            if ui
                .selectable_label(!config_selected, RichText::new("模型目录 JSON").size(13.0))
                .clicked()
            {
                app.raw_tab_is_catalog = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(path_text.clone())
                        .monospace()
                        .size(11.5)
                        .color(theme::TEXT_MUTED),
                );
            });
        });
        ui.add_space(6.0);
        widgets::note(
            ui,
            "这里是「专家模式」：直接改原始文件内容。改完点「应用到编辑器」，其它页面会立刻同步；\n点右上角「保存」才会真正写入磁盘（会自动备份）。",
            theme::TEXT_DIM,
        );
    });
    ui.add_space(8.0);

    if is_catalog && doc.catalog.is_none() {
        widgets::note(
            ui,
            "当前没有可用的模型目录（文件不存在或不是合法 JSON）。你可以直接把完整 JSON 粘进来，然后点「应用」。",
            theme::WARN,
        );
        ui.add_space(6.0);
    }

    if origin != current {
        widgets::note(
            ui,
            "文件内容已经在别的页面被改过了，这里的文本可能不是最新的。点「同步最新内容」再继续，避免覆盖掉刚才的修改。",
            theme::WARN,
        );
        ui.add_space(4.0);
        if widgets::ghost_button(ui, "\u{21BB} 同步最新内容").clicked() {
            app.raw_buffer = current.clone();
            app.raw_origin = current.clone();
        }
        ui.add_space(6.0);
    }

    let validity = if is_catalog {
        match serde_json::from_str::<serde_json::Value>(&app.raw_buffer) {
            Ok(_) => Ok(()),
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
    let height = (ui.available_height() - 150.0).max(200.0);

    let edit = egui::TextEdit::multiline(&mut app.raw_buffer)
        .font(egui::TextStyle::Monospace)
        .desired_width(f32::INFINITY)
        .desired_rows((height / 15.0).max(10.0) as usize)
        .lock_focus(true)
        .id(egui::Id::new(if is_catalog { "raw-catalog" } else { "raw-config" }));
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .max_height(height)
        .show(ui, |ui| {
            ui.add(edit);
        });

    ui.add_space(8.0);
    match &validity {
        Ok(()) => widgets::note(ui, &format!("{} 格式正确，可以应用。", icons::CHECK), theme::OK),
        Err(err) => widgets::note(ui, err, theme::DANGER),
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let can_apply = validity.is_ok() && app.raw_buffer != origin;
        ui.add_enabled_ui(can_apply, |ui| {
            if widgets::primary_button(ui, &format!("{} 应用到编辑器", icons::CHECK)).clicked() {
                let buffer = app.raw_buffer.clone();
                let result = if is_catalog {
                    if let Some(doc) = &mut app.doc {
                        doc.apply_catalog_text(&buffer)
                    } else {
                        Ok(())
                    }
                } else if let Some(doc) = &mut app.doc {
                    doc.apply_config_text(&buffer)
                } else {
                    Ok(())
                };
                match result {
                    Ok(()) => {
                        app.raw_origin = buffer;
                        if let Some(doc) = &mut app.doc {
                            doc.reload_catalog();
                        }
                        app.editing_model = None;
                        app.editing_provider = None;
                        app.provider_editor = None;
                        app.toast_info("已应用，其它页面已经同步");
                    }
                    Err(err) => app.toast_error(format!("应用失败：{err:#}")),
                }
            }
        });
        if widgets::ghost_button(ui, "\u{21BB} 放弃这里的修改").clicked() {
            app.raw_buffer = origin.clone();
        }
        if is_catalog && widgets::ghost_button(ui, "格式化 JSON").clicked() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&app.raw_buffer) {
                app.raw_buffer = pretty_json(&value);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{lines} 行 · {chars} 字符"))
                    .size(11.5)
                    .color(theme::TEXT_MUTED),
            );
        });
    });
    ui.add_space(20.0);
}
