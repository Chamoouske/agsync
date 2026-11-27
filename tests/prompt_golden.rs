//! Golden output of the confirmation questions without a terminal and with `--yes`,
//! against a fake `codex`.

mod common;

use common::{Env, fake_bin, git, path_with};

const PLAN: &str = "codex:
  mcp plain: adicionar [stdio: node v1.js]
pulados:
  claude: claude nao encontrado no PATH
  opencode: opencode nao encontrado no PATH
  hermes: hermes nao encontrado no PATH
  antigravity: agy nao encontrado no PATH
";

const APPLY_NO_TTY: &str = "erro: sem TTY: nada feito; rode `agsync apply --yes`
";

const APPLY_YES: &str = "codex: mcp plain: ok (adicionar)
Mudancas de MCP so valem apos reiniciar o agente.
";

const HOOK_NO_TTY: &str =
    "sem TTY: hook de shell nao instalado; rode `agsync hook install --yes`\n";

const UNINSTALL_NO_TTY: &str = "agsync uninstall vai:
  remover dos agentes cada mcp, skill e plugin do state.json
  remover o agendamento do `agsync check` e o hook de todos os shells
  apagar state.json e pending.json
erro: sem TTY: nada feito; rode `agsync uninstall --yes`
";

fn setup() -> Env {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars
        .push(("FAKE_AGENT_LOG".into(), log.to_string_lossy().to_string()));
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(
        "version: 1\nprofiles: [work]\nmcps:\n  plain:\n    command: node\n    args: [\"v1.js\"]\n",
    );
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    env
}

fn run(env: &Env, args: &[&str]) -> (bool, String) {
    let (ok, out) = env.agsync(args);
    (ok, out.replace("\r\n", "\n"))
}

#[test]
fn confirmation_output_is_stable() {
    let env = setup();
    assert_eq!(
        run(&env, &["apply"]),
        (false, format!("{PLAN}{APPLY_NO_TTY}"))
    );
    assert_eq!(
        run(&env, &["hook", "install"]),
        (true, HOOK_NO_TTY.to_string())
    );
    assert_eq!(
        run(&env, &["uninstall"]),
        (false, UNINSTALL_NO_TTY.to_string())
    );
    assert_eq!(
        run(&env, &["apply", "--yes"]),
        (true, format!("{PLAN}{APPLY_YES}"))
    );
}
