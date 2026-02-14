use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use cc_switch_lib::{
    get_claude_settings_path, get_codex_auth_path, update_settings, AppError, AppSettings,
    AppState, AppType, MultiAppConfig, Prompt, PromptService,
};

#[path = "support.rs"]
#[allow(dead_code)]
mod support;
use support::test_mutex;

struct TestHome {
    path: PathBuf,
    prev_home: Option<OsString>,
    prev_userprofile: Option<OsString>,
}

impl TestHome {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        restore_env_var("HOME", &self.prev_home);
        restore_env_var("USERPROFILE", &self.prev_userprofile);
    }
}

fn restore_env_var(key: &str, value: &Option<OsString>) {
    match value {
        Some(existing) => std::env::set_var(key, existing),
        None => std::env::remove_var(key),
    }
}

fn setup_test_home(test_name: &str) -> TestHome {
    let prev_home = std::env::var_os("HOME");
    let prev_userprofile = std::env::var_os("USERPROFILE");
    let base = std::env::temp_dir().join(format!("cc-switch-prompt-service-{test_name}"));
    if base.exists() {
        let _ = fs::remove_dir_all(&base);
    }
    fs::create_dir_all(&base).expect("create test home");
    std::env::set_var("HOME", &base);
    #[cfg(windows)]
    std::env::set_var("USERPROFILE", &base);
    update_settings(AppSettings::default()).expect("reset settings");
    TestHome {
        path: base,
        prev_home,
        prev_userprofile,
    }
}

fn build_state() -> AppState {
    AppState {
        config: RwLock::new(MultiAppConfig::default()),
    }
}

fn make_prompt(id: &str, content: &str, enabled: bool) -> Prompt {
    Prompt {
        id: id.to_string(),
        name: format!("Prompt {id}"),
        content: content.to_string(),
        description: None,
        enabled,
        created_at: None,
        updated_at: None,
    }
}

fn expected_prompt_path(app: &AppType, home: &Path) -> PathBuf {
    match *app {
        AppType::Claude => get_claude_settings_path()
            .expect("claude settings path")
            .parent()
            .expect("claude settings parent")
            .join("CLAUDE.md"),
        AppType::Codex => get_codex_auth_path()
            .expect("codex auth path")
            .parent()
            .expect("codex auth parent")
            .join("AGENTS.md"),
        AppType::Gemini => home.join(".gemini").join("GEMINI.md"),
        AppType::Opencode | AppType::Omo => {
            panic!("upcoming app should not be used in prompt service tests")
        }
    }
}

#[cfg(unix)]
fn assert_private_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)
        .expect("prompt file metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "expected 0600 permissions");
}

#[test]
fn get_prompts_empty_returns_empty_map() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _home = setup_test_home("get-prompts-empty");
    let state = build_state();

    for app in [AppType::Claude, AppType::Codex, AppType::Gemini] {
        let prompts = PromptService::get_prompts(&state, app.clone()).expect("get prompts");
        assert!(prompts.is_empty(), "expected empty prompts for {app:?}");
    }
}

#[test]
fn get_prompts_returns_all_for_app() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _home = setup_test_home("get-prompts-all");
    let state = build_state();

    {
        let mut cfg = state.config.write().expect("write config");
        cfg.prompts.claude.prompts.insert(
            "claude-1".to_string(),
            make_prompt("claude-1", "Claude one", false),
        );
        cfg.prompts.claude.prompts.insert(
            "claude-2".to_string(),
            make_prompt("claude-2", "Claude two", true),
        );
        cfg.prompts.codex.prompts.insert(
            "codex-1".to_string(),
            make_prompt("codex-1", "Codex one", false),
        );
    }

    let claude = PromptService::get_prompts(&state, AppType::Claude).expect("claude prompts");
    assert_eq!(claude.len(), 2);
    assert!(claude.contains_key("claude-1"));
    assert!(claude.contains_key("claude-2"));

    let codex = PromptService::get_prompts(&state, AppType::Codex).expect("codex prompts");
    assert_eq!(codex.len(), 1);
    assert!(codex.contains_key("codex-1"));

    let gemini = PromptService::get_prompts(&state, AppType::Gemini).expect("gemini prompts");
    assert!(gemini.is_empty());
}

#[test]
fn upsert_prompt_create_and_update() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _home = setup_test_home("upsert-create-update");
    let state = build_state();

    PromptService::upsert_prompt(
        &state,
        AppType::Claude,
        "prompt-1",
        make_prompt("prompt-1", "first", false),
    )
    .expect("create prompt");

    let prompts = PromptService::get_prompts(&state, AppType::Claude).expect("get prompts");
    assert_eq!(
        prompts.get("prompt-1").expect("prompt exists").content,
        "first"
    );

    PromptService::upsert_prompt(
        &state,
        AppType::Claude,
        "prompt-1",
        make_prompt("prompt-1", "second", false),
    )
    .expect("update prompt");

    let prompts = PromptService::get_prompts(&state, AppType::Claude).expect("get prompts");
    assert_eq!(
        prompts.get("prompt-1").expect("prompt exists").content,
        "second"
    );
}

