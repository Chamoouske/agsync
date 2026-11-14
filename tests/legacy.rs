//! `agsync migrate-legacy` against a fake legacy setup in a temp home.
//!
//! Tasks or units and the user variable carry test names (`AGSYNC_TEST_LEGACY_NAME`,
//! `AGSYNC_TEST_LEGACY_VAR`), and a guard removes them even when the test fails. The
//! `1mcp` entry is planted in the fake Codex and Hermes, and in the real Claude Code and
//! OpenCode when they are in PATH, all with temp configs.

mod common;

#[cfg(not(windows))]
#[path = "legacy/linux.rs"]
mod os;
#[cfg(windows)]
#[path = "legacy/windows.rs"]
mod os;

use std::path::{Path, PathBuf};

use common::{Env, Untouched, fake_bin, git, path_with};

const CODEX: &str = "[mcp_servers.1mcp]\ncommand = \"1mcp\"\nargs = [\"proxy\"]\n\n[mcp_servers.1mcp.env]\n\n[mcp_servers.keep]\ncommand = \"manual-tool\"\n";
const HERMES: &str = "mcp_servers:\n  1mcp:\n    command: 1mcp\n    args: [proxy]\n  keep:\n    command: manual-tool\n";
const OPENCODE: &str = r#"{
  // mine
  "mcp": {
    "1mcp": { "type": "local", "command": ["1mcp", "proxy"] },
    "keep": { "type": "local", "command": ["manual-tool"] }
  }
}
"#;

fn unique(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}{}-{nanos}", std::process::id())
}

fn skill(dir: &Path, name: &str) {
    std::fs::create_dir_all(dir.join(name)).unwrap();
    let text = format!("---\nname: {name}\ndescription: t\n---\n");
    std::fs::write(dir.join(name).join("SKILL.md"), text).unwrap();
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn plant_agents(env: &Env, claude: bool, opencode: bool) {
    std::fs::write(env.codex_dir.join("config.toml"), CODEX).unwrap();
    std::fs::write(env.hermes_dir.join("config.yaml"), HERMES).unwrap();
    if claude {
        let proxy = r#"{"type":"stdio","command":"1mcp","args":["proxy"]}"#;
        let keep = r#"{"type":"stdio","command":"manual-tool"}"#;
        env.claude(&["mcp", "add-json", "--scope", "user", "1mcp", proxy]);
        env.claude(&["mcp", "add-json", "--scope", "user", "keep", keep]);
    }
    if opencode {
        std::fs::write(env.opencode_dir.join("opencode.json"), OPENCODE).unwrap();
    }
}

fn plant_home(env: &Env) {
    let apm = env.home.join(".apm");
    std::fs::create_dir_all(apm.join("apm_modules")).unwrap();
    std::fs::write(apm.join("apm.yml"), "name: legacy\n").unwrap();
    skill(&env.home.join(".claude").join("skills"), "tdd");
    let canonical = env.home.join(".agents").join("skills");
    skill(&canonical, "grill-me");
    let backups = env.root.path().join("link-backups");
    let opencode_skills = env.home.join(".config").join("opencode").join("skills");
    let target = opencode_skills.join("grill-me");
    agsync::agent::link::install(&canonical.join("grill-me"), &target, &backups).unwrap();
}

fn init_with_tdd(env: &Env) {
    git(&env.remote, &["init", "-q", "-b", "main"]);
    skill(&env.remote.join("skills"), "tdd");
    env.push("version: 1\nskills:\n  - path: skills/tdd\n");
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
}

fn backup_dir(env: &Env) -> PathBuf {
    let backups = env.data.join("backups");
    let found = std::fs::read_dir(&backups)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("legacy-")
        });
    found.expect("legacy backup dir")
}

