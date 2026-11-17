//! `imports`: items of one kind taken from other config repos, for the active profiles.

use std::collections::{BTreeMap, BTreeSet};

use crate::git::{self, Snapshot};
use crate::manifest::{Import, Manifest, selected};
use crate::resolved::Watched;

/// The manifest with its imports merged in, and what the merge has to say.
pub struct Composed {
    pub manifest: Manifest,
    pub warnings: Vec<String>,
    pub skipped: Vec<String>,
    /// Imports followed by branch or tag, with the commit they resolved to.
    pub watched: Vec<Watched>,
}

/// Merges every import selected for `active` into `manifest`. Imports of inactive
/// profiles are not fetched. Any import that cannot be read fails the whole plan:
/// skipping it would remove its items from the agents.
pub fn compose(mut manifest: Manifest, active: &[String]) -> anyhow::Result<Composed> {
    let imports = std::mem::take(&mut manifest.imports);
    let mut merge = Merge {
        owners: Owners::of(&manifest, active),
        manifest,
        imported: BTreeSet::new(),
        warnings: Vec::new(),
        skipped: Vec::new(),
    };
    let mut remotes = Remotes::default();
    let mut watched = Vec::new();
    for (kind, entries) in imports.kinds() {
        for entry in entries {
            let import = &entry.value;
            if !selected(&import.profiles, active) {
                continue;
            }
            let origin = format!("imports.{kind} {}@{}", import.git, import.git_ref());
            let (sha, remote) = remotes
                .get(import)
                .map_err(|e| anyhow::anyhow!("{origin}: {e:#}"))?;
            watched.extend(Watched::new(&import.git, import.git_ref(), &sha));
            merge.add(kind, import, &sha, remote, &origin);
        }
    }
    merge.drop_checkout_clashes(active);
    watched.sort();
    watched.dedup();
    Ok(Composed {
        manifest: merge.manifest,
        warnings: merge.warnings,
        skipped: merge.skipped,
        watched,
    })
}

/// Manifests of imported repos read in this run, by git url and commit.
#[derive(Default)]
struct Remotes {
    read: BTreeMap<(String, String), Manifest>,
}

impl Remotes {
    fn get(&mut self, import: &Import) -> anyhow::Result<(String, &Manifest)> {
        let sha = git::resolve(&import.git, import.git_ref())?;
        let key = (import.git.clone(), sha.clone());
        if !self.read.contains_key(&key) {
            let manifest = read_remote(import, &sha)?;
            self.read.insert(key.clone(), manifest);
        }
        Ok((sha, &self.read[&key]))
    }
}

fn read_remote(import: &Import, sha: &str) -> anyhow::Result<Manifest> {
    let snapshot = Snapshot::fetch(&import.git, import.git_ref(), sha)?;
    let text = std::fs::read_to_string(snapshot.path().join("agsync.yaml"))
        .map_err(|_| anyhow::anyhow!("agsync.yaml nao encontrado"))?;
    let manifest = Manifest::parse(&text)?;
    manifest.check_skill_dirs(snapshot.path())?;
    snapshot.close()?;
    Ok(manifest)
}

