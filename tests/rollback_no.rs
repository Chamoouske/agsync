//! `agsync::rollback` answered no, run in this process against a fake `codex`: nothing
//! is written. Alone in its binary, since it points this process's environment at the
//! temp dirs.

mod common;

use agsync::prompt::Confirm;
use common::{Env, Untouched, fake_bin, git, path_with};

struct No;

impl Confirm for No {
    fn can_answer(&self) -> bool {
        true
    }

    fn ask(&self, _question: &str) -> anyhow::Result<bool> {
        Ok(false)
    }
}

fn manifest(arg: &str) -> String {
    format!("version: 1\nmcps:\n  plain:\n    command: node\n    args: [\"{arg}\"]\n")
}

/// Points this process at `env`, as [`Env::isolate`] does for a child.
fn enter(env: &Env) {
    let vars = [
        ("CLAUDE_CONFIG_DIR", env.claude_dir.as_os_str()),
        ("CODEX_HOME", env.codex_dir.as_os_str()),
        ("AGSYNC_OPENCODE_CONFIG_DIR", env.opencode_dir.as_os_str()),
        ("HERMES_HOME", env.hermes_dir.as_os_str()),
        ("HOME", env.home.as_os_str()),
        ("USERPROFILE", env.home.as_os_str()),
        ("AGSYNC_CONFIG_DIR", env.config.as_os_str()),
        ("AGSYNC_DATA_DIR", env.data.as_os_str()),
        ("PATH", env.path.as_deref().unwrap()),
    ];
    for (key, value) in vars {
        unsafe { std::env::set_var(key, value) };
    }
    unsafe { std::env::remove_var("OPENCODE_CONFIG_DIR") };
}

#[test]
fn rollback_answered_no_changes_nothing() {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&manifest("v1.js"));
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    env.ok(&["apply", "--yes"]);
    env.push(&manifest("v2.js"));
    env.ok(&["apply", "--yes"]);
    let watch = env.data.join("watch.json");
    assert!(watch.exists());
    let files = [
        env.codex_dir.join("config.toml"),
        env.data.join("state.json"),
        watch,
    ];
    let before = Untouched::snapshot(&files);

    enter(&env);
    let planned = agsync::rollback::target().unwrap().plan().unwrap();
    let mut events = Vec::new();
    let applied = agsync::rollback::run(planned, &No, &mut |e| events.push(e.clone())).unwrap();

    assert!(applied.pending && !applied.confirmed);
    assert!(applied.events.is_empty() && events.is_empty());
    before.assert_same();
}
