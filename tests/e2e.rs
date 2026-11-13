//! End to end: local git repo as remote, real `claude` CLI, every dir temporary.
//!
//! Skipped (with a message) when `claude` is not in PATH. The real Claude config
//! is never used: every `claude` and `agsync` run gets a temp `CLAUDE_CONFIG_DIR`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use agsync::tmp::TempDir;
use serde_json::Value;

const SECRET: &str = "s3cr3t-token-value";

struct Env {
    _root: TempDir,
    remote: PathBuf,
    config: PathBuf,
    data: PathBuf,
    claude_dir: PathBuf,
    tmp: PathBuf,
}

impl Env {
    fn new() -> Env {
        let root = TempDir::new("agsync-e2e").unwrap();
        let dir = |name: &str| {
            let path = root.path().join(name);
            std::fs::create_dir_all(&path).unwrap();
            path
        };
        Env {
            remote: dir("remote"),
            config: dir("config"),
            data: dir("data"),
            claude_dir: dir("claude"),
            tmp: dir("tmp"),
            _root: root,
        }
    }

    fn isolate(&self, cmd: &mut Command) {
        assert!(self.claude_dir.starts_with(std::env::temp_dir()));
        cmd.env("CLAUDE_CONFIG_DIR", &self.claude_dir)
            .env("AGSYNC_CONFIG_DIR", &self.config)
            .env("AGSYNC_DATA_DIR", &self.data)
            .env("TEMP", &self.tmp)
            .env("TMP", &self.tmp)
            .env("TMPDIR", &self.tmp)
            .stdin(Stdio::null());
    }

    fn agsync(&self, args: &[&str]) -> (bool, String) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_agsync"));
        cmd.args(args);
        self.isolate(&mut cmd);
        let out = cmd.output().unwrap();
        let text = text(&out);
        assert!(!text.contains(SECRET), "secret leaked in output:\n{text}");
        eprintln!("$ agsync {}\n{text}", args.join(" "));
        (out.status.success(), text)
    }

    fn ok(&self, args: &[&str]) -> String {
        let (ok, text) = self.agsync(args);
        assert!(ok, "agsync {args:?} failed:\n{text}");
        text
    }

    fn claude(&self, args: &[&str]) {
        let mut cmd = Command::new("claude");
        cmd.args(args);
        self.isolate(&mut cmd);
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "claude {args:?}: {}", text(&out));
    }

    fn push(&self, yaml: &str) {
        std::fs::write(self.remote.join("agsync.yaml"), yaml).unwrap();
        git(&self.remote, &["add", "-A"]);
        git(&self.remote, &["commit", "-q", "-m", "update"]);
    }

    fn servers(&self) -> serde_json::Map<String, Value> {
        let text = std::fs::read_to_string(self.claude_dir.join(".claude.json")).unwrap();
        let root: Value = serde_json::from_str(&text).unwrap();
        root["mcpServers"].as_object().cloned().unwrap_or_default()
    }

    fn args_of(&self, name: &str) -> Value {
        self.servers()[name]["args"].clone()
    }

    fn state(&self) -> String {
        std::fs::read_to_string(self.data.join("state.json")).unwrap()
    }

    fn skill(&self, name: &str) -> PathBuf {
        self.claude_dir.join("skills").join(name)
    }
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", text(&out));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn yaml(echo_arg: &str, extra_mcps: &str, skills: &str) -> String {
    format!(
        r#"version: 1
profiles: [work, personal]
mcps:
  echo:
    command: node
    args: ["{echo_arg}"]
    env: {{TOKEN: "{SECRET}"}}
  home:
    profiles: [personal]
    command: node
  built:
    command: node
    source: {{git: "https://example.com/mcp.git", ref: v1.0.0}}
{extra_mcps}skills:
  - git: https://example.com/skills.git
    ref: v2.0.0
    path: sub/remote-skill
{skills}"#
    )
}

const HELLO: &str = "  - path: skills/hello\n";

