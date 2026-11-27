//! `agsync doctor`: checks what apply needs on this machine.

use serde::Serialize;

use crate::agent;
use crate::git::{self, Snapshot};
use crate::machine::Machine;
use crate::manifest::{Manifest, selected};
use crate::mcp::McpSpec;
use crate::paths::Dirs;
use crate::schedule::{self, Status};
use crate::secret::{self, Secrets};
use crate::source;

/// How bad one check is; only `Problem` makes the exit code 1.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    Info,
    Warn,
    Problem,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Ok => "ok      ",
            Level::Info => "info    ",
            Level::Warn => "AVISO   ",
            Level::Problem => "PROBLEMA ",
        }
    }
}

/// One line of `agsync doctor`.
#[derive(Serialize, Debug)]
pub struct Check {
    pub level: Level,
    pub text: String,
}

/// Every check in run order. `complete` is false when doctor stopped early, either on
/// an unreadable machine file or on `error`.
#[derive(Serialize, Default, Debug)]
pub struct Report {
    pub checks: Vec<Check>,
    pub complete: bool,
    pub error: Option<String>,
}

impl Report {
    /// Number of `Problem` checks.
    pub fn problems(&self) -> usize {
        let problems = self.checks.iter().filter(|c| c.level == Level::Problem);
        problems.count()
    }

    fn push(&mut self, level: Level, text: &str) {
        let text = text.to_string();
        self.checks.push(Check { level, text });
    }

    fn ok(&mut self, text: &str) {
        self.push(Level::Ok, text);
    }

    fn info(&mut self, text: &str) {
        self.push(Level::Info, text);
    }

    fn warn(&mut self, text: &str) {
        self.push(Level::Warn, text);
    }

    fn problem(&mut self, text: &str) {
        self.push(Level::Problem, text);
    }
}

/// Runs every check and prints one line each. Returns true if nothing is wrong.
pub fn doctor() -> anyhow::Result<bool> {
    let report = report();
    print(&report);
    if let Some(error) = &report.error {
        anyhow::bail!("{error}");
    }
    Ok(report.problems() == 0)
}

/// Runs every check without printing.
pub fn report() -> Report {
    let mut report = Report::default();
    match run(&mut report) {
        Ok(complete) => report.complete = complete,
        Err(e) => report.error = Some(format!("{e:#}")),
    }
    report
}

fn print(report: &Report) {
    for check in &report.checks {
        println!("{}{}", check.level.label(), check.text);
    }
    if !report.complete {
        return;
    }
    match report.problems() {
        0 => println!("tudo certo"),
        problems => println!("{problems} problema(s)"),
    }
}

fn run(checks: &mut Report) -> anyhow::Result<bool> {
    if let Some(note) = crate::host::current().doctor_note() {
        checks.info(&note);
    }
    check_git(checks);
    let dirs = Dirs::from_env()?;
    check_version(checks, &dirs);
    check_agents(checks);
    check_secrets_file(checks, &dirs);
    check_backups(checks, &dirs);
    check_schedule(checks);
    let machine = match Machine::load(&dirs.machine_file()) {
        Ok(machine) => machine,
        Err(e) => {
            checks.problem(&format!("{e:#}"));
            return Ok(false);
        }
    };
    if let Some(manifest) = check_config_repo(checks, &machine) {
        check_items(checks, &manifest, &machine.profiles, &dirs)?;
    }
    Ok(true)
}

/// Extra hint for git errors: TLS/certificate or authentication.
pub fn hint(error: &str) -> Option<&'static str> {
    let text = error.to_lowercase();
    let tls = ["ssl", "certificate", "tls", "schannel", "issuer"];
    let auth = [
        "authentication failed",
        "could not read username",
        "permission denied",
        "terminal prompts disabled",
        "access denied",
        "401",
        "403",
    ];
    if git::ssh_hint(&text).is_some() {
        return None;
    }
    if tls.iter().any(|w| text.contains(w)) {
        return Some(
            "erro de TLS/certificado: aponte o git para a CA corporativa com \
             `git config --global http.sslCAInfo <ca.pem>` (nunca desligue a verificacao)",
        );
    }
    if auth.iter().any(|w| text.contains(w)) {
        return Some(
            "erro de autenticacao: o agsync roda o git sem prompt de senha (GIT_TERMINAL_PROMPT=0), \
             entao a credencial precisa ja estar salva: configure o credential helper ou um token \
             para este host, ou use chave SSH carregada no ssh-agent; teste com `git ls-remote <url>`",
        );
    }
    None
}

