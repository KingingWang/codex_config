//! GUI tests: the app is rendered off-screen with `egui_kittest`, every page is
//! written out as a PNG (see `screenshots/`) and real interactions are simulated.

use std::fs;
use std::path::{Path, PathBuf};

use codex_config::app::Dialog;
use codex_config::doc::catalog;
use codex_config::doc::toml_ext::TomlPathExt;
use codex_config::doc::validate::{validate, Severity};
use codex_config::page::Page;
use codex_config::App;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::kittest::NodeT;

fn fixture_home() -> PathBuf {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex-home");
    let home = dir.into_path();
    for entry in fs::read_dir(&source).expect("fixture dir") {
        let entry = entry.expect("dir entry");
        fs::copy(entry.path(), home.join(entry.file_name())).expect("copy fixture");
    }
    home
}

fn screenshot_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("screenshots");
    fs::create_dir_all(&dir).expect("create screenshots dir");
    dir
}

fn app_harness() -> (Harness<'static, App>, PathBuf) {
    let home = fixture_home();
    let target = home.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 940.0))
        .with_pixels_per_point(2.0)
        .build_eframe(move |cc| App::with_home(cc, target));
    harness.run_steps(3);
    (harness, home)
}

fn render(harness: &mut Harness<'static, App>, name: &str) {
    let image = harness
        .render()
        .unwrap_or_else(|err| panic!("render {name} failed: {err}"));
    assert!(image.width() > 800, "{name} should be a full window render");
    let path = screenshot_dir().join(format!("{name}.png"));
    image.save(&path).unwrap_or_else(|err| panic!("save {name}: {err}"));
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
    assert!(different > 200, "{name} looks blank ({different} differing samples)");
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
        model.get("display_name").and_then(serde_json::Value::as_str),
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
    assert!(app.dialog.is_none(), "dialog should close after picking a template");
    let config = &app.doc.as_ref().unwrap().config;
    assert_eq!(
        config.str_at(&["model_providers", "openai", "wire_api"]).as_deref(),
        Some("responses")
    );
    assert_eq!(
        config.str_at(&["model_providers", "openai", "env_key"]).as_deref(),
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
            .any(|issue| issue.detail.contains("does-not-exist") && issue.severity == Severity::Warning),
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

    let written = fs::read_to_string(home.join("config.toml")).expect("config");
    assert!(written.contains("# 示例配置"), "comments must survive a save");
    assert!(written.contains("model_reasoning_effort = \"xhigh\""));
    assert!(written.contains("[model_providers.relay]"));

    let backups: Vec<_> = fs::read_dir(&home)
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
fn loads_a_real_codex_home_if_present() {
    let home = codex_config::doc::Document::default_home();
    if !home.join("config.toml").exists() {
        eprintln!("skipping: no real CODEX_HOME at {}", home.display());
        return;
    }
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 940.0))
        .build_eframe(move |cc| App::with_home(cc, home));
    harness.run_steps(3);
    let doc = harness.state().doc.as_ref().expect("document loaded");
    assert!(doc.config.keys_at(&["model_providers"]).len() >= 0);
    let model_count = doc
        .catalog
        .as_ref()
        .map(catalog::model_count)
        .unwrap_or(0);
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
    use codex_config::server::{restart_instance, Host, ServerInstance};
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