#[test]
fn end_to_end_with_real_claude() {
    if agsync::agent::find_in_path("claude").is_none() {
        eprintln!("SKIP end_to_end_with_real_claude: `claude` nao esta no PATH");
        return;
    }
    let env = Env::new();
    git(&env.remote, &["init", "-q", "-b", "main"]);
    let skill_dir = env.remote.join("skills").join("hello");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: hello\ndescription: test\n---\nhi\n",
    )
    .unwrap();
    env.push(&yaml("v1.js", "", HELLO));

    let untouched = r#"{"type":"stdio","command":"manual-tool","args":["keep"]}"#;
    env.claude(&["mcp", "add-json", "--scope", "user", "untouched", untouched]);
    assert!(env.servers().contains_key("untouched"));

    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
    assert!(env.config.join("machine.toml").is_file());

    let status = env.ok(&["status"]);
    let head = git(&env.remote, &["rev-parse", "HEAD"]);
    assert!(status.contains("commit aplicado: nenhum"), "{status}");
    assert!(
        status.contains(&format!("commit remoto:   {head}")),
        "{status}"
    );
    assert!(status.contains("mcp echo: adicionar"), "{status}");
    assert!(status.contains("env: TOKEN"), "{status}");
    assert!(status.contains("skill hello: adicionar"), "{status}");
    assert!(status.contains("mcp built: fonte git: marco 3"), "{status}");
    assert!(
        status.contains("skill remote-skill: fonte git: marco 3"),
        "{status}"
    );
    assert!(
        status.contains("codex: adaptador ainda nao implementado"),
        "{status}"
    );
    assert!(!status.contains("home"), "{status}");
    assert!(!status.contains("untouched"), "{status}");

    let diff = env.ok(&["diff"]);
    assert!(diff.contains("mcp echo: adicionar"), "{diff}");

    let (ok, text) = env.agsync(&["apply"]);
    assert!(!ok, "apply without tty and --yes must refuse");
    assert!(text.contains("sem TTY"), "{text}");
    assert!(!env.servers().contains_key("echo"));

    let applied = env.ok(&["apply", "--yes"]);
    assert!(applied.contains("reiniciar"), "{applied}");
    assert_eq!(env.args_of("echo"), serde_json::json!(["v1.js"]));
    assert_eq!(env.servers()["echo"]["env"]["TOKEN"], SECRET);
    assert!(env.skill("hello").join("SKILL.md").is_file());
    let state = env.state();
    assert!(state.contains(&head), "{state}");
    assert!(!state.contains(SECRET), "{state}");
    assert!(!state.contains("TOKEN"), "{state}");
    assert!(env.ok(&["status"]).contains("nada a mudar"));

    let two =
        "  two:\n    url: https://example.com/mcp\n    headers: {Authorization: \"Bearer x\"}\n";
    env.push(&yaml("v2.js", two, HELLO));
    let status = env.ok(&["status"]);
    assert!(status.contains("mcp echo: atualizar"), "{status}");
    assert!(status.contains("mcp two: adicionar"), "{status}");
    env.ok(&["apply", "--yes"]);
    assert_eq!(env.args_of("echo"), serde_json::json!(["v2.js"]));
    assert_eq!(env.servers()["two"]["type"], "http");

    env.push(&yaml("v2.js", "", ""));
    let diff = env.ok(&["diff"]);
    assert!(diff.contains("mcp two: remover"), "{diff}");
    assert!(diff.contains("skill hello: remover"), "{diff}");
    env.ok(&["apply", "--yes"]);
    assert!(!env.servers().contains_key("two"));
    assert!(std::fs::symlink_metadata(env.skill("hello")).is_err());
    assert!(!env.state().contains("\"two\""));

    let hacked = r#"{"type":"stdio","command":"node","args":["hacked.js"]}"#;
    env.claude(&["mcp", "remove", "--scope", "user", "echo"]);
    env.claude(&["mcp", "add-json", "--scope", "user", "echo", hacked]);
    let status = env.ok(&["status"]);
    assert!(status.contains("mcp echo: desvio manual"), "{status}");
    let backups_before = count(&env.data.join("backups"));
    env.ok(&["apply", "--yes"]);
    assert_eq!(env.args_of("echo"), serde_json::json!(["v2.js"]));
    assert!(count(&env.data.join("backups")) > backups_before);

    let manual = r#"{"type":"stdio","command":"node","args":["manual.js"]}"#;
    env.claude(&["mcp", "add-json", "--scope", "user", "manual", manual]);
    let repo_manual = "  manual:\n    command: node\n    args: [\"repo.js\"]\n";
    env.push(&yaml("v2.js", repo_manual, HELLO));
    let status = env.ok(&["status"]);
    assert!(status.contains("mcp manual: conflito"), "{status}");
    env.ok(&["apply", "--yes"]);
    assert_eq!(env.args_of("manual"), serde_json::json!(["manual.js"]));
    assert!(!env.state().contains("\"manual\""));
    assert!(env.skill("hello").join("SKILL.md").is_file());

    let adopted = env.ok(&["apply", "--yes", "--adopt"]);
    assert!(adopted.contains("assumir"), "{adopted}");
    assert_eq!(env.args_of("manual"), serde_json::json!(["repo.js"]));
    assert!(env.state().contains("\"manual\""));
    assert!(env.ok(&["status"]).contains("nada a mudar"));

    assert_eq!(
        env.servers()["untouched"]["args"],
        serde_json::json!(["keep"])
    );
    let leftovers: Vec<_> = std::fs::read_dir(&env.tmp)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("agsync-snapshot")
        })
        .collect();
    assert!(leftovers.is_empty(), "snapshot not deleted: {leftovers:?}");
}

fn count(dir: &Path) -> usize {
    std::fs::read_dir(dir).map(|d| d.count()).unwrap_or(0)
}