fn check_version(checks: &mut Report, dirs: &Dirs) {
    checks.info(&format!("agsync {}", crate::update::Version::current()));
    if let Some(newer) = crate::update::recorded(dirs) {
        checks.info(&crate::update::line(newer));
    }
}

fn check_git(checks: &mut Report) {
    if let Err(reason) = crate::which::require("git") {
        checks.problem(&reason);
        return;
    }
    match git::git(&["--version"], None) {
        Ok(version) => checks.ok(version.trim()),
        Err(e) => checks.problem(&format!("git --version: {e:#}")),
    }
}

fn check_agents(checks: &mut Report) {
    let mut found = 0;
    for (name, detect) in agent::AGENTS {
        match detect() {
            Ok(adapter) => {
                found += 1;
                checks.ok(&format!("agente {name} detectado"));
                for note in adapter.doctor_notes() {
                    checks.warn(&format!("{name}: {note}"));
                }
            }
            Err(reason) => checks.info(&format!("agente {name}: {reason}")),
        }
    }
    if found == 0 {
        checks.problem("nenhum agente detectado; apply nao teria onde escrever");
    }
}

fn check_secrets_file(checks: &mut Report, dirs: &Dirs) {
    let file = secret::file_path(&dirs.config);
    if !file.is_file() {
        checks.info(&format!("{} nao existe", file.display()));
        return;
    }
    match secret::file::is_private(&file) {
        Ok(true) => checks.ok(&format!("{} acessivel so pelo usuario", file.display())),
        Ok(false) => checks.problem(&format!(
            "{} pode ser lido por outros usuarios; rode `agsync secret set <NOME>` para regravar com permissao restrita",
            file.display()
        )),
        Err(e) => checks.problem(&format!("permissao de {}: {e:#}", file.display())),
    }
}

fn check_backups(checks: &mut Report, dirs: &Dirs) {
    let dir = dirs.backups();
    if !dir.is_dir() {
        return;
    }
    match secret::file::is_private(&dir) {
        Ok(true) => checks.ok(&format!("{} acessivel so pelo usuario", dir.display())),
        Ok(false) => checks.problem(&format!(
            "{} pode ser lida por outros usuarios e guarda copias de configs com segredos; o proximo `agsync apply` com mudancas restringe a pasta",
            dir.display()
        )),
        Err(e) => checks.problem(&format!("permissao de {}: {e:#}", dir.display())),
    }
}

fn check_schedule(checks: &mut Report) {
    let exe = match crate::paths::current_exe() {
        Ok(exe) => exe,
        Err(e) => return checks.problem(&format!("{e:#}")),
    };
    match schedule::for_this_machine().status(&exe) {
        Ok(found @ Status::Current) => checks.ok(&schedule::describe(&found, &exe)),
        Ok(found @ Status::Missing) => checks.warn(&schedule::describe(&found, &exe)),
        Ok(found @ Status::Other(_)) => checks.problem(&schedule::describe(&found, &exe)),
        Err(e) => checks.problem(&format!("agendamento: {e:#}")),
    }
}

fn check_remote(checks: &mut Report, what: &str, url: &str, git_ref: &str) -> Option<String> {
    match git::resolve(url, git_ref) {
        Ok(sha) => {
            checks.ok(&format!("{what}: {url} ({git_ref} = {sha})"));
            Some(sha)
        }
        Err(e) => {
            let text = format!("{e:#}");
            let extra = hint(&text).map(|h| format!("; {h}")).unwrap_or_default();
            checks.problem(&format!("{what}: {url} inacessivel: {text}{extra}"));
            None
        }
    }
}

