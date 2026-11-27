//! End to end for git sources, builds, `{dir}`, secrets, cache cleanup and doctor.
//!
//! A second local repo is the source of MCP servers and of a skill. Skipped (with a
//! message) when `claude` is not in PATH; the probe part also needs `node`.

mod common;

use std::path::{Path, PathBuf};

use common::{Env, claude_missing, count, git, path_with};

const ENV_SECRET: &str = "env-secret-value-123";
const FILE_SECRET: &str = "file-secret-value-456";

const PROBE_JS: &str = r#"const fs = require('fs');
fs.writeFileSync(process.env.PROBE_OUT, JSON.stringify({ tok: process.env.PROBE_TOK }));
let buf = '';
process.stdin.on('data', (d) => {
  buf += d;
  let i;
  while ((i = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, i);
    buf = buf.slice(i + 1);
    if (!line.trim()) continue;
    const m = JSON.parse(line);
    if (m.id === undefined) continue;
    const result = m.method === 'initialize'
      ? { protocolVersion: m.params.protocolVersion, capabilities: { tools: {} }, serverInfo: { name: 'probe', version: '1' } }
      : { tools: [] };
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: m.id, result }) + '\n');
  }
});
"#;

struct Source {
    dir: PathBuf,
    url: String,
}

impl Source {
    fn new(env: &Env) -> Source {
        let dir = env.root.path().join("source");
        let skill = dir.join("skills").join("remote-skill");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(
            skill.join("SKILL.md"),
            "---\nname: remote-skill\ndescription: test\n---\nv1\n",
        )
        .unwrap();
        std::fs::write(dir.join("probe.js"), PROBE_JS).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "one"]);
        git(&dir, &["tag", "-a", "v1", "-m", "v1"]);
        let url = dir.to_string_lossy().replace('\\', "/");
        Source { dir, url }
    }

    fn head(&self) -> String {
        git(&self.dir, &["rev-parse", "HEAD"])
    }

    fn commit(&self, file: &str, text: &str) -> String {
        let path = self.dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
        git(&self.dir, &["add", "-A"]);
        git(&self.dir, &["commit", "-q", "-m", file]);
        self.head()
    }
}

struct Yaml<'a> {
    url: &'a str,
    pinned: &'a str,
    broken_build: &'a str,
    with_problems: bool,
    skill_ref: &'a str,
}

impl Yaml<'_> {
    fn render(&self) -> String {
        let url = self.url;
        let problems = if self.with_problems {
            format!(
                r#"  needs:
    command: node
    source:
      git: "{url}"
      ref: v1
      requires: [agsync-no-such-tool-xyz]
      build: ["echo x> never.txt"]
  nosecret:
    command: node
    env: {{T: "${{E2E_MISSING_SECRET}}"}}
"#
            )
        } else {
            String::new()
        };
        format!(
            r#"version: 1
profiles: [work, personal]
mcps:
  tagged:
    command: node
    args: ["{{dir}}/out.txt"]
    env: {{ROOT: "{{dir}}", TOKEN: "${{E2E_ENV_SECRET}}", FILE_TOKEN: "${{E2E_FILE_SECRET}}"}}
    source:
      git: "{url}"
      ref: v1
      requires: [git]
      build: ["echo built> out.txt", "echo second step"]
  pinned:
    command: node
    args: ["{{dir}}/out.txt"]
    source: {{git: "{url}", ref: "{pinned}", build: ["echo pinned> out.txt"]}}
  branchy:
    command: node
    args: ["{{dir}}/out.txt"]
    source: {{git: "{url}", ref: main, build: ["echo branch> out.txt"]}}
  broken:
    command: node
    args: ["{{dir}}/out.txt"]
    source: {{git: "{url}", ref: v1, build: ["{broken}"]}}
  probe:
    command: node
    args: ["{{dir}}/probe.js"]
    env: {{PROBE_TOK: "${{E2E_ENV_SECRET}}", PROBE_OUT: "{{dir}}/probe-out.json"}}
    source: {{git: "{url}", ref: v1}}
  inactive:
    profiles: [personal]
    command: node
    source: {{git: "{url}", ref: v1, build: ["echo x> x.txt"]}}
{problems}skills:
  - git: "{url}"
    ref: {skill_ref}
    path: skills/remote-skill
"#,
            pinned = self.pinned,
            broken = self.broken_build,
            skill_ref = self.skill_ref,
        )
    }
}

fn checkout(env: &Env, name: &str, sha: &str) -> PathBuf {
    env.data.join("sources").join(name).join(sha)
}

fn built(dir: &Path) -> bool {
    dir.join(agsync_core::source::BUILT_MARK).is_file()
}

fn arg0(env: &Env, name: &str) -> String {
    env.args_of(name)[0].as_str().unwrap().to_string()
}

