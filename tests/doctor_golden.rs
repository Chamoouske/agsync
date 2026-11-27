//! Golden output of `agsync doctor` against a fake `codex` and a config repo with an MCP
//! with a found secret and one with a missing secret, and no schedule, so ok, info,
//! warning and problem lines show up.

mod common;

use common::{Env, fake_bin, git, path_with};

const DOCTOR: &str = "ok      git version <VERSION>
info    agsync <VERSION>
info    agente claude: claude nao encontrado no PATH
ok      agente codex detectado
info    agente opencode: opencode nao encontrado no PATH
info    agente hermes: hermes nao encontrado no PATH
info    agente antigravity: agy nao encontrado no PATH
info    <ROOT>/config/secrets.env nao existe
AVISO   agendamento ausente; o `agsync hook` dispara o check quando o shell abre; rode `agsync schedule install`
ok      repo de config: <REMOTE> (main = <HEAD>)
ok      agsync.yaml valido
ok      mcp found: segredo E2E_DOCTOR_FOUND (env)
PROBLEMA mcp missing: segredo E2E_DOCTOR_MISSING ausente; defina com `agsync secret set E2E_DOCTOR_MISSING` ou exporte E2E_DOCTOR_MISSING
1 problema(s)
";

const YAML: &str = r#"version: 1
profiles: [work]
mcps:
  found:
    command: node
    env: {TOKEN: "${E2E_DOCTOR_FOUND}"}
  missing:
    command: node
    env: {TOKEN: "${E2E_DOCTOR_MISSING}"}
"#;

fn setup() -> (Env, Vec<(String, &'static str)>) {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars
        .push(("FAKE_AGENT_LOG".into(), log.to_string_lossy().to_string()));
    env.vars
        .push(("E2E_DOCTOR_FOUND".into(), "found-value".into()));
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(YAML);
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    env.ok(&["schedule", "remove"]);
    let head = git(&env.remote, &["rev-parse", "HEAD"]);
    let root = env.root.path().to_string_lossy().to_string();
    let names = vec![(remote, "<REMOTE>"), (head, "<HEAD>"), (root, "<ROOT>")];
    (env, names)
}

/// Output with temp paths and commits replaced by placeholders, and the git and
/// agsync versions cut down to `<VERSION>`.
fn normalize(text: &str, names: &[(String, &'static str)]) -> String {
    let mut text = text.replace('\\', "/");
    for (value, name) in names {
        text = text.replace(&value.replace('\\', "/"), name);
    }
    let lines: Vec<String> = text.lines().map(stable_version).collect();
    lines.join("\n") + "\n"
}

fn stable_version(line: &str) -> String {
    for prefix in ["ok      git version ", "info    agsync "] {
        if line.starts_with(prefix) {
            return format!("{prefix}<VERSION>");
        }
    }
    line.to_string()
}

#[test]
fn doctor_output_is_stable() {
    let (env, names) = setup();
    let (ok, out) = env.agsync(&["doctor"]);
    assert!(!ok, "{out}");
    assert_eq!(normalize(&out, &names), DOCTOR);
}
