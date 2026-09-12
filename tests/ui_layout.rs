//! Shared component geometry must remain usable independently of page layout.

use codex_config::ui::{theme, widgets};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

#[test]
fn status_badges_do_not_inherit_button_height() {
    let mut harness = Harness::new_ui_state(
        |ui, height| {
            ui.set_style(theme::style());
            ui.horizontal(|ui| {
                *height = widgets::badge(ui, "ACTIVE", theme::OK, theme::OK_WEAK).height();
                widgets::ghost_button(ui, "Action");
            });
        },
        0.0,
    );
    harness.run_steps(3);
    assert!(
        *harness.state() <= 24.0,
        "read-only badge is too tall: {}",
        harness.state()
    );
    assert!(harness.root().get_by_label("Action").rect().height() >= 32.0);
}

#[test]
fn dropdown_respects_a_narrow_form_column() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(200.0, 200.0))
        .build_ui_state(
            |ui, selected| {
                ui.set_style(theme::style());
                widgets::string_dropdown(
                    ui,
                    "narrow",
                    selected,
                    &[("model".into(), "A model".into())],
                    "Choose",
                );
            },
            String::from("model"),
        );
    harness.run_steps(3);
    let control = harness.root().get_by_value("A model").rect();
    assert!(
        control.right() <= 200.0,
        "dropdown exceeds its column: {control:?}"
    );
}