#[test]
fn git_sources_secrets_cleanup_and_doctor() {
    if claude_missing("git_sources_secrets_cleanup_and_doctor") {
        return;
    }
    let mut env = Env::new();
    env.path = Some(path_with(&["git", "claude", "node", "sh"], &[]));
    env.forbidden = vec![ENV_SECRET.to_string(), FILE_SECRET.to_string()];
    env.vars = vec![("E2E_ENV_SECRET".to_string(), ENV_SECRET.to_string())];
    let src = Source::new(&env);
    let sha1 = src.head();
    let mut yaml = Yaml {
        url: &src.url,
        pinned: &sha1,
        broken_build: "exit 7",
        with_problems: true,
        skill_ref: "v1",
    };
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&yaml.render());
    let config_url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &config_url, "--profile", "work"]);

    let (ok, input_text) = env.agsync_with_input(
        &["secret", "set", "E2E_FILE_SECRET"],
        Some(&format!("{FILE_SECRET}\n")),
    );
    assert!(ok, "{input_text}");
    let secrets_file = env.config.join("secrets.env");
    assert!(agsync_core::secret::file::is_private(&secrets_file).unwrap());
    assert_eq!(env.ok(&["secret", "list"]).trim(), "E2E_FILE_SECRET");

    let status = env.ok(&["status"]);
    assert!(status.contains("BUILDS"), "{status}");
    assert!(status.contains("$ echo built> out.txt"), "{status}");
    assert!(
        status.contains(&format!("mcp tagged: {} @ {sha1}", src.url)),
        "{status}"
    );
    assert!(status.contains("$ exit 7"), "{status}");
    assert!(
        status.contains("mcp needs: requires ausente no PATH: agsync-no-such-tool-xyz"),
        "{status}"
    );
    assert!(
        status.contains("agsync secret set E2E_MISSING_SECRET"),
        "{status}"
    );
    assert!(!status.contains("inactive"), "{status}");
    assert!(!env.data.join("sources").join("tagged").exists());
    assert!(!env.data.join("sources").join("inactive").exists());

    let (ok, text) = env.agsync(&["apply", "--yes"]);
    assert!(!ok, "a failed build must fail apply:\n{text}");
    assert!(text.contains("build falhou em 'exit 7' (exit 7)"), "{text}");
    assert!(text.contains("log:"), "{text}");
    let servers = env.servers();
    for name in ["tagged", "pinned", "branchy", "probe"] {
        assert!(servers.contains_key(name), "{name} missing");
    }
    for name in ["broken", "needs", "nosecret", "inactive"] {
        assert!(!servers.contains_key(name), "{name} must be skipped");
    }
    let tagged_dir = checkout(&env, "tagged", &sha1);
    assert!(built(&tagged_dir));
    let out = std::fs::read_to_string(tagged_dir.join("out.txt")).unwrap();
    assert!(out.starts_with("built"), "{out}");
    assert_eq!(
        arg0(&env, "tagged"),
        format!("{}/out.txt", tagged_dir.display())
    );
    let tagged_env = &servers["tagged"]["env"];
    assert_eq!(tagged_env["ROOT"], tagged_dir.display().to_string());
    assert_eq!(tagged_env["TOKEN"], "${E2E_ENV_SECRET}");
    assert_eq!(tagged_env["FILE_TOKEN"], FILE_SECRET);
    assert!(checkout(&env, "pinned", &sha1).join("out.txt").is_file());
    let broken_dir = checkout(&env, "broken", &sha1);
    assert!(broken_dir.is_dir() && !built(&broken_dir));
    assert!(!env.data.join("sources").join("needs").exists());
    assert!(!env.data.join("sources").join("inactive").exists());
    assert!(env.skill("remote-skill").join("SKILL.md").is_file());
    let state = env.state();
    assert!(
        !state.contains(FILE_SECRET) && !state.contains(ENV_SECRET),
        "{state}"
    );
    assert!(state.contains(&sha1), "{state}");

    if agsync_core::which::find("node").is_some() {
        let got = env.claude(&["mcp", "get", "probe"]);
        assert!(got.contains("${E2E_ENV_SECRET}"), "{got}");
        let probe_out = checkout(&env, "probe", &sha1).join("probe-out.json");
        let seen = std::fs::read_to_string(&probe_out).unwrap();
        assert!(
            seen.contains(ENV_SECRET),
            "claude did not expand ${{VAR}}: {seen}"
        );
        eprintln!("claude expanded ${{E2E_ENV_SECRET}} for the stdio server: {seen}");
    } else {
        eprintln!("SKIP probe de expansao: `node` nao esta no PATH");
    }

    yaml.broken_build = "echo fixed> out.txt";
    env.push(&yaml.render());
    let logs_before = count(&env.data.join("logs"));
    let fixed = env.ok(&["apply", "--yes"]);
    assert!(fixed.contains("build mcp broken"), "{fixed}");
    for name in ["tagged", "pinned", "branchy", "probe"] {
        assert!(
            !fixed.contains(&format!("build mcp {name}")),
            "{name} rebuilt:\n{fixed}"
        );
    }
    assert_eq!(count(&env.data.join("logs")), logs_before + 1);
    assert!(built(&broken_dir));
    assert!(env.servers().contains_key("broken"));
    assert!(env.ok(&["status"]).contains("claude:\n  nada a mudar"));

    let sha2 = src.commit("NEWS.md", "two\n");
    let status = env.ok(&["status"]);
    assert!(status.contains("mcp branchy: atualizar"), "{status}");
    assert!(
        status.contains(&format!("mcp branchy: {} @ {sha2}", src.url)),
        "{status}"
    );
    assert!(!status.contains("mcp tagged: atualizar"), "{status}");
    let advanced = env.ok(&["apply", "--yes"]);
    assert!(advanced.contains("limpeza"), "{advanced}");
    let branchy_new = checkout(&env, "branchy", &sha2);
    assert_eq!(
        arg0(&env, "branchy"),
        format!("{}/out.txt", branchy_new.display())
    );
    assert!(built(&branchy_new));
    assert!(!checkout(&env, "branchy", &sha1).exists());
    assert!(checkout(&env, "tagged", &sha1).exists());

    let skill_cache = env.data.join("skills").join("remote-skill");
    let old_cache: Vec<_> = std::fs::read_dir(&skill_cache).unwrap().collect();
    assert_eq!(old_cache.len(), 1);
    let sha3 = src.commit(
        "skills/remote-skill/SKILL.md",
        "---\nname: remote-skill\ndescription: test\n---\nv3\n",
    );
    yaml.skill_ref = "main";
    env.push(&yaml.render());
    env.ok(&["apply", "--yes"]);
    let installed = std::fs::read_to_string(env.skill("remote-skill").join("SKILL.md")).unwrap();
    assert!(installed.contains("v3"), "{installed}");
    assert_eq!(count(&skill_cache), 1);
    assert!(!checkout(&env, "remote-skill", &sha1).exists());
    assert!(checkout(&env, "remote-skill", &sha3).exists());

    let (ok, doctor) = env.agsync(&["doctor"]);
    assert!(!ok, "doctor must fail with problems:\n{doctor}");
    assert!(doctor.contains("git version"), "{doctor}");
    assert!(doctor.contains("agente claude detectado"), "{doctor}");
    assert!(doctor.contains("acessivel so pelo usuario"), "{doctor}");
    assert!(
        doctor.contains("requires ausente no PATH: agsync-no-such-tool-xyz"),
        "{doctor}"
    );
    assert!(
        doctor.contains("segredo E2E_MISSING_SECRET ausente"),
        "{doctor}"
    );
    assert!(
        doctor.contains(&format!("mcp tagged: {}", src.url)),
        "{doctor}"
    );

    yaml.with_problems = false;
    env.push(&yaml.render());
    let doctor = env.ok(&["doctor"]);
    assert!(doctor.contains("tudo certo"), "{doctor}");

    let machine_file = env.config.join("machine.toml");
    let mut machine = agsync_core::machine::Machine::load(&machine_file).unwrap();
    let bad_url = env.root.path().join("no-such-repo");
    machine.repo = bad_url.to_string_lossy().to_string();
    machine.save(&machine_file).unwrap();
    let (ok, doctor) = env.agsync(&["doctor"]);
    assert!(!ok);
    assert!(doctor.contains("repo de config"), "{doctor}");
    assert!(doctor.contains("inacessivel"), "{doctor}");
}

