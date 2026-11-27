//! Golden output of `agsync apply --yes` against a fake `codex`: builds that pass and
//! fail, written and removed items, a manual conflict, nothing to apply and a cache
//! cleanup.

mod common;

use common::{Env, fake_bin, git, path_with};

const FIRST: &str = "codex:
  mcp built: adicionar [stdio: node]
  mcp manual: conflito com instalacao manual de mesmo nome, nao sera alterada (use --adopt para substituir)
  mcp plain: adicionar [stdio: node v1.js]
  skill hello: adicionar

instalacoes manuais com o mesmo nome no agsync.yaml (`agsync apply --adopt` substitui):
  codex: mcp manual

!!! BUILDS: estes comandos executam codigo de terceiros nesta maquina !!!
  mcp built: <SOURCE> @ <SOURCE_SHA> (main)
    $ echo built

pulados:
  claude: claude nao encontrado no PATH
  opencode: opencode nao encontrado no PATH
  hermes: hermes nao encontrado no PATH
  antigravity: agy nao encontrado no PATH
build mcp built (<SOURCE_SHA>)...
build mcp built: ok
codex: mcp built: ok (adicionar)
codex: mcp plain: ok (adicionar)
codex: skill hello: ok (adicionar)
Mudancas de MCP so valem apos reiniciar o agente.
";

const NOTHING: &str = "codex:
  mcp manual: conflito com instalacao manual de mesmo nome, nao sera alterada (use --adopt para substituir)

instalacoes manuais com o mesmo nome no agsync.yaml (`agsync apply --adopt` substitui):
  codex: mcp manual
pulados:
  claude: claude nao encontrado no PATH
  opencode: opencode nao encontrado no PATH
  hermes: hermes nao encontrado no PATH
  antigravity: agy nao encontrado no PATH
nada a aplicar
";

const BROKEN: &str = "codex:
  mcp broken: adicionar [stdio: node]
  mcp manual: conflito com instalacao manual de mesmo nome, nao sera alterada (use --adopt para substituir)
  mcp plain: remover (saiu do agsync.yaml)

instalacoes manuais com o mesmo nome no agsync.yaml (`agsync apply --adopt` substitui):
  codex: mcp manual

!!! BUILDS: estes comandos executam codigo de terceiros nesta maquina !!!
  mcp broken: <SOURCE> @ <SOURCE_SHA> (main)
    $ exit 7

pulados:
  claude: claude nao encontrado no PATH
  opencode: opencode nao encontrado no PATH
  hermes: hermes nao encontrado no PATH
  antigravity: agy nao encontrado no PATH
build mcp broken (<SOURCE_SHA>)...
codex: mcp plain: ok (remover (saiu do agsync.yaml))
Mudancas de MCP so valem apos reiniciar o agente.
mcp broken: build falhou em 'exit 7' (exit 7); log: <LOG>; item pulado
erro: 1 item(ns) falharam; os demais foram aplicados
";

const CLEANED: &str = "codex:
  mcp built: remover (saiu do agsync.yaml)
  mcp manual: conflito com instalacao manual de mesmo nome, nao sera alterada (use --adopt para substituir)

instalacoes manuais com o mesmo nome no agsync.yaml (`agsync apply --adopt` substitui):
  codex: mcp manual
pulados:
  claude: claude nao encontrado no PATH
  opencode: opencode nao encontrado no PATH
  hermes: hermes nao encontrado no PATH
  antigravity: agy nao encontrado no PATH
codex: mcp built: ok (remover (saiu do agsync.yaml))
limpeza: 2 pasta(s) sem uso removida(s)
Mudancas de MCP so valem apos reiniciar o agente.
";

const CODEX_FIXTURE: &str = r#"[mcp_servers.manual]
command = "manual-tool"
args = ["mine"]
"#;

fn source(env: &Env) -> (String, String) {
    let dir = env.root.path().join("source");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("README"), "x\n").unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    let sha = git(&dir, &["rev-parse", "HEAD"]);
    (dir.to_string_lossy().replace('\\', "/"), sha)
}

fn manifest(url: &str, mcps: &str) -> String {
    format!(
        "version: 1\nprofiles: [work]\nmcps:\n  manual:\n    command: node\n    args: [\"repo.js\"]\n{}skills:\n  - path: skills/hello\n",
        mcps.replace("<URL>", url)
    )
}

const PLAIN: &str = "  plain:\n    command: node\n    args: [\"v1.js\"]\n";

const BUILT: &str = "  built:\n    command: node\n    source: {git: \"<URL>\", ref: main, build: [\"echo built\"]}\n";

const BROKEN_MCP: &str =
    "  broken:\n    command: node\n    source: {git: \"<URL>\", ref: main, build: [\"exit 7\"]}\n";

fn setup() -> (Env, String, Vec<(String, &'static str)>) {
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
    env.push(&manifest(&url, &format!("{PLAIN}{BUILT}")));
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    let root = env.root.path().to_string_lossy().to_string();
    let names = vec![
        (sha, "<SOURCE_SHA>"),
        (url.clone(), "<SOURCE>"),
        (root, "<ROOT>"),
    ];
    (env, url, names)
}

fn apply(env: &Env, names: &[(String, &'static str)]) -> (bool, String) {
    let (ok, out) = env.agsync(&["apply", "--yes"]);
    let mut text = out.replace("\r\n", "\n").replace('\\', "/");
    for (value, name) in names {
        text = text.replace(&value.replace('\\', "/"), name);
    }
    let lines: Vec<String> = text.lines().map(stable_log).collect();
    (ok, lines.join("\n") + "\n")
}

/// The log of a failed build as `<LOG>`: its file name has a timestamp.
fn stable_log(line: &str) -> String {
    let (Some(start), Some(end)) = (line.find("log: "), line.find("; item pulado")) else {
        return line.to_string();
    };
    format!("{}<LOG>{}", &line[..start + 5], &line[end..])
}

#[test]
fn apply_output_is_stable() {
    let (env, url, names) = setup();
    assert_eq!(apply(&env, &names), (true, FIRST.to_string()));
    assert_eq!(apply(&env, &names), (true, NOTHING.to_string()));
    env.push(&manifest(&url, &format!("{BUILT}{BROKEN_MCP}")));
    assert_eq!(apply(&env, &names), (false, BROKEN.to_string()));
    env.push(&manifest(&url, ""));
    assert_eq!(apply(&env, &names), (true, CLEANED.to_string()));
}