fn check_config_repo(checks: &mut Report, machine: &Machine) -> Option<Manifest> {
    let sha = check_remote(checks, "repo de config", &machine.repo, &machine.git_ref)?;
    let snapshot = match Snapshot::fetch(&machine.repo, &machine.git_ref, &sha) {
        Ok(snapshot) => snapshot,
        Err(e) => {
            checks.problem(&format!("snapshot do repo de config: {e:#}"));
            return None;
        }
    };
    let text = std::fs::read_to_string(snapshot.path().join("agsync.yaml"));
    let parsed = text
        .map_err(|e| anyhow::anyhow!("agsync.yaml nao encontrado no repo: {e}"))
        .and_then(|text| Manifest::parse(&text));
    let _ = snapshot.close();
    match parsed {
        Ok(manifest) => {
            checks.ok("agsync.yaml valido");
            Some(manifest)
        }
        Err(e) => {
            checks.problem(&format!("{e:#}"));
            None
        }
    }
}

fn check_items(
    checks: &mut Report,
    manifest: &Manifest,
    active: &[String],
    dirs: &Dirs,
) -> anyhow::Result<()> {
    let secrets = Secrets::load(&secret::file_path(&dirs.config))?;
    for (name, mcp) in &manifest.mcps {
        let mcp = &mcp.value;
        if !selected(&mcp.profiles, active) {
            continue;
        }
        if let Some(src) = &mcp.source {
            let git_ref = src.git_ref.as_deref().unwrap_or_default();
            check_remote(checks, &format!("mcp {name}"), &src.git, git_ref);
            let missing = source::missing_requires(&src.requires);
            if missing.is_empty() && !src.requires.is_empty() {
                checks.ok(&format!("mcp {name}: requires {}", src.requires.join(", ")));
            }
            if !missing.is_empty() {
                checks.problem(&format!(
                    "mcp {name}: requires ausente no PATH: {}",
                    missing.join(", ")
                ));
            }
        }
        for var in McpSpec::from_manifest(mcp).secret_refs() {
            match secrets.lookup(&var) {
                Some((origin, _)) => {
                    checks.ok(&format!("mcp {name}: segredo {var} ({})", origin.label()))
                }
                None => checks.problem(&format!(
                    "mcp {name}: {}",
                    crate::deliver::missing_hint(&var)
                )),
            }
        }
    }
    for skill in &manifest.skills {
        let skill = &skill.value;
        let (Some(url), Some(git_ref)) = (&skill.git, &skill.git_ref) else {
            continue;
        };
        if selected(&skill.profiles, active) {
            check_remote(checks, &format!("skill {}", skill.name()), url, git_ref);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_serializes_to_json() {
        let mut report = Report::default();
        report.ok("agente codex detectado");
        report.warn("agendamento ausente");
        report.problem("mcp x: segredo Y ausente");
        report.complete = true;
        assert_eq!(report.problems(), 1);
        let json = serde_json::to_value(&report).unwrap();
        let expected = serde_json::json!({
            "checks": [
                {"level": "ok", "text": "agente codex detectado"},
                {"level": "warn", "text": "agendamento ausente"},
                {"level": "problem", "text": "mcp x: segredo Y ausente"},
            ],
            "complete": true,
            "error": null,
        });
        assert_eq!(json, expected);
    }

    #[test]
    fn hints_for_tls_and_auth() {
        let tls = "git ls-remote falhou: fatal: unable to access 'https://git.corp/x/': SSL certificate problem: unable to get local issuer certificate";
        assert!(hint(tls).unwrap().contains("http.sslCAInfo"));
        assert!(!hint(tls).unwrap().contains("sslVerify"));
        let auth = "fatal: Authentication failed for 'https://git.corp/x/'";
        assert!(hint(auth).unwrap().contains("autenticacao"));
        let no_prompt =
            "fatal: could not read Username for 'https://git.corp': terminal prompts disabled";
        assert!(hint(no_prompt).unwrap().contains("GIT_TERMINAL_PROMPT=0"));
        assert!(hint("fatal: repository not found").is_none());
    }

    #[test]
    fn ssh_failure_shows_the_agent_hint_once() {
        let text = "git ls-remote falhou: git@host: Permission denied (publickey).";
        assert!(git::ssh_hint(text).unwrap().contains("ssh-add"));
        assert!(hint(text).is_none());
    }
}