/// Where each active item name of each kind comes from.
struct Owners(BTreeMap<(&'static str, String), String>);

impl Owners {
    fn of(manifest: &Manifest, active: &[String]) -> Owners {
        let main = "agsync.yaml".to_string();
        let mut owners = BTreeMap::new();
        for (name, mcp) in &manifest.mcps {
            if selected(&mcp.value.profiles, active) {
                owners.insert(("mcps", name.clone()), main.clone());
            }
        }
        for skill in &manifest.skills {
            if selected(&skill.value.profiles, active) {
                owners.insert(("skills", skill.value.name()), main.clone());
            }
        }
        for plugin in &manifest.plugins {
            if selected(&plugin.value.profiles, active) {
                owners.insert(("plugins", plugin.value.name.clone()), main.clone());
            }
        }
        Owners(owners)
    }
}

struct Merge {
    manifest: Manifest,
    owners: Owners,
    /// Imported skill and MCP names, by kind.
    imported: BTreeSet<(&'static str, String)>,
    warnings: Vec<String>,
    skipped: Vec<String>,
}

impl Merge {
    fn add(
        &mut self,
        kind: &'static str,
        import: &Import,
        sha: &str,
        remote: &Manifest,
        origin: &str,
    ) {
        let names = match kind {
            "mcps" => self.add_mcps(import, remote, origin),
            "skills" => self.add_skills(import, sha, remote, origin),
            _ => self.add_plugins(import, remote, origin),
        };
        for name in &import.excludes {
            if !names.contains(name) {
                self.warnings
                    .push(format!("{origin}: excludes '{name}' nao existe na origem"));
            }
        }
        let nested = remote.imports.kinds().iter().any(|(_, e)| !e.is_empty());
        if nested {
            self.warnings.push(format!(
                "{origin}: imports do repo importado ignorados (so um nivel)"
            ));
        }
    }

    /// True when `name` may join from `origin`; warns when an earlier owner keeps it.
    fn claim(&mut self, kind: &'static str, name: &str, origin: &str) -> bool {
        let key = (kind, name.to_string());
        if let Some(owner) = self.owners.0.get(&key) {
            self.warnings.push(format!(
                "{origin}: {kind} '{name}' ignorado; ja vem de {owner}"
            ));
            return false;
        }
        self.owners.0.insert(key.clone(), origin.to_string());
        self.imported.insert(key);
        true
    }

    fn add_mcps(&mut self, import: &Import, remote: &Manifest, origin: &str) -> Vec<String> {
        for (name, mcp) in &remote.mcps {
            if import.excludes.contains(name) || !self.claim("mcps", name, origin) {
                continue;
            }
            let mut mcp = mcp.clone();
            mcp.value.profiles = import.profiles.clone();
            self.manifest.mcps.insert(name.clone(), mcp);
        }
        remote.mcps.keys().cloned().collect()
    }

    fn add_skills(
        &mut self,
        import: &Import,
        sha: &str,
        remote: &Manifest,
        origin: &str,
    ) -> Vec<String> {
        for skill in &remote.skills {
            let name = skill.value.name();
            if import.excludes.contains(&name) || !self.claim("skills", &name, origin) {
                continue;
            }
            let mut skill = skill.clone();
            skill.value.profiles = import.profiles.clone();
            if skill.value.git.is_none() {
                skill.value.git = Some(import.git.clone());
                skill.value.git_ref = Some(sha.to_string());
                skill.value.name = Some(name.clone());
            }
            self.manifest.skills.retain(|s| s.value.name() != name);
            self.manifest.skills.push(skill);
        }
        remote.skills.iter().map(|s| s.value.name()).collect()
    }

    fn add_plugins(&mut self, import: &Import, remote: &Manifest, origin: &str) -> Vec<String> {
        for plugin in &remote.plugins {
            let name = plugin.value.name.clone();
            if import.excludes.contains(&name) {
                continue;
            }
            if crate::plugin::is_local_marketplace(&plugin.value.marketplace.git) {
                self.skipped.push(format!(
                    "plugin {name}: marketplace local em {origin} nao vale fora do repo de origem"
                ));
                continue;
            }
            if !self.claim("plugins", &name, origin) {
                continue;
            }
            let mut plugin = plugin.clone();
            plugin.value.profiles = import.profiles.clone();
            self.manifest.plugins.retain(|p| p.value.name != name);
            self.manifest.plugins.push(plugin);
        }
        remote
            .plugins
            .iter()
            .map(|p| p.value.name.clone())
            .collect()
    }

    /// A git skill and an MCP with source of the same name would share one checkout
    /// folder: the imported one of the pair is dropped.
    fn drop_checkout_clashes(&mut self, active: &[String]) {
        let mut drop_skills = Vec::new();
        let mut drop_mcps = Vec::new();
        for skill in &self.manifest.skills {
            let skill = &skill.value;
            let name = skill.name();
            let Some(mcp) = self.manifest.mcps.get(&name) else {
                continue;
            };
            let both_active =
                selected(&skill.profiles, active) && selected(&mcp.value.profiles, active);
            if skill.git.is_none() || !mcp.value.has_git_source() || !both_active {
                continue;
            }
            if self.imported.contains(&("skills", name.clone())) {
                drop_skills.push(name);
            } else if self.imported.contains(&("mcps", name.clone())) {
                drop_mcps.push(name);
            }
        }
        for name in drop_skills {
            self.manifest.skills.retain(|s| s.value.name() != name);
            self.warnings.push(format!(
                "skill '{name}' importada ignorada: tem o mesmo nome de um mcp com source"
            ));
        }
        for name in drop_mcps {
            self.manifest.mcps.remove(&name);
            self.warnings.push(format!(
                "mcp '{name}' importado ignorado: tem o mesmo nome de uma skill git"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    struct Repo {
        _dir: TempDir,
        path: std::path::PathBuf,
        url: String,
    }

    impl Repo {
        fn new(yaml: &str, skills: &[&str]) -> Repo {
            let dir = TempDir::new("agsync-test").unwrap();
            let path = dir.path().to_path_buf();
            let url = path.to_string_lossy().replace('\\', "/");
            let repo = Repo {
                _dir: dir,
                path,
                url,
            };
            repo.run(&["init", "-q", "-b", "main"]);
            for skill in skills {
                let folder = repo.path.join(skill);
                std::fs::create_dir_all(&folder).unwrap();
                std::fs::write(folder.join("SKILL.md"), skill).unwrap();
            }
            repo.commit(yaml);
            repo
        }

        fn run(&self, args: &[&str]) -> String {
            let mut all = vec![
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "tag.gpgsign=false",
            ];
            all.extend_from_slice(args);
            git::git(&all, Some(&self.path)).unwrap().trim().to_string()
        }

        /// Commits `yaml` as `agsync.yaml`, or a repo without one when empty.
        fn commit(&self, yaml: &str) -> String {
            if yaml.is_empty() {
                std::fs::write(self.path.join("README"), "x").unwrap();
            } else {
                std::fs::write(self.path.join("agsync.yaml"), yaml).unwrap();
            }
            self.run(&["add", "-A"]);
            self.run(&["commit", "-q", "-m", "c"]);
            self.run(&["rev-parse", "HEAD"])
        }
    }

    fn mcps(names: &[(&str, &str)]) -> String {
        let mut yaml = "version: 1\nmcps:\n".to_string();
        for (name, command) in names {
            yaml.push_str(&format!("  {name}:\n    command: {command}\n"));
        }
        yaml
    }

    fn compose_yaml(yaml: &str, active: &[&str]) -> anyhow::Result<Composed> {
        let active: Vec<String> = active.iter().map(|s| s.to_string()).collect();
        compose(Manifest::parse(yaml).unwrap(), &active)
    }

    fn command(c: &Composed, name: &str) -> String {
        c.manifest.mcps[name].value.command.clone().unwrap()
    }

    fn missing_url(dir: &TempDir) -> String {
        dir.path().join("nope").to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn imports_each_kind_from_several_repos() {
        let a = Repo::new(
            r#"version: 1
profiles: [home]
imports:
  mcps:
    - git: https://x/nested.git
mcps:
  a1:
    profiles: [home]
    agents: [claude]
    command: a1
  a2:
    command: a2
skills:
  - path: skills/s1
    profiles: [home]
  - git: https://x/other.git
    ref: v3
    path: sub/s2
plugins:
  - name: p1
    marketplace: {git: https://x/market.git, ref: v1}
  - name: plocal
    marketplace: {git: ./market, ref: v1}
"#,
            &["skills/s1"],
        );
        let b = Repo::new(&mcps(&[("b1", "b1")]), &[]);
        let main = format!(
            r#"version: 1
profiles: [work]
imports:
  mcps:
    - git: "{a}"
      profiles: [work]
    - git: "{b}"
  skills:
    - git: "{a}"
  plugins:
    - git: "{a}"
"#,
            a = a.url,
            b = b.url
        );
        let c = compose_yaml(&main, &["work"]).unwrap();
        let names: Vec<_> = c.manifest.mcps.keys().map(String::as_str).collect();
        assert_eq!(names, ["a1", "a2", "b1"]);
        let a1 = &c.manifest.mcps["a1"].value;
        assert_eq!(a1.profiles, Some(vec!["work".to_string()]));
        assert_eq!(a1.agents, Some(vec!["claude".to_string()]));
        assert_eq!(c.manifest.mcps["b1"].value.profiles, None);

        let head = a.run(&["rev-parse", "HEAD"]);
        let s1 = &c.manifest.skills[0].value;
        assert_eq!(s1.git.as_deref(), Some(a.url.as_str()));
        assert_eq!(s1.git_ref.as_deref(), Some(head.as_str()));
        assert_eq!(s1.path.as_deref(), Some("skills/s1"));
        assert_eq!(s1.name(), "s1");
        assert_eq!(s1.profiles, None);
        let s2 = &c.manifest.skills[1].value;
        assert_eq!(s2.git.as_deref(), Some("https://x/other.git"));
        assert_eq!(s2.git_ref.as_deref(), Some("v3"));

        let plugins: Vec<_> = c.manifest.plugins.iter().map(|p| &p.value.name).collect();
        assert_eq!(plugins, ["p1"]);
        assert!(c.skipped[0].contains("plugin plocal"), "{:?}", c.skipped);
        assert!(c.warnings.iter().any(|w| w.contains("so um nivel")));
        assert_eq!(c.watched.len(), 2, "{:?}", c.watched);
        assert!(c.watched.iter().all(|w| w.git_ref == "main"));
        assert!(c.manifest.imports.mcps.is_empty());
    }

    #[test]
    fn inactive_import_is_not_fetched() {
        let dir = TempDir::new("agsync-test").unwrap();
        let missing = missing_url(&dir);
        let main = format!(
            "version: 1\nprofiles: [work, personal]\nimports:\n  mcps:\n    - git: \"{missing}\"\n      profiles: [personal]\n"
        );
        let c = compose_yaml(&main, &["work"]).unwrap();
        assert!(c.manifest.mcps.is_empty());
        let msg = compose_yaml(&main, &["personal"])
            .err()
            .unwrap()
            .to_string();
        assert!(
            msg.starts_with(&format!("imports.mcps {missing}@main: ")),
            "{msg}"
        );
    }

    #[test]
    fn content_matches_the_ref_not_head() {
        let repo = Repo::new(&mcps(&[("x", "one")]), &[]);
        let first = repo.run(&["rev-parse", "HEAD"]);
        repo.run(&["tag", "-a", "v1", "-m", "v1"]);
        repo.run(&["checkout", "-q", "-b", "dev"]);
        repo.commit(&mcps(&[("x", "three")]));
        repo.run(&["checkout", "-q", "main"]);
        repo.commit(&mcps(&[("x", "two")]));
        let at = |git_ref: &str| {
            let main = format!(
                "version: 1\nimports:\n  mcps:\n    - git: \"{}\"\n      ref: {git_ref}\n",
                repo.url
            );
            compose_yaml(&main, &[]).unwrap()
        };
        let tag = at("v1");
        assert_eq!(command(&tag, "x"), "one");
        assert_eq!(tag.watched[0].sha, first);
        let pinned = at(&first);
        assert_eq!(command(&pinned, "x"), "one");
        assert!(pinned.watched.is_empty());
        assert_eq!(command(&at("dev"), "x"), "three");
        assert_eq!(command(&at("main"), "x"), "two");
    }

    #[test]
    fn main_wins_then_first_import() {
        let a = Repo::new(&mcps(&[("m", "a"), ("n", "a"), ("k", "a")]), &[]);
        let b = Repo::new(&mcps(&[("k", "b"), ("j", "b")]), &[]);
        let main = format!(
            r#"version: 1
profiles: [work, personal]
mcps:
  m:
    command: main
  n:
    profiles: [personal]
    command: main
imports:
  mcps:
    - git: "{a}"
    - git: "{b}"
    - git: "{b}"
      ref: other
      profiles: [personal]
"#,
            a = a.url,
            b = b.url
        );
        let c = compose_yaml(&main, &["work"]).unwrap();
        assert_eq!(command(&c, "m"), "main");
        assert_eq!(command(&c, "n"), "a");
        assert_eq!(c.manifest.mcps["n"].value.profiles, None);
        assert_eq!(command(&c, "k"), "a");
        assert_eq!(command(&c, "j"), "b");
        assert_eq!(c.warnings.len(), 2, "{:?}", c.warnings);
        assert!(c.warnings[0].contains("'m'") && c.warnings[0].contains("agsync.yaml"));
        let first = format!("imports.mcps {}@main", a.url);
        assert!(c.warnings[1].contains("'k'") && c.warnings[1].contains(&first));
        assert!(c.warnings[1].starts_with(&format!("imports.mcps {}", b.url)));
    }

    #[test]
    fn skills_and_plugins_keep_the_main_item() {
        let a = Repo::new(
            "version: 1\nskills:\n  - path: s/dup\n  - path: s/new\nplugins:\n  - name: p\n    marketplace: {git: https://x/a.git, ref: v1}\n",
            &["s/dup", "s/new"],
        );
        let main = format!(
            r#"version: 1
skills:
  - path: local/dup
plugins:
  - name: p
    marketplace: {{git: https://x/main.git, ref: v1}}
imports:
  skills:
    - git: "{a}"
  plugins:
    - git: "{a}"
"#,
            a = a.url
        );
        let c = compose_yaml(&main, &[]).unwrap();
        let skills: Vec<_> = c.manifest.skills.iter().map(|s| s.value.name()).collect();
        assert_eq!(skills, ["dup", "new"]);
        assert!(c.manifest.skills[0].value.git.is_none());
        assert_eq!(c.manifest.plugins.len(), 1);
        assert_eq!(
            c.manifest.plugins[0].value.marketplace.git,
            "https://x/main.git"
        );
        assert_eq!(c.warnings.len(), 2, "{:?}", c.warnings);
    }

    #[test]
    fn excludes_only_that_origin() {
        let a = Repo::new(&mcps(&[("x", "a"), ("y", "a")]), &[]);
        let b = Repo::new(&mcps(&[("x", "b")]), &[]);
        let main = format!(
            "version: 1\nimports:\n  mcps:\n    - git: \"{}\"\n      excludes: [x, ghost]\n    - git: \"{}\"\n",
            a.url, b.url
        );
        let c = compose_yaml(&main, &[]).unwrap();
        assert_eq!(command(&c, "x"), "b");
        assert_eq!(command(&c, "y"), "a");
        assert_eq!(c.warnings.len(), 1, "{:?}", c.warnings);
        assert!(c.warnings[0].contains("excludes 'ghost'"));
    }

    #[test]
    fn imported_git_skill_clashing_with_mcp_source_is_dropped() {
        let a = Repo::new("version: 1\nskills:\n  - path: s/tool\n", &["s/tool"]);
        let main = format!(
            "version: 1\nmcps:\n  tool:\n    command: x\n    source: {{git: https://x/t.git, ref: v1}}\nimports:\n  skills:\n    - git: \"{}\"\n",
            a.url
        );
        let c = compose_yaml(&main, &[]).unwrap();
        assert!(c.manifest.skills.is_empty());
        assert!(c.warnings[0].contains("skill 'tool'"), "{:?}", c.warnings);
    }

    #[test]
    fn unreadable_imports_fail_with_origin() {
        let empty = Repo::new("", &[]);
        let invalid = Repo::new("version: 2\n", &[]);
        let no_skill_md = Repo::new("version: 1\nskills:\n  - path: s/x\n", &[]);
        let dir = TempDir::new("agsync-test").unwrap();
        let missing = missing_url(&dir);
        let fail = |url: &str, git_ref: &str| {
            let main = format!(
                "version: 1\nimports:\n  skills:\n    - git: \"{url}\"\n      ref: {git_ref}\n"
            );
            let msg = format!("{:#}", compose_yaml(&main, &[]).err().unwrap());
            let origin = format!("imports.skills {url}@{git_ref}: ");
            assert!(msg.starts_with(&origin), "{msg}");
            msg
        };
        assert!(fail(&missing, "main").contains("git"));
        assert!(fail(&empty.url, "main").contains("agsync.yaml nao encontrado"));
        assert!(fail(&invalid.url, "main").contains("version 2"));
        assert!(fail(&no_skill_md.url, "main").contains("SKILL.md"));
        assert!(fail(&empty.url, "nope").contains("nao encontrada"));
    }
}
