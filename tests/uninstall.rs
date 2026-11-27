//! `agsync uninstall` against the fake Codex and Hermes: tracked entries leave the
//! agents, manual ones stay, schedule, hook and state go, and `--purge` deletes the
//! config and data dirs.

mod common;

use common::{Env, count, fake_bin, git, path_with};

const CODEX: &str = "[mcp_servers.untouched]\ncommand = \"manual-tool\"\n";
const HERMES: &str = "mcp_servers:\n  untouched:\n    command: manual-tool\n";
const BASHRC: &str = "export A=1\n";

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn setup() -> Env {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex", "hermes"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    std::fs::write(env.codex_dir.join("config.toml"), CODEX).unwrap();
    std::fs::write(env.hermes_dir.join("config.yaml"), HERMES).unwrap();
    git(&env.remote, &["init", "-q", "-b", "main"]);
    let skill = env.remote.join("skills").join("hello");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: hello\ndescription: t\n---\n",
    )
    .unwrap();
    env.push("version: 1\nmcps:\n  plain:\n    command: node\nskills:\n  - path: skills/hello\n");
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
    env.ok(&["apply", "--yes"]);
    let (ok, out) = env.agsync_with_input(&["secret", "set", "UNINSTALL_T"], Some("v\n"));
    assert!(ok, "{out}");
    let block = agsync_core::shell::block::with_block(BASHRC, "agsync hook");
    std::fs::write(env.home.join(".bashrc"), block).unwrap();
    std::fs::write(env.data.join("pending.json"), "{}").unwrap();
    env
}

#[test]
fn uninstall_removes_tracked_entries_schedule_hook_and_state() {
    let env = setup();
    let codex_skill = env.home.join(".agents").join("skills").join("hello");
    let hermes_skill = env.hermes_dir.join("skills").join("hello");
    assert!(read(&env.codex_dir.join("config.toml")).contains("[mcp_servers.plain]"));
    assert!(codex_skill.join("SKILL.md").is_file());
    assert!(hermes_skill.join("SKILL.md").is_file());
    assert!(env.schedule_file().is_file());

    let (ok, out) = env.agsync(&["uninstall"]);
    assert!(!ok, "{out}");
    assert!(out.contains("rode `agsync uninstall --yes`"), "{out}");
    assert!(env.data.join("state.json").is_file());

    let out = env.ok(&["uninstall", "--yes"]);
    assert!(!out.contains("TODOS OS BACKUPS"), "{out}");
    assert!(out.contains("codex: mcp plain: removido"), "{out}");
    assert!(out.contains("hermes: skill hello: removida"), "{out}");
    let exe = std::path::absolute(env!("CARGO_BIN_EXE_agsync")).unwrap();
    assert!(
        out.contains(&format!("o binario continua em {}", exe.display())),
        "{out}"
    );
    assert_eq!(
        read(&env.codex_dir.join("config.toml")).trim(),
        CODEX.trim()
    );
    let hermes = read(&env.hermes_dir.join("config.yaml"));
    assert!(
        hermes.contains("untouched") && !hermes.contains("plain"),
        "{hermes}"
    );
    assert!(std::fs::symlink_metadata(&codex_skill).is_err());
    assert!(std::fs::symlink_metadata(&hermes_skill).is_err());
    assert!(!env.schedule_file().exists());
    assert_eq!(read(&env.home.join(".bashrc")), BASHRC);
    assert!(!env.data.join("state.json").exists());
    assert!(!env.data.join("pending.json").exists());
    assert!(env.config.join("machine.toml").is_file());
    assert!(env.config.join("secrets.env").is_file());
    assert!(count(&env.data.join("backups")) > 0);

    let out = env.ok(&["uninstall", "--yes", "--purge"]);
    assert!(out.contains("TODOS OS BACKUPS"), "{out}");
    assert!(!env.config.exists());
    assert!(!env.data.exists());
}
