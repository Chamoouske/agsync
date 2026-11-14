use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::*;
use crate::agent::Adapter;
use crate::deliver::McpWant;
use crate::desired::{Desired, SkillSrc};
use crate::hash::hash_dir;
use crate::mcp::McpSpec;
use crate::plugin::PluginSpec;
use crate::state::LinkMode;
use crate::tmp::TempDir;

/// Agent config held in memory; items named in `broken` fail to be written.
#[derive(Default)]
struct Agent {
    mcps: RefCell<BTreeMap<String, McpSpec>>,
    plugins: RefCell<BTreeSet<String>>,
    broken: BTreeSet<String>,
    skills_dir: PathBuf,
}

impl Agent {
    fn fail_if_broken(&self, name: &str) -> anyhow::Result<()> {
        if self.broken.contains(name) {
            anyhow::bail!("falha simulada em {name}");
        }
        Ok(())
    }
}

struct Fake(Rc<Agent>);

impl Adapter for Fake {
    fn mcps(&self) -> anyhow::Result<BTreeMap<String, String>> {
        let mcps = self.0.mcps.borrow();
        Ok(mcps.iter().map(|(n, s)| (n.clone(), s.hash())).collect())
    }

    fn mcp_spec(&self, name: &str) -> anyhow::Result<Option<McpSpec>> {
        Ok(self.0.mcps.borrow().get(name).cloned())
    }

    fn skills_dir(&self) -> PathBuf {
        self.0.skills_dir.clone()
    }

    fn backup(&self, _dir: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn write_mcp(&self, name: &str, spec: &McpSpec) -> anyhow::Result<()> {
        self.0.fail_if_broken(name)?;
        self.0.mcps.borrow_mut().insert(name.into(), spec.clone());
        Ok(())
    }

    fn remove_mcp(&self, name: &str) -> anyhow::Result<()> {
        self.0.mcps.borrow_mut().remove(name);
        Ok(())
    }

    fn write_skill(&self, name: &str, cached: &Path, backups: &Path) -> anyhow::Result<LinkMode> {
        self.0.fail_if_broken(name)?;
        link::install(cached, &self.skills_dir().join(name), backups)
    }

    fn plugin_support(&self) -> Result<(), String> {
        Ok(())
    }

    fn plugin_installed(&self, spec: &PluginSpec) -> anyhow::Result<Option<String>> {
        let id = repo_id(&spec.name);
        Ok(self.0.plugins.borrow().contains(&id).then_some(id))
    }

    fn plugin_named(&self, name: &str) -> anyhow::Result<Option<String>> {
        let prefix = format!("{name}@");
        let plugins = self.0.plugins.borrow();
        let ids: Vec<&String> = plugins
            .iter()
            .filter(|id| id.starts_with(&prefix))
            .collect();
        Ok(match ids.as_slice() {
            [id] => Some(id.to_string()),
            _ => None,
        })
    }

    fn plugin_present(&self, id: &str) -> anyhow::Result<bool> {
        Ok(self.0.plugins.borrow().contains(id))
    }

    fn install_plugin(
        &self,
        spec: &PluginSpec,
        _managed: &BTreeSet<String>,
    ) -> anyhow::Result<String> {
        self.0.fail_if_broken(&spec.name)?;
        let id = repo_id(&spec.name);
        self.0.plugins.borrow_mut().insert(id.clone());
        Ok(id)
    }

    fn uninstall_plugin(&self, id: &str) -> anyhow::Result<()> {
        self.0.plugins.borrow_mut().remove(id);
        Ok(())
    }
}

fn repo_id(name: &str) -> String {
    format!("{name}@repo-market")
}

fn stdio(arg: &str) -> McpSpec {
    McpSpec::Stdio {
        command: "node".into(),
        args: vec![arg.into()],
        env: BTreeMap::new(),
    }
}

struct World {
    tmp: TempDir,
    dirs: Dirs,
    state: State,
}

impl World {
    fn new() -> World {
        let tmp = TempDir::new("agsync-test").unwrap();
        let dirs = Dirs {
            config: tmp.path().join("config"),
            data: tmp.path().join("data"),
        };
        World {
            tmp,
            dirs,
            state: State::default(),
        }
    }