fn skill_md(name: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: test\n---\n{body}\n")
}

#[test]
fn git_skill_paths_share_one_source() {
    if claude_missing("git_skill_paths_share_one_source") {
        return;
    }
    let mut env = Env::new();
    env.path = Some(path_with(&["git", "claude", "sh"], &[]));
    let src = Source::new(&env);
    let sha1 = src.commit("skills/other/SKILL.md", &skill_md("other", "v1"));
    let grouped = format!(
        r#"version: 1
skills:
  - git: "{url}"
    ref: main
    paths:
      - skills/remote-skill
      - skills/other
      - skills/missing
"#,
        url = src.url
    );
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&grouped);
    let config_url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &config_url, "--profile", "work"]);

    let applied = env.ok(&["apply", "--yes"]);
    assert!(
        applied.contains("skill missing: 'skills/missing' sem SKILL.md"),
        "{applied}"
    );
    for name in ["remote-skill", "other"] {
        assert!(env.skill(name).join("SKILL.md").is_file(), "{name} missing");
        assert!(checkout(&env, name, &sha1).is_dir(), "{name} checkout");
    }
    assert!(!env.skill("missing").exists());

    let sha2 = src.commit("skills/other/SKILL.md", &skill_md("other", "v2"));
    let status = env.ok(&["status"]);
    assert!(status.contains("skill other: atualizar"), "{status}");
    env.ok(&["apply", "--yes"]);
    let other = std::fs::read_to_string(env.skill("other").join("SKILL.md")).unwrap();
    assert!(other.contains("v2"), "{other}");
    for name in ["remote-skill", "other"] {
        assert!(checkout(&env, name, &sha2).is_dir(), "{name} checkout");
        assert!(!checkout(&env, name, &sha1).exists(), "{name} old checkout");
    }

    let single = format!(
        r#"version: 1
skills:
  - git: "{url}"
    ref: main
    path: skills/remote-skill
  - git: "{url}"
    ref: main
    path: skills/other
"#,
        url = src.url
    );
    env.push(&single);
    let status = env.ok(&["status"]);
    assert!(status.contains("claude:\n  nada a mudar"), "{status}");
}
