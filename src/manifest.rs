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
    #[serde(default)]
    pub plugins: Vec<Spanned<Plugin>>,
    /// `agsync init` applies without asking, as with `--silent-update`.
    #[serde(rename = "silentUpdate", default)]
    pub silent_update: bool,
    #[serde(default)]
    pub imports: Imports,
}

/// Other config repos whose items of one kind join this manifest.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Imports {
    #[serde(default)]
    pub mcps: Vec<Spanned<Import>>,
    #[serde(default)]
    pub skills: Vec<Spanned<Import>>,
    #[serde(default)]
    pub plugins: Vec<Spanned<Import>>,
}

/// One repo to import items of a single kind from, read from its root `agsync.yaml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Import {
    pub git: String,
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    pub profiles: Option<Vec<String>>,
    #[serde(default)]
    pub excludes: Vec<String>,
}

impl Import {
    /// `ref` if given, else `main`, as `agsync init` does.
    pub fn git_ref(&self) -> &str {
        self.git_ref.as_deref().unwrap_or("main")
    }
}

impl Imports {
    /// Each kind with its entries, in a fixed order.
    pub fn kinds(&self) -> [(&'static str, &[Spanned<Import>]); 3] {
        [
            ("mcps", &self.mcps),
            ("skills", &self.skills),
            ("plugins", &self.plugins),
        ]
    }
}

/// One plugin entry from a marketplace.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plugin {
    pub name: String,
    pub profiles: Option<Vec<String>>,
    pub agents: Option<Vec<String>>,
    pub marketplace: Marketplace,
}

/// Where a plugin is published.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marketplace {
    /// Git URL, GitHub shorthand or a local folder with `.claude-plugin`.
    pub git: String,
    /// Required; the default lets [`Manifest::validate_plugin`] name the line.
    #[serde(rename = "ref", default)]
    pub git_ref: String,
}

