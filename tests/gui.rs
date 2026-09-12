//! GUI tests: the app is rendered off-screen with `egui_kittest`, every page is
//! written out as a PNG (see `screenshots/`) and real interactions are simulated.

use std::fs;
use std::path::{Path, PathBuf};

use codex_config::App;
use codex_config::app::Dialog;
use codex_config::doc::catalog;
use codex_config::doc::toml_ext::TomlPathExt;
use codex_config::doc::validate::{Severity, validate};
use codex_config::page::Page;
use egui_kittest::Harness;
use egui_kittest::kittest::NodeT;
use egui_kittest::kittest::Queryable;

fn fixture_home() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex-home");
    let home = dir.path();
    for entry in fs::read_dir(&source).expect("fixture dir") {
        let entry = entry.expect("dir entry");
        fs::copy(entry.path(), home.join(entry.file_name())).expect("copy fixture");
    }
    dir
}

fn screenshot_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("screenshots");
    fs::create_dir_all(&dir).expect("create screenshots dir");
    dir
}

fn app_harness() -> (Harness<'static, App>, tempfile::TempDir) {
    let home = fixture_home();
    let target = home.path().to_path_buf();
    let mut harness = Harness::builder()
        .with_theme(egui::Theme::Light)
        .with_size(egui::vec2(1440.0, 940.0))
        .with_pixels_per_point(2.0)
        .build_eframe(move |cc| App::with_home(cc, target));
    harness.run_steps(3);
    (harness, home)
}

fn render(harness: &mut Harness<'static, App>, name: &str) {
    harness.state_mut().toasts = egui_notify::Toasts::new();
    harness.run_steps(2);
    let image = harness
        .render()
        .unwrap_or_else(|err| panic!("render {name} failed: {err}"));
    assert!(image.width() > 800, "{name} should be a full window render");
    let path = screenshot_dir().join(format!("{name}.png"));
    image
        .save(&path)
        .unwrap_or_else(|err| panic!("save {name}: {err}"));
    println!("wrote {}", path.display());
}

/// Asserts the render is not a flat colour block, i.e. something was drawn.
fn assert_not_blank(harness: &mut Harness<'static, App>, name: &str) {
    let image = harness.render().expect("render");
    let first = image.get_pixel(0, 0);
    let mut different = 0usize;
    for x in (0..image.width()).step_by(7) {
        for y in (0..image.height()).step_by(7) {
            if image.get_pixel(x, y) != first {
                different += 1;
            }
        }
    }
    assert!(
        different > 200,
        "{name} looks blank ({different} differing samples)"
    );
}

#[test]
fn every_page_renders() {
    let (mut harness, _home) = app_harness();
    for page in Page::ALL {
        harness.state_mut().page = page;
        harness.state_mut().raw_source.clear();
        harness.run_steps(3);
        let name = format!("page-{:?}", page).to_lowercase();
        assert_not_blank(&mut harness, &name);
        render(&mut harness, &name);
    }
}

#[test]
fn clicking_the_sidebar_switches_pages() {
    let (mut harness, _home) = app_harness();
    let button = harness
        .root()
        .query_by_label("模型管理")
        .expect("sidebar entry for 模型管理");
    button.click();
    harness.run_steps(2);
    assert_eq!(harness.state().page, Page::Models);
    render(&mut harness, "interaction-nav-to-models");
}

#[test]
fn selecting_a_model_opens_its_editor() {
    let (mut harness, _home) = app_harness();
    harness.state_mut().page = Page::Models;
    harness.run_steps(2);
    let row = harness
        .root()
        .query_by_label_contains("Claude Opus 5")
        .expect("model row");
    row.click();
    harness.run_steps(3);
    let app = harness.state();
    assert_eq!(app.editing_model, Some(1));
    assert_eq!(app.model_editor.slug, "claude-opus-5");
    assert_eq!(app.model_editor.context_window, "200000");
    render(&mut harness, "interaction-model-selected");
}