    fn agent(&self, name: &str, broken: &[&str]) -> Rc<Agent> {
        Rc::new(Agent {
            skills_dir: self.tmp.path().join(name).join("skills"),
            broken: broken.iter().map(|s| s.to_string()).collect(),
            ..Agent::default()
        })
    }

    /// Folder with one `SKILL.md`, under `parent`.
    fn skill_dir(&self, parent: &Path, name: &str, body: &str) -> PathBuf {
        let dir = parent.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), body).unwrap();
        dir
    }

    /// What the config repo declares: mcp, skill and plugin, each under its own name.
    fn desired(&self, mcp: &str, skill: &str, plugin: &str) -> Desired {
        let mut desired = Desired::default();
        let spec = stdio("repo.js");
        let want = McpWant {
            shown: spec.clone(),
            write: spec.clone(),
            hash: spec.hash(),
            sha: None,
        };
        desired.mcps.insert(mcp.into(), want);
        let repo = self.tmp.path().join("repo");
        let dir = self.skill_dir(&repo, skill, "repo skill");
        let hash = hash_dir(&dir).unwrap();
        let src = SkillSrc {
            dir,
            hash,
            sha: None,
        };
        desired.skills.insert(skill.into(), src);
        let spec = PluginSpec::new(plugin, "https://x/repo-market.git", "v1");
        desired.plugins.insert(plugin.into(), spec);
        desired
    }

    fn plan(
        &self,
        name: &'static str,
        agent: &Rc<Agent>,
        desired: Desired,
        adopt: bool,
    ) -> AgentPlan {
        let tracked = self.state.agent(name);
        let fake = Box::new(Fake(agent.clone()));
        AgentPlan::new(name, fake, desired, &tracked, adopt, &mut Vec::new()).unwrap()
    }

    fn apply(&mut self, plan: &AgentPlan) -> usize {
        let outcome = apply_agent(plan, &self.dirs, &mut self.state, &BTreeSet::new());
        outcome.failures
    }

    fn saved(&self) -> State {
        State::load(&self.dirs.state_file()).unwrap()
    }
}

fn actions(plan: &AgentPlan) -> Vec<(&'static str, String, Action)> {
    let mcps = plan.mcps.iter().map(|c| ("mcp", c));
    let skills = plan.skills.iter().map(|c| ("skill", c));
    let plugins = plan.plugins.iter().map(|c| ("plugin", c));
    mcps.chain(skills)
        .chain(plugins)
        .map(|(kind, c)| (kind, c.name.clone(), c.action))
        .collect()
}

/// Manual mcp `x`, skill `s` and plugin `p` (from another marketplace) on `agent`.
fn install_by_hand(world: &World, agent: &Agent) {
    agent
        .mcps
        .borrow_mut()
        .insert("x".into(), stdio("manual.js"));
    world.skill_dir(&agent.skills_dir, "s", "manual skill");
    agent.plugins.borrow_mut().insert("p@other-market".into());
}

fn skill_body(agent: &Agent, name: &str) -> String {
    std::fs::read_to_string(agent.skills_dir.join(name).join("SKILL.md")).unwrap()
}

#[test]
fn exact_match_is_a_conflict_left_alone_without_adopt() {
    let mut world = World::new();
    let claude = world.agent("claude", &[]);
    install_by_hand(&world, &claude);

    let plan = world.plan("claude", &claude, world.desired("x", "s", "p"), false);
    let expected = vec![
        ("mcp", "x".to_string(), Action::Conflict),
        ("skill", "s".to_string(), Action::Conflict),
        ("plugin", "p".to_string(), Action::Conflict),
    ];
    assert_eq!(actions(&plan), expected);
    assert_eq!(world.apply(&plan), 0);

    assert_eq!(claude.mcps.borrow()["x"], stdio("manual.js"));
    assert_eq!(skill_body(&claude, "s"), "manual skill");
    assert!(claude.plugins.borrow().contains("p@other-market"));
    assert_eq!(world.state.agent("claude"), Default::default());
}