#[test]
fn upsert_prompt_id_conflict_overwrites_entry() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _home = setup_test_home("upsert-id-conflict");
    let state = build_state();

    PromptService::upsert_prompt(
        &state,
        AppType::Claude,
        "key-1",
        make_prompt("prompt-1", "first", false),
    )
    .expect("create prompt");

    PromptService::upsert_prompt(
        &state,
        AppType::Claude,
        "key-1",
        make_prompt("prompt-2", "second", false),
    )
    .expect("overwrite prompt");

    let prompts = PromptService::get_prompts(&state, AppType::Claude).expect("get prompts");
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts.get("key-1").expect("prompt exists").id, "prompt-2");
}

#[test]
fn disable_prompt_clears_file_when_last_enabled() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let home = setup_test_home("disable-last-enabled");
    let state = build_state();

    PromptService::upsert_prompt(
        &state,
        AppType::Claude,
        "prompt-1",
        make_prompt("prompt-1", "active content", true),
    )
    .expect("create enabled prompt");

    let path = expected_prompt_path(&AppType::Claude, home.path());
    let content = fs::read_to_string(&path).expect("read CLAUDE.md");
    assert_eq!(content, "active content");

    PromptService::upsert_prompt(
        &state,
        AppType::Claude,
        "prompt-1",
        make_prompt("prompt-1", "active content", false),
    )
    .expect("disable prompt");

    let content = fs::read_to_string(&path).expect("read CLAUDE.md");
    assert!(
        content.is_empty(),
        "expected prompt file to be cleared when disabling"
    );
}

#[test]
fn delete_prompt_success_and_missing() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _home = setup_test_home("delete-success-missing");
    let state = build_state();

    PromptService::upsert_prompt(
        &state,
        AppType::Codex,
        "prompt-1",
        make_prompt("prompt-1", "remove me", false),
    )
    .expect("create prompt");

    PromptService::delete_prompt(&state, AppType::Codex, "prompt-1")
        .expect("delete existing prompt");
    let prompts = PromptService::get_prompts(&state, AppType::Codex).expect("get prompts");
    assert!(!prompts.contains_key("prompt-1"));

    PromptService::delete_prompt(&state, AppType::Codex, "missing")
        .expect("delete missing prompt should succeed");
}

#[test]
fn delete_prompt_rejects_active() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _home = setup_test_home("delete-active");
    let state = build_state();

    PromptService::upsert_prompt(
        &state,
        AppType::Gemini,
        "prompt-1",
        make_prompt("prompt-1", "active", true),
    )
    .expect("create active prompt");

    let err = PromptService::delete_prompt(&state, AppType::Gemini, "prompt-1")
        .expect_err("delete active prompt should fail");
    assert!(matches!(err, AppError::InvalidInput(_)));
}

#[test]
fn enable_prompt_writes_file_and_disables_previous() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let home = setup_test_home("enable-switch");
    let state = build_state();

    {
        let mut cfg = state.config.write().expect("write config");
        cfg.prompts
            .codex
            .prompts
            .insert("old".to_string(), make_prompt("old", "old content", true));
        cfg.prompts
            .codex
            .prompts
            .insert("new".to_string(), make_prompt("new", "new content", false));
    }

    PromptService::enable_prompt(&state, AppType::Codex, "new").expect("enable prompt");

    let prompts = PromptService::get_prompts(&state, AppType::Codex).expect("get prompts");
    assert!(prompts.get("new").expect("new prompt").enabled);
    assert!(!prompts.get("old").expect("old prompt").enabled);

    let path = expected_prompt_path(&AppType::Codex, home.path());
    let content = fs::read_to_string(&path).expect("read AGENTS.md");
    assert_eq!(content, "new content");
}

#[test]
fn import_prompt_from_file_missing() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _home = setup_test_home("import-missing");
    let state = build_state();

    let err = PromptService::import_from_file(&state, AppType::Claude)
        .expect_err("missing prompt file should fail");
    assert!(matches!(err, AppError::Message(_)));
}

#[test]
fn import_prompt_from_file_existing() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let home = setup_test_home("import-existing");
    let state = build_state();

    let path = expected_prompt_path(&AppType::Gemini, home.path());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create gemini config dir");
    }
    fs::write(&path, "imported content").expect("write GEMINI.md");

    let id =
        PromptService::import_from_file(&state, AppType::Gemini).expect("import prompt from file");
    assert!(id.starts_with("imported-"));

    let prompts = PromptService::get_prompts(&state, AppType::Gemini).expect("get prompts");
    let prompt = prompts.get(&id).expect("imported prompt exists");
    assert_eq!(prompt.content, "imported content");
    assert!(!prompt.enabled);
}

#[test]
fn prompt_file_ops_write_and_permissions() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let home = setup_test_home("file-ops-permissions");
    let state = build_state();

    let cases = [
        (AppType::Claude, "claude-content"),
        (AppType::Codex, "codex-content"),
        (AppType::Gemini, "gemini-content"),
    ];

    for (app, content) in cases {
        let id = format!("{app:?}-prompt");
        PromptService::upsert_prompt(&state, app.clone(), &id, make_prompt(&id, content, true))
            .expect("upsert prompt");

        let path = expected_prompt_path(&app, home.path());
        let written = fs::read_to_string(&path).expect("read prompt file");
        assert_eq!(written, content);
        #[cfg(unix)]
        assert_private_permissions(&path);
    }
}