#[test]
fn editing_a_model_writes_through_to_the_catalog() {
    let (mut harness, home) = app_harness();
    harness.state_mut().page = Page::Models;
    harness.run_steps(2);
    harness.state_mut().select_model(0);
    harness.run_steps(2);

    // Simulate typing into the display name text input of the editor.
    let field = harness
        .root()
        .children_recursive()
        .find(|node| {
            let ak = node.accesskit_node();
            format!("{:?}", ak.role()) == "TextInput"
                && ak.value().unwrap_or_default() == "GPT-5.2 Codex"
        })
        .expect("display name text input");
    field.focus();
    field.type_text(" 加强版");
    harness.run_steps(2);

    let app = harness.state();
    let catalog = app.doc.as_ref().unwrap().catalog.as_ref().unwrap();
    let model = &catalog::models(catalog).unwrap()[0];
    assert_eq!(
        model
            .get("display_name")
            .and_then(serde_json::Value::as_str),
        Some("GPT-5.2 Codex 加强版"),
        "typing in the GUI should update the in-memory catalog"
    );
    assert!(app.doc.as_ref().unwrap().catalog_dirty());
    let _ = home;
    render(&mut harness, "interaction-model-edited");
}

#[test]
fn adding_a_provider_from_a_template_updates_config() {
    let (mut harness, _home) = app_harness();
    harness.state_mut().page = Page::Providers;
    harness.state_mut().dialog = Some(Dialog::NewProvider);
    harness.run_steps(3);
    render(&mut harness, "dialog-new-provider");

    let template = harness
        .root()
        .query_by_label("OpenAI 官方")
        .expect("template card");
    template.click();
    harness.run_steps(3);

    let app = harness.state();
    assert!(
        app.dialog.is_none(),
        "dialog should close after picking a template"
    );
    let config = &app.doc.as_ref().unwrap().config;
    assert_eq!(
        config
            .str_at(&["model_providers", "openai", "wire_api"])
            .as_deref(),
        Some("responses")
    );
    assert_eq!(
        config
            .str_at(&["model_providers", "openai", "env_key"])
            .as_deref(),
        Some("OPENAI_API_KEY")
    );
    assert_eq!(app.editing_provider.as_deref(), Some("openai"));
    render(&mut harness, "interaction-provider-added");
}

#[test]
fn validation_reports_the_broken_model() {
    let (harness, _home) = app_harness();
    let doc = harness.state().doc.as_ref().expect("document");
    let issues = validate(doc);
    assert!(
        issues
            .iter()
            .any(|issue| issue.detail.contains("does-not-exist")
                && issue.severity == Severity::Warning),
        "expected a warning about the missing provider, got {issues:?}"
    );
}

#[test]
fn saving_keeps_comments_and_creates_a_backup() {
    let (mut harness, home) = app_harness();
    harness.state_mut().page = Page::Overview;
    harness.run_steps(2);

    // Change something through the public API, the way a page would.
    {
        let app = harness.state_mut();
        let doc = app.doc.as_mut().unwrap();
        doc.config
            .set_value_at(&["model_reasoning_effort"], toml_edit::Value::from("xhigh"));
    }
    harness.run_steps(2);
    render(&mut harness, "interaction-before-save");

    harness.state_mut().save();
    harness.run_steps(2);

    let written = fs::read_to_string(home.path().join("config.toml")).expect("config");
    assert!(
        written.contains("# 示例配置"),
        "comments must survive a save"
    );
    assert!(written.contains("model_reasoning_effort = \"xhigh\""));
    assert!(written.contains("[model_providers.relay]"));

    let backups: Vec<_> = fs::read_dir(home.path())
        .expect("read home")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.contains(".bak-"))
        .collect();
    assert!(!backups.is_empty(), "a backup should have been written");
    assert!(!harness.state().doc.as_ref().unwrap().dirty());
    render(&mut harness, "interaction-after-save");
}