#[test]
fn migrate_legacy_backs_up_and_removes_the_old_setup() {
    let mut env = Env::new();
    let claude = agsync::which::find("claude").is_some();
    let opencode = agsync::which::find("opencode").is_some();
    if !claude || !opencode {
        eprintln!("SKIP parte real de migrate_legacy: claude={claude} opencode={opencode}");
    }
    let real = Untouched::snapshot(&os::real_files());
    let bin = fake_bin(&env, &["codex", "hermes", "1mcp"]);
    std::fs::write(bin.join("apm"), "#!/usr/bin/env python3\nprint('apm')\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(bin.join("apm"), mode).unwrap();
    }
    env.path = Some(path_with(
        &["git", "sh", "claude", "opencode", "node"],
        &[bin],
    ));
    let name = unique("agsync-test-legacy-");
    let var = unique("AGSYNC_TEST_ONE_MCP_CONFIG_").replace('-', "_");
    env.vars = vec![
        ("AGSYNC_TEST_LEGACY_NAME".to_string(), name.clone()),
        ("AGSYNC_TEST_LEGACY_VAR".to_string(), var.clone()),
        (
            "FAKE_AGENT_LOG".to_string(),
            env.root
                .path()
                .join("fake.log")
                .to_string_lossy()
                .to_string(),
        ),
    ];
    env.vars.extend(os::vars(&env));
    let fake = os::Fake::create(&env, &name, &var);
    plant_agents(&env, claude, opencode);
    plant_home(&env);
    init_with_tdd(&env);
    fake.assert_present();

    let (ok, out) = env.agsync(&["migrate-legacy"]);
    assert!(!ok, "{out}");
    assert!(out.contains("rode `agsync migrate-legacy --yes`"), "{out}");
    assert!(env.home.join(".apm").is_dir());

    let out = env.ok(&["migrate-legacy", "--yes"]);
    for line in [
        "codex: mcp 1mcp (1mcp proxy)",
        "hermes: mcp 1mcp (1mcp proxy)",
        "removido: codex: mcp 1mcp",
        "npm uninstall -g @1mcp/agent",
        os::APM_HINT,
    ] {
        assert!(out.contains(line), "missing {line:?}:\n{out}");
    }
    let tdd = format!(
        "{} (copia): o agsync.yaml tem esse nome: `agsync apply --adopt` vai assumi-la",
        env.home
            .join(".claude")
            .join("skills")
            .join("tdd")
            .display()
    );
    assert!(out.contains(&tdd), "{out}");
    assert!(out.contains("grill-me (link para "), "{out}");
    assert!(out.contains("grill-me (copia): fora do agsync.yaml: fica como entrada manual"));

    let codex = read(&env.codex_dir.join("config.toml"));
    assert!(
        !codex.contains("1mcp") && codex.contains("[mcp_servers.keep]"),
        "{codex}"
    );
    let hermes = read(&env.hermes_dir.join("config.yaml"));
    assert!(
        !hermes.contains("1mcp") && hermes.contains("keep"),
        "{hermes}"
    );
    if claude {
        assert!(out.contains("removido: claude: mcp 1mcp"), "{out}");
        let servers = env.servers();
        assert!(!servers.contains_key("1mcp") && servers.contains_key("keep"));
    }
    if opencode {
        assert!(out.contains("removido: opencode: mcp 1mcp"), "{out}");
        let text = read(&env.opencode_dir.join("opencode.json"));
        assert!(!text.contains("1mcp") && text.contains("// mine"), "{text}");
    }
    assert!(!env.home.join(".apm").exists());
    let backup = backup_dir(&env);
    assert_eq!(read(&backup.join("apm").join("apm.yml")), "name: legacy\n");
    assert!(backup.join("apm").join("apm_modules").is_dir());
    assert!(std::fs::read_dir(&backup).unwrap().count() >= 4);
    assert!(env.home.join(".claude").join("skills").join("tdd").is_dir());
    assert!(
        env.home
            .join(".agents")
            .join("skills")
            .join("grill-me")
            .is_dir()
    );
    fake.assert_removed(&backup);

    let again = env.ok(&["migrate-legacy", "--yes"]);
    assert!(
        again.contains("nada do sistema antigo para remover"),
        "{again}"
    );
    real.assert_same();
}
