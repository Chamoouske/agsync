//! Golden output of `agsync status` and `agsync diff` against a fake `codex`.
//!
//! The plan covers an added MCP, a manual conflict, a git source with builds, a skill
//! and the agents missing from `PATH`, so every section of both commands shows up.

mod common;

use common::{Env, fake_bin, git, path_with};

const CODEX_FIXTURE: &str = r#"[mcp_servers.manual]
command = "manual-tool"
args = ["mine"]
"#;

const STATUS: &str = "repo: <REMOTE> (main)
perfis ativos: work
commit aplicado: nenhum
commit remoto:   <HEAD>
codex:
  mcp built: adicionar [stdio: node <DIR>/out.txt]
  mcp manual: conflito com instalacao manual de mesmo nome, nao sera alterada (use --adopt para substituir)
  mcp plain: adicionar [stdio: node v1.js; env: MODE]
  skill hello: adicionar

instalacoes manuais com o mesmo nome no agsync.yaml (`agsync apply --adopt` substitui):
  codex: mcp manual

!!! BUILDS: estes comandos executam codigo de terceiros nesta maquina !!!
  mcp built: <SOURCE> @ <SOURCE_SHA> (v1)
    $ echo built> out.txt
    $ echo second step

pulados:
  claude: claude nao encontrado no PATH
  opencode: opencode nao encontrado no PATH
  hermes: hermes nao encontrado no PATH
  antigravity: agy nao encontrado no PATH
";

const DIFF: &str = "codex:
  mcp built: adicionar [stdio: node <DIR>/out.txt]
  mcp manual: conflito com instalacao manual de mesmo nome, nao sera alterada (use --adopt para substituir)
  mcp plain: adicionar [stdio: node v1.js; env: MODE]
  skill hello: adicionar

instalacoes manuais com o mesmo nome no agsync.yaml (`agsync apply --adopt` substitui):
  codex: mcp manual

!!! BUILDS: estes comandos executam codigo de terceiros nesta maquina !!!
  mcp built: <SOURCE> @ <SOURCE_SHA> (v1)
    $ echo built> out.txt
    $ echo second step

";

fn source(env: &Env) -> (String, String) {
    let dir = env.root.path().join("source");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("README"), "x\n").unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    git(&dir, &["tag", "-a", "v1", "-m", "v1"]);
    let sha = git(&dir, &["rev-parse", "HEAD"]);
    (dir.to_string_lossy().replace('\\', "/"), sha)
}

fn setup() -> (Env, Vec<(String, &'static str)>) {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars
        .push(("FAKE_AGENT_LOG".into(), log.to_string_lossy().to_string()));
    std::fs::write(env.codex_dir.join("config.toml"), CODEX_FIXTURE).unwrap();
    git(&env.remote, &["init", "-q", "-b", "main"]);
    let skill = env.remote.join("skills").join("hello");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: hello\ndescription: t\n---\nhi\n",
    )
    .unwrap();
    let (url, sha) = source(&env);
    env.push(&format!(
        r#"version: 1
profiles: [work]
mcps:
  plain:
    command: node
    args: ["v1.js"]
    env: {{MODE: fast}}
  manual:
    command: node
    args: ["repo.js"]
  built:
    command: node
    args: ["{{dir}}/out.txt"]
    source: {{git: "{url}", ref: v1, build: ["echo built> out.txt", "echo second step"]}}
skills:
  - path: skills/hello
"#
    ));
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    let head = git(&env.remote, &["rev-parse", "HEAD"]);
    let names = vec![
        (remote, "<REMOTE>"),
        (head, "<HEAD>"),
        (sha, "<SOURCE_SHA>"),
        (url, "<SOURCE>"),
    ];
    (env, names)
}

/// Output with temp paths and commits replaced by placeholders, and the build dir of
/// the git source cut down to `<DIR>`.
fn normalize(text: &str, names: &[(String, &'static str)]) -> String {
    let mut text = text.replace('\\', "/");
    for (value, name) in names {
        text = text.replace(&value.replace('\\', "/"), name);
    }
    let lines: Vec<String> = text.lines().map(stable_dir).collect();
    lines.join("\n") + "\n"
}

fn stable_dir(line: &str) -> String {
    let prefix = "[stdio: node ";
    let (Some(start), Some(end)) = (line.find(prefix), line.find("/out.txt]")) else {
        return line.to_string();
    };
    let from = start + prefix.len();
    format!("{}<DIR>{}", &line[..from], &line[end..])
}

#[test]
fn status_and_diff_output_is_stable() {
    let (env, names) = setup();
    assert_eq!(normalize(&env.ok(&["status"]), &names), STATUS);
    assert_eq!(normalize(&env.ok(&["diff"]), &names), DIFF);
}