/// Smoke test against the developer's real CODEX_HOME when present: the app must
/// load a large, messy, real-world config without panicking.
#[test]
#[ignore = "opt-in only: normal tests must not inspect personal configuration"]
fn loads_a_real_codex_home_if_present() {
    let home = codex_config::doc::Document::default_home();
    if !home.join("config.toml").exists() {
        eprintln!("skipping: no real CODEX_HOME at {}", home.display());
        return;
    }
    let mut harness = Harness::builder()
        .with_theme(egui::Theme::Light)
        .with_size(egui::vec2(1440.0, 940.0))
        .build_eframe(move |cc| App::with_home(cc, home));
    harness.run_steps(3);
    let doc = harness.state().doc.as_ref().expect("document loaded");
    assert_eq!(doc.config_path.file_name().unwrap(), "config.toml");
    let model_count = doc.catalog.as_ref().map(catalog::model_count).unwrap_or(0);
    eprintln!(
        "real home loaded: {} providers, {model_count} models, {} issues",
        doc.provider_ids().len(),
        codex_config::doc::validate::validate(doc).len()
    );
    for page in Page::ALL {
        harness.state_mut().page = page;
        harness.state_mut().raw_source.clear();
        harness.run_steps(2);
        let image = harness.render().expect("render real home page");
        assert!(image.width() > 800);
    }
}

#[test]
fn diff_window_shows_the_pending_change() {
    let (mut harness, _home) = app_harness();
    {
        let app = harness.state_mut();
        let doc = app.doc.as_mut().unwrap();
        doc.config
            .set_value_at(&["sandbox_mode"], toml_edit::Value::from("read-only"));
    }
    harness.state_mut().show_diff = true;
    harness.run_steps(3);
    render(&mut harness, "dialog-diff");
    assert!(harness.state().show_diff);
}

#[test]
fn restart_dialog_renders_with_synthetic_instances() {
    use codex_config::server::{Host, ServerInstance};
    let (mut harness, _home) = app_harness();
    harness.state_mut().restart_scan = vec![
        ServerInstance {
            pid: 53872,
            shell_pid: Some(53867),
            host: Host::ChatGptDesktop,
            cmd_summary: "codex … app-server".to_string(),
            is_our_host: true,
        },
        ServerInstance {
            pid: 58788,
            shell_pid: Some(58787),
            host: Host::Zed,
            cmd_summary: "codex … app-server".to_string(),
            is_our_host: false,
        },
    ];
    harness.state_mut().restart_selected = vec![false, true];
    harness.state_mut().restart_scanned = true;
    harness.state_mut().dialog = Some(Dialog::RestartServers);
    harness.run_steps(3);
    render(&mut harness, "dialog-restart-servers");

    // The protected (current-session) instance must never be selected.
    assert!(!harness.state().restart_selected[0]);
}

#[test]
fn restart_refuses_to_kill_the_current_session_host() {
    use codex_config::server::{Host, ServerInstance, restart_instance};
    let protected = ServerInstance {
        pid: std::process::id(),
        shell_pid: None,
        host: Host::ChatGptDesktop,
        cmd_summary: "self".to_string(),
        is_our_host: true,
    };
    let outcome = restart_instance(&protected);
    assert!(!outcome.ok, "must refuse to kill the protected instance");
    assert!(outcome.message.contains("当前会话"));
}

fn click(harness: &mut Harness<'static, App>, label: &str) {
    harness.root().get_by_label(label).scroll_to_me();
    harness.run_steps(3);
    harness.root().get_by_label(label).click();
    harness.run_steps(3);
}

fn change_reasoning(harness: &mut Harness<'static, App>) {
    harness
        .state_mut()
        .doc
        .as_mut()
        .unwrap()
        .config
        .set_value_at(&["model_reasoning_effort"], toml_edit::Value::from("xhigh"));
    harness.run_steps(2);
}