#[test]
fn exact_match_is_replaced_and_tracked_with_adopt() {
    let mut world = World::new();
    let claude = world.agent("claude", &[]);
    install_by_hand(&world, &claude);

    let plan = world.plan("claude", &claude, world.desired("x", "s", "p"), true);
    let adopted = actions(&plan).iter().all(|(_, _, a)| *a == Action::Adopt);
    assert!(adopted, "{:?}", actions(&plan));
    assert_eq!(world.apply(&plan), 0);

    assert_eq!(claude.mcps.borrow()["x"], stdio("repo.js"));
    assert_eq!(skill_body(&claude, "s"), "repo skill");
    let plugins: Vec<String> = claude.plugins.borrow().iter().cloned().collect();
    assert_eq!(plugins, [repo_id("p")]);

    let saved = world.saved().agent("claude");
    assert!(saved.mcps.contains_key("x"));
    assert!(saved.skills.contains_key("s"));
    assert_eq!(saved.plugins["p"].plugin, Some(repo_id("p")));

    let again = world.plan("claude", &claude, world.desired("x", "s", "p"), false);
    let kept = actions(&again).iter().all(|(_, _, a)| *a == Action::Keep);
    assert!(kept, "{:?}", actions(&again));
}

#[test]
fn manual_items_without_a_match_stay_untouched() {
    let mut world = World::new();
    let claude = world.agent("claude", &[]);
    install_by_hand(&world, &claude);

    let plan = world.plan("claude", &claude, world.desired("y", "t", "q"), true);
    let added = actions(&plan).iter().all(|(_, _, a)| *a == Action::Add);
    assert!(added, "{:?}", actions(&plan));
    assert_eq!(world.apply(&plan), 0);

    assert_eq!(claude.mcps.borrow()["x"], stdio("manual.js"));
    assert_eq!(skill_body(&claude, "s"), "manual skill");
    assert!(claude.plugins.borrow().contains("p@other-market"));
    let saved = world.saved().agent("claude");
    assert!(!saved.mcps.contains_key("x"));
    assert!(!saved.skills.contains_key("s"));
    assert!(!saved.plugins.contains_key("p"));
}

#[test]
fn homonyms_of_another_kind_or_agent_are_not_a_match() {
    let mut world = World::new();
    let claude = world.agent("claude", &[]);
    world.skill_dir(&claude.skills_dir, "x", "manual skill named like the mcp");
    claude
        .mcps
        .borrow_mut()
        .insert("s".into(), stdio("manual mcp named like the skill"));
    let codex = world.agent("codex", &[]);
    install_by_hand(&world, &codex);

    let plan = world.plan("claude", &claude, world.desired("x", "s", "p"), true);
    let added = actions(&plan).iter().all(|(_, _, a)| *a == Action::Add);
    assert!(added, "{:?}", actions(&plan));
    assert_eq!(world.apply(&plan), 0);
    assert_eq!(skill_body(&claude, "x"), "manual skill named like the mcp");
    assert_eq!(
        claude.mcps.borrow()["s"],
        stdio("manual mcp named like the skill")
    );

    let other = world.plan("codex", &codex, Desired::default(), true);
    assert!(actions(&other).is_empty(), "{:?}", actions(&other));
    assert_eq!(world.apply(&other), 0);
    assert_eq!(codex.mcps.borrow()["x"], stdio("manual.js"));
    assert_eq!(skill_body(&codex, "s"), "manual skill");
    assert!(codex.plugins.borrow().contains("p@other-market"));
    assert_eq!(world.saved().agent("codex"), Default::default());
}

#[test]
fn failed_replacement_is_reported_and_not_tracked() {
    let mut world = World::new();
    let claude = world.agent("claude", &["x", "s", "p"]);
    install_by_hand(&world, &claude);

    let plan = world.plan("claude", &claude, world.desired("x", "s", "p"), true);
    assert_eq!(world.apply(&plan), 3);

    assert_eq!(world.saved().agent("claude"), Default::default());
    assert_eq!(claude.mcps.borrow()["x"], stdio("manual.js"));
    assert_eq!(skill_body(&claude, "s"), "manual skill");
    assert!(claude.plugins.borrow().contains("p@other-market"));

    let again = world.plan("claude", &claude, world.desired("x", "s", "p"), false);
    let conflicts = actions(&again)
        .iter()
        .all(|(_, _, a)| *a == Action::Conflict);
    assert!(conflicts, "{:?}", actions(&again));
}
