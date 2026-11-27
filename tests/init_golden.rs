//! Golden stdout and stderr of `agsync init` without a terminal, against a fake `codex`:
//! a reachable repo with and without `--silent-update`, an unreachable repo, and the
//! url and profile questions read from stdin.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::{Env, fake_bin, git, path_with};

const SILENT_OUT: &str = "[1/5] machine.toml
gravado <ROOT>/config/machine.toml
perfis ativos: work
[2/5] acesso ao repo
repo ok: <REMOTE> (main em <HEAD>)
[3/5] agendamento do check
agendado: `<EXE> check` a cada 15 min
[4/5] hook de shell
sem TTY: hook de shell nao instalado; rode `agsync hook install --yes`
[5/5] primeiro apply
atualizacao silenciosa: aplicando sem perguntar
codex:
  mcp echo: adicionar [stdio: node]
pulados:
  claude: claude nao encontrado no PATH
  opencode: opencode nao encontrado no PATH
  hermes: hermes nao encontrado no PATH
  antigravity: agy nao encontrado no PATH
codex: mcp echo: ok (adicionar)
Mudancas de MCP so valem apos reiniciar o agente.
";

const NO_TTY_OUT: &str = "[1/5] machine.toml
gravado <ROOT>/config/machine.toml
perfis ativos: work
[2/5] acesso ao repo
repo ok: <REMOTE> (main em <HEAD>)
[3/5] agendamento do check
agendado: `<EXE> check` a cada 15 min
[4/5] hook de shell
sem TTY: hook de shell nao instalado; rode `agsync hook install --yes`
[5/5] primeiro apply
pulado (sem TTY): rode `agsync apply --yes`
";

const UNREACHABLE_OUT: &str = "[1/5] machine.toml
gravado <ROOT>/config/machine.toml
perfis ativos: work, ci
[2/5] acesso ao repo
[3/5] agendamento do check
agendado: `<EXE> check` a cada 15 min
[4/5] hook de shell
sem TTY: hook de shell nao instalado; rode `agsync hook install --yes`
[5/5] primeiro apply
pulado: repo inacessivel; corrija e rode `agsync apply`
";

const ASKED_OUT: &str = "URL do repo de config: URL do repo de config: Perfis desta maquina (separados por virgula): [1/5] machine.toml
gravado <ROOT>/config/machine.toml
perfis ativos: work, personal
[2/5] acesso ao repo
repo ok: <REMOTE> (main em <HEAD>)
[3/5] agendamento do check
agendado: `<EXE> check` a cada 15 min
[4/5] hook de shell
sem TTY: hook de shell nao instalado; rode `agsync hook install --yes`
[5/5] primeiro apply
pulado (sem TTY): rode `agsync apply --yes`
";

const NO_ANSWER_OUT: &str = "URL do repo de config: \n";

const NO_ANSWER_ERR: &str = "erro: entrada encerrada sem resposta para \"URL do repo de config\"\n";

struct Run {
    ok: bool,
    out: String,
    err: String,
}

fn setup(yaml: &str) -> (Env, String, Vec<(String, &'static str)>) {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars
        .push(("FAKE_AGENT_LOG".into(), log.to_string_lossy().to_string()));
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(yaml);
    let remote = env.remote.to_string_lossy().to_string();
    let head = git(&env.remote, &["rev-parse", "HEAD"]);
    let names = names(&env, vec![(remote.clone(), "<REMOTE>"), (head, "<HEAD>")]);
    (env, remote, names)
}

fn names(env: &Env, mut names: Vec<(String, &'static str)>) -> Vec<(String, &'static str)> {
    let exe = std::fs::canonicalize(env!("CARGO_BIN_EXE_agsync")).unwrap();
    names.push((exe.to_string_lossy().to_string(), "<EXE>"));
    names.push((env!("CARGO_BIN_EXE_agsync").to_string(), "<EXE>"));
    names.push((env.root.path().to_string_lossy().to_string(), "<ROOT>"));
    names
}

/// agsync with stdout and stderr kept apart, `input` on stdin when given.
fn run(env: &Env, args: &[&str], input: Option<&str>, names: &[(String, &'static str)]) -> Run {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_agsync"));
    cmd.args(args);
    env.isolate(&mut cmd);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().unwrap();
    if let Some(input) = input {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(input.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    Run {
        ok: out.status.success(),
        out: normalize(&String::from_utf8_lossy(&out.stdout), names),
        err: normalize(&String::from_utf8_lossy(&out.stderr), names),
    }
}

/// Temp paths, the binary and commits replaced by placeholders.
fn normalize(text: &str, names: &[(String, &'static str)]) -> String {
    let mut text = text.replace("\r\n", "\n").replace('\\', "/");
    for (value, name) in names {
        text = text.replace(&value.replace('\\', "/"), name);
    }
    text.replace("//?/", "")
}

#[test]
fn init_silent_update_output_is_stable() {
    let (env, remote, names) = setup("version: 1\nmcps:\n  echo:\n    command: node\n");
    let args = ["init", &remote, "--profile", "work", "--silent-update"];
    let run = run(&env, &args, None, &names);
    assert!(run.ok, "{}{}", run.out, run.err);
    assert_eq!(run.out, SILENT_OUT);
    assert_eq!(run.err, "");
}

#[test]
fn init_without_a_tty_output_is_stable() {
    let (env, remote, names) = setup("version: 1\nmcps:\n  echo:\n    command: node\n");
    let run = run(&env, &["init", &remote, "--profile", "work"], None, &names);
    assert!(run.ok, "{}{}", run.out, run.err);
    assert_eq!(run.out, NO_TTY_OUT);
    assert_eq!(run.err, "");
}

#[test]
fn init_unreachable_repo_output_is_stable() {
    let env = Env::new();
    let gone = env.root.path().join("no-such-repo");
    let url = gone.to_string_lossy().to_string();
    let names = names(&env, Vec::new());
    let args = ["init", &url, "--profile", "work", "--profile", "ci"];
    let run = run(&env, &args, None, &names);
    assert!(!run.ok, "{}{}", run.out, run.err);
    assert_eq!(run.out, UNREACHABLE_OUT);
    let failed = "repo inacessivel: git ls-remote falhou: fatal: '<ROOT>/no-such-repo'";
    assert!(run.err.starts_with(failed), "{}", run.err);
    assert_eq!(
        run.err.matches("repo inacessivel").count(),
        1,
        "{}",
        run.err
    );
}

#[test]
fn init_questions_output_is_stable() {
    let (env, remote, names) = setup("version: 1\n");
    let input = format!("\n{remote}\nwork, personal\n");
    let run = run(&env, &["init"], Some(&input), &names);
    assert!(run.ok, "{}{}", run.out, run.err);
    assert_eq!(run.out, ASKED_OUT);
    assert_eq!(run.err, "");
}

#[test]
fn init_without_answers_output_is_stable() {
    let env = Env::new();
    let names = names(&env, Vec::new());
    let run = run(&env, &["init"], Some(""), &names);
    assert!(!run.ok);
    assert_eq!(run.out, NO_ANSWER_OUT);
    assert_eq!(run.err, NO_ANSWER_ERR);
}