#[test]
fn browsing_every_page_does_not_change_configuration() {
    let (mut harness, home) = app_harness();
    let before = fs::read_to_string(home.path().join("config.toml")).unwrap();
    for page in Page::ALL {
        harness.state_mut().page = page;
        harness.run_steps(3);
        assert!(
            !harness.state().has_unsaved_changes(),
            "visiting {page:?} changed settings"
        );
    }
    assert_eq!(
        before,
        fs::read_to_string(home.path().join("config.toml")).unwrap()
    );
    assert!(
        harness
            .root()
            .get_by_label("保存配置")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn every_navigation_button_is_keyboard_accessible() {
    let (mut harness, _home) = app_harness();
    for page in [
        Page::Models,
        Page::Providers,
        Page::Profiles,
        Page::Advanced,
        Page::Raw,
        Page::Overview,
    ] {
        harness.root().get_by_label(page.title()).focus();
        harness.key_press(egui::Key::Space);
        harness.run_steps(3);
        assert_eq!(harness.state().page, page);
    }
}

#[test]
fn save_button_writes_changes_and_shows_restart_reminder() {
    let (mut harness, home) = app_harness();
    change_reasoning(&mut harness);
    click(&mut harness, "保存配置");
    assert!(!harness.state().has_unsaved_changes());
    assert!(harness.state().saved_needs_restart);
    assert!(
        fs::read_to_string(home.path().join("config.toml"))
            .unwrap()
            .contains("xhigh")
    );
    assert!(
        harness
            .root()
            .query_by_label_contains("重启 Codex 后使用新配置")
            .is_some()
    );
}

#[test]
fn command_s_saves_without_opening_a_dialog() {
    let (mut harness, _home) = app_harness();
    change_reasoning(&mut harness);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.run_steps(3);
    assert!(!harness.state().has_unsaved_changes());
    assert!(harness.state().dialog.is_none());
}

#[test]
fn cancelling_discard_preserves_edits_and_confirming_restores_disk() {
    let (mut harness, home) = app_harness();
    let before = fs::read_to_string(home.path().join("config.toml")).unwrap();
    change_reasoning(&mut harness);
    click(&mut harness, "撤销修改");
    assert!(matches!(
        harness.state().dialog,
        Some(Dialog::DiscardChanges)
    ));
    render(&mut harness, "dialog-confirm-discard");
    click(&mut harness, "继续编辑");
    assert!(harness.state().has_unsaved_changes());
    click(&mut harness, "撤销修改");
    click(&mut harness, "确认撤销修改");
    assert!(!harness.state().has_unsaved_changes());
    assert_eq!(harness.state().doc.as_ref().unwrap().config_text(), before);
    assert_eq!(
        fs::read_to_string(home.path().join("config.toml")).unwrap(),
        before
    );
}

#[test]
fn invalid_provider_blocks_saving_and_preserves_disk() {
    let (mut harness, home) = app_harness();
    let before = fs::read_to_string(home.path().join("config.toml")).unwrap();
    harness
        .state_mut()
        .doc
        .as_mut()
        .unwrap()
        .config
        .set_value_at(
            &["model_providers", "relay", "base_url"],
            toml_edit::Value::from("not-a-url"),
        );
    harness.run_steps(2);
    click(&mut harness, "保存配置");
    assert!(harness.state().has_unsaved_changes());
    assert_eq!(harness.state().page, Page::Providers);
    assert_eq!(
        fs::read_to_string(home.path().join("config.toml")).unwrap(),
        before
    );
}

#[test]
fn saving_does_not_erase_unapplied_raw_draft() {
    let (mut harness, home) = app_harness();
    let before = fs::read_to_string(home.path().join("config.toml")).unwrap();
    harness.state_mut().page = Page::Raw;
    harness.run_steps(3);
    harness
        .state_mut()
        .raw_buffer
        .push_str("\n# a draft not applied yet\n");
    let draft = harness.state().raw_buffer.clone();
    harness.state_mut().save();
    harness.run_steps(3);
    assert_eq!(harness.state().raw_buffer, draft);
    assert!(harness.state().has_raw_draft());
    assert_eq!(
        fs::read_to_string(home.path().join("config.toml")).unwrap(),
        before
    );
}

#[test]
fn invalid_raw_toml_cannot_be_applied() {
    let (mut harness, _home) = app_harness();
    let original = harness.state().doc.as_ref().unwrap().config_text();
    harness.state_mut().page = Page::Raw;
    harness.run_steps(3);
    harness.state_mut().raw_buffer = "model = [".into();
    harness.run_steps(3);
    assert!(
        harness
            .root()
            .query_by_label_contains("TOML 解析失败")
            .is_some()
    );
    assert!(
        harness
            .root()
            .get_by_label(&format!("{} 应用到编辑器", codex_config::ui::icons::CHECK))
            .accesskit_node()
            .is_disabled()
    );
    assert_eq!(
        harness.state().doc.as_ref().unwrap().config_text(),
        original
    );
}

#[test]
fn malformed_config_shows_a_recoverable_error_page() {
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("config.toml"), "model = [").unwrap();
    let target = home.path().to_path_buf();
    let mut harness = Harness::builder()
        .with_theme(egui::Theme::Light)
        .with_size(egui::vec2(1000.0, 660.0))
        .build_eframe(move |cc| App::with_home(cc, target));
    harness.run_steps(3);
    assert!(harness.state().doc.is_none());
    assert!(harness.root().query_by_label("无法载入配置").is_some());
    render(&mut harness, "state-load-error");
}

#[test]
fn closing_with_unsaved_changes_offers_a_safe_way_back() {
    let (mut harness, _home) = app_harness();
    change_reasoning(&mut harness);
    harness
        .input_mut()
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    harness.run_steps(3);
    assert!(matches!(harness.state().dialog, Some(Dialog::ExitUnsaved)));
    click(&mut harness, "继续编辑");
    assert!(!harness.state().close_confirmed);
    assert!(harness.state().has_unsaved_changes());
}

#[test]
fn every_page_renders_at_the_minimum_window_size() {
    let (mut harness, _home) = app_harness();
    harness.set_size(egui::vec2(1000.0, 660.0));
    for page in Page::ALL {
        harness.state_mut().page = page;
        harness.run_steps(4);
        let save = harness.root().get_by_label("保存配置");
        let bounds = save.rect();
        assert!(
            bounds.left() >= 0.0 && bounds.right() <= 1000.0,
            "save clipped on {page:?}: {bounds:?}"
        );
        render(&mut harness, &format!("compact-{page:?}").to_lowercase());
    }
}

#[test]
fn safety_presets_are_explicit_and_do_not_write_until_saved() {
    let (mut harness, home) = app_harness();
    let before = fs::read_to_string(home.path().join("config.toml")).unwrap();
    click(&mut harness, "先看看，不改文件");
    let config = &harness.state().doc.as_ref().unwrap().config;
    assert_eq!(
        config.str_at(&["sandbox_mode"]).as_deref(),
        Some("read-only")
    );
    assert_eq!(
        config.str_at(&["approval_policy"]).as_deref(),
        Some("on-request")
    );
    assert!(harness.state().has_unsaved_changes());
    assert_eq!(
        fs::read_to_string(home.path().join("config.toml")).unwrap(),
        before
    );
    click(&mut harness, "在项目里帮我工作");
    assert_eq!(
        harness
            .state()
            .doc
            .as_ref()
            .unwrap()
            .config
            .str_at(&["sandbox_mode"])
            .as_deref(),
        Some("workspace-write")
    );
}

#[test]
fn advanced_home_preferences_are_collapsed_by_default() {
    let (mut harness, _home) = app_harness();
    assert!(harness.root().query_by_label("思考摘要").is_none());
    click(&mut harness, "更多偏好设置");
    assert!(harness.root().query_by_label("思考摘要").is_some());
    assert!(!harness.state().has_unsaved_changes());
}

#[test]
fn profile_override_is_visible_and_base_presets_are_disabled() {
    let (mut harness, _home) = app_harness();
    harness
        .state_mut()
        .doc
        .as_mut()
        .unwrap()
        .config
        .set_value_at(&["profile"], toml_edit::Value::from("work"));
    harness.run_steps(3);
    assert!(
        harness
            .root()
            .query_by_label_contains("正在使用配置档「work」")
            .is_some()
    );
    assert!(
        harness
            .root()
            .get_by_label("先看看，不改文件")
            .accesskit_node()
            .is_disabled()
    );
    click(&mut harness, "停用配置档，使用下面的设置");
    assert!(
        harness
            .state()
            .doc
            .as_ref()
            .unwrap()
            .config
            .str_at(&["profile"])
            .is_none()
    );
}

#[test]
fn new_user_can_start_without_creating_any_files() {
    let home = tempfile::tempdir().unwrap();
    let target = home.path().to_path_buf();
    let mut harness = Harness::builder()
        .with_theme(egui::Theme::Light)
        .with_size(egui::vec2(1360.0, 880.0))
        .build_eframe(move |cc| App::with_home(cc, target));
    harness.run_steps(3);
    assert!(!harness.state().has_unsaved_changes());
    assert!(!home.path().join("config.toml").exists());
    render(&mut harness, "state-first-launch");
    click(&mut harness, "连接一个服务商");
    assert_eq!(harness.state().page, Page::Providers);
    assert!(
        harness
            .root()
            .query_by_label_contains("添加服务商（选模板）")
            .is_some()
    );
}

#[test]
fn new_catalog_is_staged_until_save() {
    let (mut harness, home) = app_harness();
    harness.state_mut().dialog = Some(Dialog::CreateCatalog {
        filename: "fresh.json".into(),
    });
    harness.run_steps(3);
    click(&mut harness, "创建并启用");
    assert!(
        !home.path().join("fresh.json").exists(),
        "creation must not bypass explicit save"
    );
    assert!(harness.state().doc.as_ref().unwrap().catalog_dirty());
    click(&mut harness, "保存配置");
    assert!(home.path().join("fresh.json").exists());
}

#[test]
fn raw_catalog_application_preserves_the_edited_model() {
    let (mut harness, _home) = app_harness();
    harness.state_mut().page = Page::Raw;
    harness.state_mut().raw_tab_is_catalog = true;
    harness.run_steps(3);
    harness.state_mut().raw_buffer = harness
        .state()
        .raw_buffer
        .replace("GPT-5.2 Codex", "Edited via JSON");
    harness.run_steps(3);
    let label = format!("{} 应用到编辑器", codex_config::ui::icons::CHECK);
    harness.root().get_by_label(&label).focus();
    harness.key_press(egui::Key::Space);
    harness.run_steps(3);
    assert!(
        harness
            .state()
            .doc
            .as_ref()
            .unwrap()
            .catalog_text()
            .contains("Edited via JSON")
    );
    assert!(harness.state().doc.as_ref().unwrap().catalog_dirty());
}

#[test]
fn preview_can_be_closed_without_losing_the_edits() {
    let (mut harness, _home) = app_harness();
    change_reasoning(&mut harness);
    click(&mut harness, "预览修改");
    assert!(harness.state().show_diff);
    click(&mut harness, "关闭");
    assert!(!harness.state().show_diff);
    assert!(harness.state().has_unsaved_changes());
}

#[test]
fn searching_models_and_cancelling_deletion_preserves_the_catalog() {
    let (mut harness, _home) = app_harness();
    harness.state_mut().page = Page::Models;
    harness.state_mut().model_query = "Claude".into();
    harness.run_steps(3);
    let row = harness.root().get_by_label("Claude Opus 5");
    row.click();
    harness.run_steps(3);
    click(&mut harness, "更多操作");
    click(
        &mut harness,
        &format!("{} 删除模型", codex_config::ui::icons::DELETE),
    );
    assert!(matches!(
        harness.state().dialog,
        Some(Dialog::DeleteModel { .. })
    ));
    click(&mut harness, "取消");
    assert!(!harness.state().has_unsaved_changes());
    assert_eq!(
        catalog::model_count(
            harness
                .state()
                .doc
                .as_ref()
                .unwrap()
                .catalog
                .as_ref()
                .unwrap()
        ),
        4
    );
}
