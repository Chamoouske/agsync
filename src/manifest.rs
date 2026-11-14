//! `agsync.yaml`: parsing, validation and profile selection.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde_saphyr::Spanned;

/// Root of `agsync.yaml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub mcps: BTreeMap<String, Spanned<Mcp>>,
    #[serde(default)]
    pub skills: Vec<Spanned<Skill>>,
}

/// One MCP server entry.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mcp {
    pub profiles: Option<Vec<String>>,
    pub agents: Option<Vec<String>>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<BTreeMap<String, String>>,
    pub source: Option<Source>,
}

/// Git source to build an MCP server from.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub git: String,
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub build: Vec<String>,
}

/// One skill entry: a folder in the config repo or in another git repo.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skill {
    pub path: Option<String>,
    pub name: Option<String>,
    pub profiles: Option<Vec<String>>,
    pub agents: Option<Vec<String>>,
    pub git: Option<String>,
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
}

impl Manifest {
    /// Parses and validates the YAML text. Errors carry the line number.
    pub fn parse(text: &str) -> anyhow::Result<Manifest> {
        let manifest: Manifest =
            serde_saphyr::from_str(text).map_err(|e| anyhow::anyhow!("agsync.yaml: {e}"))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Checks that every local skill folder exists in `root` and has a `SKILL.md`.
    pub fn check_skill_dirs(&self, root: &Path) -> anyhow::Result<()> {
        for skill in &self.skills {
            let Some(path) = &skill.value.path else {
                continue;
            };
            if skill.value.git.is_some() {
                continue;
            }
            if !root.join(path).join("SKILL.md").is_file() {
                anyhow::bail!(
                    "agsync.yaml linha {}: skill '{path}' sem SKILL.md no repo",
                    skill.defined.line()
                );
            }
        }
        Ok(())
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.version != 1 {
            anyhow::bail!(
                "agsync.yaml: version {} nao suportada (use 1)",
                self.version
            );
        }
        for (name, mcp) in &self.mcps {
            let line = mcp.defined.line();
            self.validate_mcp(name, &mcp.value)
                .map_err(|e| anyhow::anyhow!("agsync.yaml linha {line}: mcp '{name}': {e}"))?;
        }
        let mut names = BTreeMap::new();
        for skill in &self.skills {
            let line = skill.defined.line();
            let name = self
                .validate_skill(&skill.value)
                .map_err(|e| anyhow::anyhow!("agsync.yaml linha {line}: skill: {e}"))?;
            let git_mcp = self
                .mcps
                .get(&name)
                .is_some_and(|m| m.value.source.is_some());
            if skill.value.git.is_some() && git_mcp {
                anyhow::bail!(
                    "agsync.yaml linha {line}: skill '{name}' tem o mesmo nome de um mcp com source; use name"
                );
            }
            if let Some(first) = names.insert(name.clone(), line) {
                anyhow::bail!(
                    "agsync.yaml linha {line}: skill '{name}' repetida (ja definida na linha {first})"
                );
            }
        }
        Ok(())
    }

    fn validate_mcp(&self, name: &str, mcp: &Mcp) -> anyhow::Result<()> {
        check_name(name)?;
        self.check_profiles(&mcp.profiles)?;
        check_agents(&mcp.agents)?;
        let is_stdio = mcp.command.is_some();
        let is_remote = mcp.url.is_some();
        if is_stdio == is_remote {
            anyhow::bail!("defina exatamente um entre command e url");
        }
        if is_stdio && mcp.headers.is_some() {
            anyhow::bail!("headers so vale com url");
        }
        if is_remote && (mcp.args.is_some() || mcp.env.is_some()) {
            anyhow::bail!("args e env so valem com command");
        }
        if let Some(source) = &mcp.source {
            check_ref(source.git_ref.as_deref(), "source.git")?;
        }
        Ok(())
    }

    fn validate_skill(&self, skill: &Skill) -> anyhow::Result<String> {
        self.check_profiles(&skill.profiles)?;
        check_agents(&skill.agents)?;
        if skill.git.is_some() {
            check_ref(skill.git_ref.as_deref(), "git")?;
        }
        if skill.path.is_none() {
            anyhow::bail!("defina path (pasta no repo de config ou no repo git)");
        }
        if skill.git.is_none() && skill.git_ref.is_some() {
            anyhow::bail!("ref so vale com git");
        }
        if let Some(path) = &skill.path {
            check_relative(path)?;
        }
        let name = skill.name();
        check_name(&name)?;
        Ok(name)
    }

    fn check_profiles(&self, profiles: &Option<Vec<String>>) -> anyhow::Result<()> {
        for profile in profiles.iter().flatten() {
            if !self.profiles.contains(profile) {
                anyhow::bail!("perfil '{profile}' nao existe em profiles");
            }
        }
        Ok(())
    }
}

impl Mcp {
    /// True when the server is built from a git source.
    pub fn has_git_source(&self) -> bool {
        self.source.is_some()
    }
}

impl Skill {
    /// `name` if given, else the last segment of `path` or of the git url.
    pub fn name(&self) -> String {
        if let Some(name) = &self.name {
            return name.clone();
        }
        let base = self.path.as_deref().or(self.git.as_deref()).unwrap_or("");
        let last = base.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        last.trim_end_matches(".git").to_string()
    }
}

/// True if the item has no profiles or one of them is active on this machine.
pub fn selected(item_profiles: &Option<Vec<String>>, active: &[String]) -> bool {
    match item_profiles {
        None => true,
        Some(list) if list.is_empty() => true,
        Some(list) => list.iter().any(|p| active.contains(p)),
    }
}

/// True if the item targets `agent` (no `agents` means every agent).
pub fn targets(item_agents: &Option<Vec<String>>, agent: &str) -> bool {
    match item_agents {
        None => true,
        Some(list) => list.iter().any(|a| a == agent),
    }
}

fn check_agents(agents: &Option<Vec<String>>) -> anyhow::Result<()> {
    let Some(list) = agents else {
        return Ok(());
    };
    if list.is_empty() {
        anyhow::bail!("agents vazio; omita para usar todos");
    }
    for agent in list {
        if !crate::agent::names().any(|known| known == agent) {
            let known: Vec<&str> = crate::agent::names().collect();
            anyhow::bail!("agente '{agent}' desconhecido (use {})", known.join(", "));
        }
    }
    Ok(())
}

fn check_ref(git_ref: Option<&str>, field: &str) -> anyhow::Result<()> {
    let Some(git_ref) = git_ref else {
        anyhow::bail!("{field} exige ref");
    };
    if crate::git::is_short_sha(git_ref) {
        anyhow::bail!("ref '{git_ref}' parece SHA abreviado; use o SHA completo");
    }
    Ok(())
}

fn check_name(name: &str) -> anyhow::Result<()> {
    let valid_chars = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if name.is_empty() || !valid_chars {
        anyhow::bail!("nome '{name}' invalido (use letras, numeros, - e _)");
    }
    Ok(())
}

fn check_relative(path: &str) -> anyhow::Result<()> {
    let p = Path::new(path);
    let escapes = p
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)));
    if path.is_empty() || escapes {
        anyhow::bail!("path '{path}' precisa ser relativo e dentro do repo");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
version: 1
profiles: [work, personal]
mcps:
  echo:
    command: npx
    args: ["-y", "pkg"]
    env: {TOKEN: "${TOKEN}"}
  remote:
    profiles: [work]
    agents: [claude]
    url: https://example.com/mcp
    headers: {Authorization: "Bearer x"}
  built:
    command: node
    source:
      git: https://example.com/repo.git
      ref: v1.0.0
      requires: [node]
      build: ["npm ci"]
skills:
  - path: skills/hello
  - path: skills/x
    name: other
    profiles: [personal]
    agents: [claude]
  - git: https://example.com/skills.git
    ref: v2.0.0
    path: sub/pasta
"#;

    fn err(text: &str) -> String {
        Manifest::parse(text).unwrap_err().to_string()
    }

    #[test]
    fn parses_full_example() {
        let m = Manifest::parse(FULL).unwrap();
        assert_eq!(m.profiles, ["work", "personal"]);
        assert_eq!(m.mcps.len(), 3);
        let echo = &m.mcps["echo"].value;
        assert_eq!(echo.env.as_ref().unwrap()["TOKEN"], "${TOKEN}");
        assert!(m.mcps["built"].value.has_git_source());
        let names: Vec<_> = m.skills.iter().map(|s| s.value.name()).collect();
        assert_eq!(names, ["hello", "other", "pasta"]);
    }

    #[test]
    fn unknown_field_is_error_with_line() {
        let text = "version: 1\nmcps:\n  a:\n    command: x\n    bogus: 1\n";
        let msg = err(text);
        assert!(msg.contains("bogus"), "{msg}");
        assert!(msg.contains("line 5"), "{msg}");
    }

    #[test]
    fn unknown_top_level_field_is_error() {
        assert!(err("version: 1\nextra: true\n").contains("extra"));
    }

    #[test]
    fn unknown_profile_is_error_with_line() {
        let text =
            "version: 1\nprofiles: [work]\nmcps:\n  a:\n    profiles: [home]\n    command: x\n";
        let msg = err(text);
        assert!(msg.contains("linha 5"), "{msg}");
        assert!(msg.contains("home"), "{msg}");
    }

    #[test]
    fn command_and_url_are_exclusive() {
        let both = "version: 1\nmcps:\n  a:\n    command: x\n    url: https://x\n";
        assert!(err(both).contains("exatamente um"));
        let none = "version: 1\nmcps:\n  a:\n    args: [x]\n";
        assert!(err(none).contains("exatamente um"));
    }

    #[test]
    fn git_requires_ref() {
        let mcp = "version: 1\nmcps:\n  a:\n    command: x\n    source:\n      git: https://x\n";
        assert!(err(mcp).contains("exige ref"));
        let skill = "version: 1\nskills:\n  - git: https://x\n";
        assert!(err(skill).contains("exige ref"));
    }

    #[test]
    fn rejects_bad_agent_name_and_path() {
        let agent = "version: 1\nmcps:\n  a:\n    command: x\n    agents: [vim]\n";
        assert!(err(agent).contains("vim"));
        let name = "version: 1\nmcps:\n  'a b':\n    command: x\n";
        assert!(err(name).contains("invalido"));
        let path = "version: 1\nskills:\n  - path: ../fora\n";
        assert!(err(path).contains("relativo"));
    }

    #[test]
    fn git_skill_needs_path_and_its_own_name() {
        let no_path = "version: 1\nskills:\n  - git: https://x/s.git\n    ref: v1\n";
        assert!(err(no_path).contains("defina path"));
        let clash = r#"version: 1
mcps:
  s:
    command: x
    source: {git: "https://x", ref: v1}
skills:
  - git: https://x/s.git
    ref: v1
    path: a/s
"#;
        assert!(err(clash).contains("mesmo nome"));
    }

    #[test]
    fn rejects_duplicate_skill_names_and_bad_version() {
        let dup = "version: 1\nskills:\n  - path: a/x\n  - path: b/x\n";
        assert!(err(dup).contains("repetida"));
        assert!(err("version: 2\n").contains("version"));
    }

    #[test]
    fn checks_skill_md_exists() {
        let dir = crate::tmp::TempDir::new("agsync-test").unwrap();
        std::fs::create_dir_all(dir.path().join("skills/hello")).unwrap();
        let m = Manifest::parse("version: 1\nskills:\n  - path: skills/hello\n").unwrap();
        assert!(
            m.check_skill_dirs(dir.path())
                .unwrap_err()
                .to_string()
                .contains("SKILL.md")
        );
        std::fs::write(dir.path().join("skills/hello/SKILL.md"), "x").unwrap();
        m.check_skill_dirs(dir.path()).unwrap();
    }

    #[test]
    fn profile_selection() {
        let active = vec!["work".to_string(), "oss".to_string()];
        assert!(selected(&None, &active));
        assert!(selected(&Some(vec![]), &active));
        assert!(selected(
            &Some(vec!["personal".into(), "oss".into()]),
            &active
        ));
        assert!(!selected(&Some(vec!["personal".into()]), &active));
        assert!(!selected(&Some(vec!["work".into()]), &[]));
    }

    #[test]
    fn agent_targeting() {
        assert!(targets(&None, "codex"));
        assert!(targets(&Some(vec!["claude".into()]), "claude"));
        assert!(!targets(&Some(vec!["claude".into()]), "codex"));
    }
}
