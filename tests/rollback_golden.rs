//! Golden output of `agsync rollback` against a fake `codex`: without a previous commit,
//! without a terminal, and with `--yes`.

mod common;

use common::{Env, fake_bin, git, path_with};

const NO_PREVIOUS: &str = "erro: sem commit anterior em <ROOT>/data/state.json: o rollback precisa de dois applies de commits diferentes
";

const NO_TTY: &str = "rollback: <SECOND> -> <FIRST>
codex:
  mcp plain: atualizar [stdio: node v1.js]
pulados:
  claude: claude nao encontrado no PATH
  opencode: opencode nao encontrado no PATH
  hermes: hermes nao encontrado no PATH
  antigravity: agy nao encontrado no PATH
erro: sem TTY: nada feito; rode `agsync rollback --yes`
";

const YES: &str = "rollback: <SECOND> -> <FIRST>
codex:
  mcp plain: atualizar [stdio: node v1.js]
pulados:
  claude: claude nao encontrado no PATH
  opencode: opencode nao encontrado no PATH
  hermes: hermes nao encontrado no PATH
  antigravity: agy nao encontrado no PATH
codex: mcp plain: ok (atualizar)
Mudancas de MCP so valem apos reiniciar o agente.
aplicado o commit anterior <FIRST>
o `agsync check` volta a acusar o commit remoto como pendente; isso e esperado. Para voltar a ele: `agsync apply`
";

fn manifest(arg: &str) -> String {
    format!("version: 1\nmcps:\n  plain:\n    command: node\n    args: [\"{arg}\"]\n")
}

fn rollback(env: &Env, args: &[&str], names: &[(String, &'static str)]) -> (bool, String) {
    let (ok, out) = env.agsync(args);
    let mut text = out.replace("\r\n", "\n").replace('\\', "/");
    for (value, name) in names {
        text = text.replace(&value.replace('\\', "/"), name);
    }
    (ok, text)
}

#[test]
fn rollback_output_is_stable() {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars
        .push(("FAKE_AGENT_LOG".into(), log.to_string_lossy().to_string()));
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&manifest("v1.js"));
    let first = git(&env.remote, &["rev-parse", "HEAD"]);
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    env.ok(&["apply", "--yes"]);
    let root = env.root.path().to_string_lossy().to_string();
    let mut names = vec![(root, "<ROOT>"), (first, "<FIRST>")];

    let expected = (false, NO_PREVIOUS.to_string());
    assert_eq!(rollback(&env, &["rollback", "--yes"], &names), expected);

    env.push(&manifest("v2.js"));
    names.push((git(&env.remote, &["rev-parse", "HEAD"]), "<SECOND>"));
    env.ok(&["apply", "--yes"]);

    let expected = (false, NO_TTY.to_string());
    assert_eq!(rollback(&env, &["rollback"], &names), expected);
    let expected = (true, YES.to_string());
    assert_eq!(rollback(&env, &["rollback", "--yes"], &names), expected);
}
