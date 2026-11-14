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
