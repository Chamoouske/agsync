//! Golden output of `agsync hook` and of the warning other commands print, read from a
//! hand written `pending.json` and `update.json`. Without `machine.toml` no check starts.

mod common;

use common::Env;

const PENDING: &str = r#"{
  "commit": "abcdef1234567890",
  "checked_at": "2026-11-27T00:00:00Z",
  "sources": [],
  "kind": "changes",
  "agents": {
    "codex": {
      "added": ["mcp a", "skill b"],
      "changed": [],
      "removed": [],
      "conflicts": ["mcp manual"]
    }
  },
  "builds": []
}"#;

const UPDATE: &str = r#"{"version": "99.0.0", "notified": true}"#;

fn lines() -> String {
    format!(
        "agsync: 2 mudancas pendentes e 1 instalacao manual com o mesmo nome no agsync.yaml (abcdef1); rode `agsync apply` (`--adopt` substitui as manuais)\n\
         agsync 99.0.0 disponivel (atual {}): rode `agsync update`\n",
        env!("CARGO_PKG_VERSION")
    )
}

fn setup() -> Env {
    let env = Env::new();
    std::fs::create_dir_all(&env.data).unwrap();
    std::fs::write(env.data.join("pending.json"), PENDING).unwrap();
    std::fs::write(env.data.join("update.json"), UPDATE).unwrap();
    env
}

#[test]
fn hook_prints_pending_and_new_version() {
    let env = setup();
    assert_eq!(env.ok(&["hook"]), lines());
}

#[test]
fn other_commands_warn_first() {
    let env = setup();
    let (_, out) = env.agsync(&["status"]);
    assert!(out.starts_with(&lines()), "{out}");
}

#[test]
fn nothing_recorded_prints_nothing() {
    let env = Env::new();
    assert_eq!(env.ok(&["hook"]), "");
}

#[test]
fn notices_serialize_to_json() {
    let env = setup();
    let dirs = agsync::paths::Dirs {
        config: env.config.clone(),
        data: env.data.clone(),
    };
    let notices = agsync::pending::Notices::load(&dirs);
    assert_eq!(notices.lines().concat(), lines().replace('\n', ""));
    let json = serde_json::to_value(&notices).unwrap();
    assert_eq!(json["newer"], "99.0.0");
    assert_eq!(json["pending"]["commit"], "abcdef1234567890");
    assert_eq!(json["pending"]["kind"], "changes");
    assert_eq!(json["pending"]["agents"]["codex"]["added"][1], "skill b");
}
