//! `agsync init` runs every setup step; a failed step is reported and the next ones run.

mod common;

use common::{Env, fake_bin, git, path_with};

#[test]
fn init_yes_schedules_hooks_and_applies() {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex", "bash"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push("version: 1\nmcps:\n  echo:\n    command: node\n");
    let url = env.remote.to_string_lossy().to_string();

    let out = env.ok(&["init", &url, "--profile", "work", "--yes"]);
    for step in [
        "[1/5]", "[2/5]", "[3/5]", "[4/5]", "[5/5]", "repo ok", "agendado",
    ] {
        assert!(out.contains(step), "missing {step}:\n{out}");
    }
    assert!(env.config.join("machine.toml").is_file());
    assert!(
        std::fs::read_to_string(env.schedule_file())
            .unwrap()
            .ends_with("\t15\n")
    );
    let bashrc = std::fs::read_to_string(env.home.join(".bashrc")).unwrap();
    assert!(bashrc.contains("# >>> agsync hook >>>"), "{bashrc}");
    assert!(env.state().contains("\"echo\""), "{}", env.state());
    assert!(out.contains("codex: mcp echo: ok (adicionar)"), "{out}");
}

#[test]
fn init_reports_an_unreachable_repo_and_keeps_going() {
    let env = Env::new();
    let gone = env.root.path().join("no-such-repo");
    let url = gone.to_string_lossy().to_string();
    let (ok, out) = env.agsync(&["init", &url, "--profile", "work"]);
    assert!(!ok, "{out}");
    assert!(out.contains("repo inacessivel"), "{out}");
    assert!(
        out.contains("sem TTY: hook de shell nao instalado"),
        "{out}"
    );
    assert!(out.contains("pulado: repo inacessivel"), "{out}");
    assert!(env.config.join("machine.toml").is_file());
    assert!(env.schedule_file().is_file());
    assert!(!env.home.join(".bashrc").exists());
}

/// A remote with `yaml` and a fake `codex` that logs what it is asked.
fn codex_env(yaml: &str) -> (Env, String) {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex", "bash"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(yaml);
    let url = env.remote.to_string_lossy().to_string();
    (env, url)
}

#[test]
fn init_without_a_tty_skips_the_first_apply() {
    let (env, url) = codex_env("version: 1\nmcps:\n  echo:\n    command: node\n");
    let out = env.ok(&["init", &url, "--profile", "work"]);
    assert!(out.contains("pulado (sem TTY)"), "{out}");
    assert!(!out.contains("codex: mcp echo"), "{out}");
    assert!(!env.data.join("state.json").exists());
}

#[test]
fn init_silent_update_applies_without_a_tty() {
    let (env, url) = codex_env("version: 1\nmcps:\n  echo:\n    command: node\n");
    let out = env.ok(&["init", &url, "--profile", "work", "--silent-update"]);
    assert!(out.contains("atualizacao silenciosa"), "{out}");
    assert!(out.contains("codex: mcp echo: ok (adicionar)"), "{out}");
    assert!(!out.contains("Aplicar estas mudancas?"), "{out}");
    assert!(
        out.contains("sem TTY: hook de shell nao instalado"),
        "{out}"
    );
    assert!(env.state().contains("\"echo\""), "{}", env.state());
    assert!(env.pending().is_none());
}

#[test]
fn init_silent_update_with_nothing_pending_ends_normally() {
    let (env, url) = codex_env("version: 1\n");
    let out = env.ok(&["init", &url, "--profile", "work", "--silent-update"]);
    assert!(out.contains("nada a aplicar"), "{out}");
    assert!(!out.contains("build mcp"), "{out}");
}

#[test]
fn init_applies_without_asking_when_agsync_yaml_sets_silent_update() {
    let (env, url) =
        codex_env("version: 1\nsilentUpdate: true\nmcps:\n  echo:\n    command: node\n");
    let out = env.ok(&["init", &url, "--profile", "work"]);
    assert!(out.contains("silentUpdate no agsync.yaml"), "{out}");
    assert!(out.contains("codex: mcp echo: ok (adicionar)"), "{out}");
    assert!(env.state().contains("\"echo\""), "{}", env.state());
}

#[test]
fn init_silent_update_reports_failures_and_keeps_state_consistent() {
    let (env, _) = codex_env("version: 1\n");
    let source = env.root.path().join("source");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("README"), "x\n").unwrap();
    git(&source, &["init", "-q", "-b", "main"]);
    git(&source, &["add", "-A"]);
    git(&source, &["commit", "-q", "-m", "one"]);
    let source_url = source.to_string_lossy().replace('\\', "/");
    env.push(&format!(
        "version: 1\nmcps:\n  echo:\n    command: node\n  broken:\n    command: node\n    source:\n      git: {source_url}\n      ref: main\n      build: [\"exit 7\"]\n"
    ));
    let url = env.remote.to_string_lossy().to_string();

    let (ok, out) = env.agsync(&["init", &url, "--profile", "work", "--silent-update"]);
    assert!(!ok, "{out}");
    assert!(out.contains("apply falhou"), "{out}");
    assert!(out.contains("falharam"), "{out}");
    assert!(out.contains("codex: mcp echo: ok (adicionar)"), "{out}");
    let state: serde_json::Value = serde_json::from_str(&env.state()).unwrap();
    let text = state.to_string();
    assert!(text.contains("\"echo\""), "{text}");
    assert!(!text.contains("\"broken\""), "{text}");
}

#[test]
fn init_help_documents_silent_update() {
    let env = Env::new();
    let out = env.ok(&["init", "--help"]);
    assert!(out.contains("--silent-update"), "{out}");
    assert!(out.contains("silentUpdate"), "{out}");
}