/// One MCP server entry.
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
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
///
/// A git entry may list several folders in `paths`; [`Manifest::parse`] splits it into
/// one skill per folder, so after parsing `paths` is always `None`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skill {
    pub path: Option<String>,
    pub paths: Option<Vec<Spanned<String>>>,
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
        let mut manifest: Manifest =
            serde_saphyr::from_str(text).map_err(|e| anyhow::anyhow!("agsync.yaml: {e}"))?;
        manifest.skills = split_paths(std::mem::take(&mut manifest.skills))?;
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
        let mut plugin_names = BTreeMap::new();
        let mut marketplaces: BTreeMap<String, (String, u64)> = BTreeMap::new();
        for plugin in &self.plugins {
            let line = plugin.defined.line();
            let plugin = &plugin.value;
            self.validate_plugin(plugin).map_err(|e| {
                anyhow::anyhow!("agsync.yaml linha {line}: plugin '{}': {e}", plugin.name)
            })?;
            if let Some(first) = plugin_names.insert(plugin.name.clone(), line) {
                anyhow::bail!(
                    "agsync.yaml linha {line}: plugin '{}' repetido (ja definido na linha {first})",
                    plugin.name
                );
            }
            let source = crate::plugin::repo_key(&plugin.marketplace.git);
            match marketplaces.get(&source) {
                Some((git_ref, first)) if git_ref != &plugin.marketplace.git_ref => anyhow::bail!(
                    "agsync.yaml linha {line}: marketplace '{}' ja usa a ref '{git_ref}' na linha {first}; \
                     use uma ref so por marketplace",
                    plugin.marketplace.git
                ),
                Some(_) => {}
                None => {
                    marketplaces.insert(source, (plugin.marketplace.git_ref.clone(), line));
                }
            }
        }
        self.validate_imports()
    }

    fn validate_imports(&self) -> anyhow::Result<()> {
        for (kind, entries) in self.imports.kinds() {
            let mut origins = BTreeMap::new();
            for entry in entries {
                let line = entry.defined.line();
                let import = &entry.value;
                self.validate_import(import).map_err(|e| {
                    anyhow::anyhow!("agsync.yaml linha {line}: imports.{kind}: {e}")
                })?;
                let origin = (crate::plugin::repo_key(&import.git), import.git_ref());
                if let Some(first) = origins.insert(origin, line) {
                    anyhow::bail!(
                        "agsync.yaml linha {line}: imports.{kind}: '{}' ref '{}' repetido (ja definido na linha {first})",
                        import.git,
                        import.git_ref()
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_import(&self, import: &Import) -> anyhow::Result<()> {
        check_git_url(&import.git, "imports.git")?;
        if import.git_ref.is_some() {
            check_ref(import.git_ref.as_deref(), "imports.git")?;
        }
        self.check_profiles(&import.profiles)?;
        for name in &import.excludes {
            check_name(name)?;
        }
        Ok(())
    }

    fn validate_plugin(&self, plugin: &Plugin) -> anyhow::Result<()> {
        check_name(&plugin.name)?;
        self.check_profiles(&plugin.profiles)?;
        check_agents(&plugin.agents)?;
        check_git_url(&plugin.marketplace.git, "marketplace.git")?;
        check_ref(Some(&plugin.marketplace.git_ref), "marketplace")
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
            check_git_url(&source.git, "source.git")?;
            check_ref(source.git_ref.as_deref(), "source.git")?;
        }
        Ok(())
    }

    fn validate_skill(&self, skill: &Skill) -> anyhow::Result<String> {
        self.check_profiles(&skill.profiles)?;
        check_agents(&skill.agents)?;
        if let Some(git) = &skill.git {
            check_git_url(git, "git")?;
            check_ref(skill.git_ref.as_deref(), "git")?;
        }
        if skill.path.is_none() {
            anyhow::bail!("defina path ou paths (pasta no repo de config ou no repo git)");
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

/// One skill per folder of each `paths` entry, sharing its `git`, `ref`, `profiles` and
/// `agents`. Each one carries the line of its own folder.
fn split_paths(skills: Vec<Spanned<Skill>>) -> anyhow::Result<Vec<Spanned<Skill>>> {
    let mut split = Vec::new();
    for mut entry in skills {
        let Some(paths) = entry.value.paths.take() else {
            split.push(entry);
            continue;
        };
        let line = entry.defined.line();
        check_paths_entry(&entry.value, &paths)
            .map_err(|e| anyhow::anyhow!("agsync.yaml linha {line}: skill: {e}"))?;
        for path in paths {
            let skill = Skill {
                path: Some(path.value),
                ..entry.value.clone()
            };
            split.push(Spanned::new(skill, path.referenced, path.defined));
        }
    }
    Ok(split)
}

fn check_paths_entry(skill: &Skill, paths: &[Spanned<String>]) -> anyhow::Result<()> {
    if skill.path.is_some() {
        anyhow::bail!("defina path ou paths, nao os dois");
    }
    if skill.git.is_none() {
        anyhow::bail!("paths so vale com git; no repo de config use uma entrada por path");
    }
    if paths.is_empty() {
        anyhow::bail!("paths vazio; liste ao menos uma pasta");
    }
    if skill.name.is_some() {
        anyhow::bail!("name nao vale com paths; use uma entrada com path para renomear");
    }
    Ok(())
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
    let Some(git_ref) = git_ref.filter(|r| !r.trim().is_empty()) else {
        anyhow::bail!("{field} exige ref");
    };
    if git_ref.trim_start().starts_with('-') {
        anyhow::bail!("ref '{git_ref}' nao pode comecar com -");
    }
    if crate::git::is_short_sha(git_ref) {
        anyhow::bail!("ref '{git_ref}' parece SHA abreviado; use o SHA completo");
    }
    Ok(())
}

/// Rejects a git URL carrying a password or token: git credentials belong to the
/// machine's credential helper, not to `agsync.yaml`. Also rejects one starting with
/// `-`, which `git` would read as an option.
fn check_git_url(git: &str, field: &str) -> anyhow::Result<()> {
    if git.trim().is_empty() {
        anyhow::bail!("{field} vazio");
    }
    if git.trim_start().starts_with('-') {
        anyhow::bail!("{field} '{git}' nao pode comecar com -");
    }
    if let Some((scheme, rest)) = git.split_once("://") {
        let authority = rest.split(['/', '\\']).next().unwrap_or("");
        if let Some((user, _)) = authority.split_once('@') {
            // `ssh://git@host` only names the SSH user. On http(s) any userinfo is a
            // token, and a colon on any scheme is a password.
            let http = scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https");
            if http || user.contains(':') {
                anyhow::bail!("{field} com credenciais embutidas; use o credential helper do git");
            }
        }
    } else if let Some((user, _)) = git.split_once('@')
        && user.contains(':')
    {
        anyhow::bail!("{field} com credenciais embutidas; use o credential helper do git");
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
plugins:
  - name: code-review
    marketplace:
      git: https://gitlab.example.com/group/market.git
      ref: v1.0.0
  - name: team-review
    profiles: [work]
    agents: [claude]
    marketplace:
      git: https://gitlab.example.com/group/market.git
      ref: v1.0.0
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
        let plugins: Vec<_> = m.plugins.iter().map(|p| p.value.name.as_str()).collect();
        assert_eq!(plugins, ["code-review", "team-review"]);
        assert_eq!(m.plugins[0].value.marketplace.git_ref, "v1.0.0");
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
    fn git_paths_become_one_skill_per_folder() {
        let text = r#"version: 1
profiles: [work]
skills:
  - git: https://x/skills.git
    ref: v1
    profiles: [work]
    agents: [claude]
    paths:
      - skills/engineering/pr
      - skills/productivity/writing-for-agents
  - path: local/hello
"#;
        let m = Manifest::parse(text).unwrap();
        let names: Vec<_> = m.skills.iter().map(|s| s.value.name()).collect();
        assert_eq!(names, ["pr", "writing-for-agents", "hello"]);
        let lines: Vec<_> = m.skills.iter().map(|s| s.defined.line()).collect();
        assert_eq!(lines, [9, 10, 11]);
        for skill in &m.skills[..2] {
            let skill = &skill.value;
            assert_eq!(skill.git.as_deref(), Some("https://x/skills.git"));
            assert_eq!(skill.git_ref.as_deref(), Some("v1"));
            assert_eq!(skill.profiles, Some(vec!["work".to_string()]));
            assert_eq!(skill.agents, Some(vec!["claude".to_string()]));
            assert!(skill.paths.is_none());
        }
        assert_eq!(
            m.skills[1].value.path.as_deref(),
            Some("skills/productivity/writing-for-agents")
        );
    }

    #[test]
    fn git_paths_rejects_ambiguous_entries() {
        let git = "version: 1
skills:
  - git: https://x/s.git
    ref: v1
";
        let both = format!(
            "{git}    path: a
    paths: [b]
"
        );
        assert!(err(&both).contains("nao os dois"), "{}", err(&both));
        assert!(err(&both).contains("linha 3"), "{}", err(&both));
        let empty = format!(
            "{git}    paths: []
"
        );
        assert!(err(&empty).contains("paths vazio"));
        let named = format!(
            "{git}    name: x
    paths: [a, b]
"
        );
        assert!(err(&named).contains("name nao vale"));
        let local = "version: 1
skills:
  - paths: [a, b]
";
        assert!(err(local).contains("so vale com git"));
        let no_ref = "version: 1
skills:
  - git: https://x/s.git
    paths: [a]
";
        assert!(err(no_ref).contains("exige ref"));
    }

    #[test]
    fn git_paths_checks_each_folder() {
        let git = "version: 1
skills:
  - git: https://x/s.git
    ref: v1
    paths:
";
        let escapes = format!(
            "{git}      - ok/a
      - ../fora
"
        );
        let msg = err(&escapes);
        assert!(msg.contains("linha 7") && msg.contains("relativo"), "{msg}");
        let dup = format!(
            "{git}      - a/x
      - b/x
"
        );
        let msg = err(&dup);
        assert!(msg.contains("linha 7") && msg.contains("repetida"), "{msg}");
    }

    #[test]
    fn plugin_needs_ref_name_and_git() {
        let no_ref = r#"version: 1
plugins:
  - name: p1
    marketplace:
      git: https://x/repo.git
"#;
        let msg = err(no_ref);
        assert!(msg.contains("linha"), "{msg}");
        assert!(msg.contains("ref"), "{msg}");

        let empty_ref = "version: 1\nplugins:\n  - name: p1\n    marketplace: {git: https://x/r.git, ref: ''}\n";
        assert!(err(empty_ref).contains("exige ref"));

        let no_name = "version: 1\nplugins:\n  - marketplace: {git: https://x/r.git, ref: v1}\n";
        assert!(err(no_name).contains("name"), "{no_name}");

        let no_git = "version: 1\nplugins:\n  - name: p1\n    marketplace: {ref: v1}\n";
        assert!(err(no_git).contains("git"), "{no_git}");
    }

    #[test]
    fn plugin_errors_carry_line_and_reason() {
        let dup = r#"version: 1
plugins:
  - name: p1
    marketplace: {git: https://x/r.git, ref: v1}
  - name: p1
    marketplace: {git: https://x/other.git, ref: v1}
"#;
        let msg = err(dup);
        assert!(msg.contains("linha 5"), "{msg}");
        assert!(msg.contains("repetido"), "{msg}");

        let profiles = r#"version: 1
profiles: [work]
plugins:
  - name: p1
    profiles: [home]
    marketplace: {git: https://x/r.git, ref: v1}
"#;
        let msg = err(profiles);
        assert!(msg.contains("linha 4") && msg.contains("home"), "{msg}");

        let agent = r#"version: 1
plugins:
  - name: p1
    agents: [vim]
    marketplace: {git: https://x/r.git, ref: v1}
"#;
        assert!(err(agent).contains("vim"));

        let unknown = r#"version: 1
plugins:
  - name: p1
    marketplace: {git: https://x/r.git, ref: v1, extra: 1}
"#;
        let msg = err(unknown);
        assert!(msg.contains("extra"), "{msg}");
        assert!(msg.contains("line 4"), "{msg}");
    }

    #[test]
    fn one_ref_per_marketplace() {
        let same = r#"version: 1
plugins:
  - name: p1
    marketplace: {git: https://x/r.git, ref: v1}
  - name: p2
    marketplace: {git: https://x/r, ref: v1}
"#;
        assert!(Manifest::parse(same).is_ok());

        let mixed = r#"version: 1
plugins:
  - name: p1
    marketplace: {git: https://x/r.git, ref: v1}
  - name: p2
    marketplace: {git: https://x/r.git, ref: v2}
"#;
        let msg = err(mixed);
        assert!(msg.contains("linha 5"), "{msg}");
        assert!(msg.contains("ref"), "{msg}");
    }

    #[test]
    fn plugin_rejects_credentials_and_short_sha() {
        let creds = r#"version: 1
plugins:
  - name: p1
    marketplace: {git: "https://user:token@gitlab.example.com/g/r.git", ref: v1}
"#;
        assert!(err(creds).contains("credenciais"));

        let token = "version: 1\nplugins:\n  - name: p1\n    marketplace: {git: 'https://glpat-abc@gitlab.example.com/g/r.git', ref: v1}\n";
        assert!(err(token).contains("credenciais"));

        let ssh = "version: 1\nplugins:\n  - name: p1\n    marketplace: {git: 'ssh://git@gitlab.example.com/g/r.git', ref: v1}\n";
        assert!(Manifest::parse(ssh).is_ok(), "{}", err(ssh));

        let scp_token = "version: 1\nplugins:\n  - name: p1\n    marketplace: {git: 'user:token@gitlab.example.com:g/r.git', ref: v1}\n";
        assert!(err(scp_token).contains("credenciais"));

        let scp = "version: 1\nplugins:\n  - name: p1\n    marketplace: {git: 'git@gitlab.example.com:g/r.git', ref: v1}\n";
        assert!(Manifest::parse(scp).is_ok(), "{}", err(scp));

        let short = "version: 1\nplugins:\n  - name: p1\n    marketplace: {git: https://x/r.git, ref: abc1234}\n";
        assert!(err(short).contains("SHA abreviado"));
    }

    #[test]
    fn plugin_accepts_local_marketplace() {
        let local =
            "version: 1\nplugins:\n  - name: p1\n    marketplace: {git: ./market, ref: v1}\n";
        assert!(Manifest::parse(local).is_ok());
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
    fn parses_imports_per_kind() {
        let text = r#"version: 1
profiles: [work]
imports:
  mcps:
    - git: https://x/mcps.git
      ref: v1
      profiles: [work]
      excludes: [mcp-test]
  skills:
    - git: https://x/mcps.git
  plugins:
    - git: https://x/plugins.git
"#;
        let m = Manifest::parse(text).unwrap();
        let mcps = &m.imports.mcps[0].value;
        assert_eq!(mcps.git_ref(), "v1");
        assert_eq!(mcps.profiles, Some(vec!["work".to_string()]));
        assert_eq!(mcps.excludes, ["mcp-test"]);
        assert_eq!(m.imports.skills[0].value.git_ref(), "main");
        assert_eq!(m.imports.plugins.len(), 1);
        assert!(
            Manifest::parse(
                "version: 1
"
            )
            .unwrap()
            .imports
            .mcps
            .is_empty()
        );
    }

    #[test]
    fn imports_errors_carry_line_and_reason() {
        let base = "version: 1
profiles: [work]
imports:
  skills:
";
        let cases = [
            (
                "    - git: ''
",
                "imports.git vazio",
            ),
            (
                "    - git: https://u:t@x/r.git
",
                "imports.git com credenciais",
            ),
            (
                "    - git: https://x/r.git
      ref: ''
",
                "exige ref",
            ),
            (
                "    - git: https://x/r.git
      ref: abc1234
",
                "SHA abreviado",
            ),
            (
                "    - git: https://x/r.git
      profiles: [home]
",
                "perfil 'home'",
            ),
            (
                "    - git: https://x/r.git
      excludes: ['a b']
",
                "invalido",
            ),
        ];
        for (entry, reason) in cases {
            let msg = err(&format!("{base}{entry}"));
            assert!(msg.contains("linha 5"), "{msg}");
            assert!(msg.contains("imports.skills"), "{msg}");
            assert!(msg.contains(reason), "{msg}");
        }
        let unknown = format!(
            "{base}    - git: https://x/r.git
      bogus: 1
"
        );
        assert!(err(&unknown).contains("bogus"));
        let kind = "version: 1
imports:
  hooks: []
";
        assert!(err(kind).contains("hooks"));
    }

    #[test]
    fn same_import_origin_twice_in_a_kind_is_error() {
        let dup = r#"version: 1
imports:
  mcps:
    - git: https://x/r.git
    - git: https://x/r
      ref: main
"#;
        let msg = err(dup);
        assert!(msg.contains("linha 5") && msg.contains("repetido"), "{msg}");

        let ok = r#"version: 1
imports:
  mcps:
    - git: https://x/r.git
    - git: https://x/r.git
      ref: v2
  skills:
    - git: https://x/r.git
"#;
        assert!(Manifest::parse(ok).is_ok(), "{}", err(ok));
    }

    #[test]
    fn every_git_field_rejects_options_and_credentials() {
        let cases = [
            "mcps:
  a:
    command: x
    source: {git: '--upload-pack=x', ref: v1}
",
            "mcps:
  a:
    command: x
    source: {git: https://x/r.git, ref: '-x'}
",
            "mcps:
  a:
    command: x
    source: {git: 'https://u:t@x/r.git', ref: v1}
",
            "skills:
  - git: '--upload-pack=x'
    ref: v1
    path: s
",
            "skills:
  - git: https://x/r.git
    ref: '--upload-pack=x'
    path: s
",
            "skills:
  - git: 'https://tok@x/r.git'
    ref: v1
    path: s
",
            "plugins:
  - name: p
    marketplace: {git: '-x', ref: v1}
",
            "plugins:
  - name: p
    marketplace: {git: https://x/r.git, ref: '-x'}
",
            "imports:
  mcps:
    - git: '--upload-pack=x'
",
            "imports:
  mcps:
    - git: https://x/r.git
      ref: '-x'
",
        ];
        for case in cases {
            let msg = err(&format!(
                "version: 1
{case}"
            ));
            let reason = msg.contains("nao pode comecar com -") || msg.contains("credenciais");
            assert!(reason && msg.contains("linha"), "{case}: {msg}");
        }
        let source_creds = err(&format!(
            "version: 1
{}",
            cases[2]
        ));
        assert!(
            source_creds.contains("source.git com credenciais"),
            "{source_creds}"
        );
        let skill_creds = err(&format!(
            "version: 1
{}",
            cases[5]
        ));
        assert!(
            skill_creds.contains("skill: git com credenciais"),
            "{skill_creds}"
        );
    }

    #[test]
    fn agent_targeting() {
        assert!(targets(&None, "codex"));
        assert!(targets(&Some(vec!["claude".into()]), "claude"));
        assert!(!targets(&Some(vec!["claude".into()]), "codex"));
    }
}
