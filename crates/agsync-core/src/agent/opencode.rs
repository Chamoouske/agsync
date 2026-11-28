//! OpenCode adapter.
//!
//! Config dir: `AGSYNC_OPENCODE_CONFIG_DIR`, else `$XDG_CONFIG_HOME/opencode`, else
//! `~/.config/opencode` (same path on every OS). An inherited `OPENCODE_CONFIG_DIR` is
//! ignored, because Orca points it at an overlay: every `opencode` call gets
//! `OPENCODE_CONFIG_DIR=<config dir>`, which is where `mcp add --global` writes.
//!
//! MCP servers are read from `opencode.json` and `opencode.jsonc` (both, the second
//! wins), under `mcp.servers.<name>` (v2) and `mcp.<name>` (v1, still read).
//! Writes go through `opencode mcp add --global`, which only writes v2. OpenCode has no
//! `mcp remove`, so removals and leftover copies of a name are cut from the JSONC text,
//! keeping comments and formatting. Skills are folders under `<config dir>/skills`;
//! OpenCode also loads `~/.claude/skills` and `~/.agents/skills`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use jsonc_parser::ParseOptions;
use jsonc_parser::cst::CstRootNode;
use serde_json::Value;

use super::{Adapter, backup, cli};
use crate::hash::hash_bytes;
use crate::mcp::McpSpec;
use crate::plugin::PluginSpec;
use crate::secret::{substitute, valid_name};

const FILES: [&str; 2] = ["opencode.json", "opencode.jsonc"];

/// OpenCode global config.
#[derive(Debug)]
pub struct OpenCode {
    exe: PathBuf,
    dir: PathBuf,
}

impl OpenCode {
    /// Finds `opencode` in PATH and resolves the global config dir.
    pub fn detect() -> Result<OpenCode, String> {
        let exe = crate::which::require("opencode")?;
        let dir = config_dir().map_err(|e| e.to_string())?;
        Ok(OpenCode::at(exe, dir))
    }

    /// Adapter for an explicit config dir.
    pub fn at(exe: PathBuf, dir: PathBuf) -> OpenCode {
        OpenCode { exe, dir }
    }

    fn files(&self) -> Vec<PathBuf> {
        FILES.iter().map(|name| self.dir.join(name)).collect()
    }

    /// The file `opencode mcp add` writes to: `opencode.json`, unless only the `.jsonc` exists.
    fn add_target(&self) -> PathBuf {
        let [json, jsonc] = FILES.map(|name| self.dir.join(name));
        if !json.exists() && jsonc.exists() {
            return jsonc;
        }
        json
    }

    /// Every MCP entry, v1 and v2, from both files.
    fn servers(&self) -> anyhow::Result<BTreeMap<String, Value>> {
        let mut found = BTreeMap::new();
        for file in self.files() {
            let Some(root) = read(&file)? else {
                continue;
            };
            let Some(mcp) = root.get("mcp").and_then(Value::as_object) else {
                continue;
            };
            for (name, value) in mcp {
                if name != "servers" && value.is_object() {
                    found.insert(name.clone(), value.clone());
                }
            }
            let v2 = mcp.get("servers").and_then(Value::as_object);
            for (name, value) in v2.into_iter().flatten() {
                found.insert(name.clone(), value.clone());
            }
        }
        Ok(found)
    }

    /// Cuts `name` out of both files: the v1 key always, the v2 key except in `keep_v2`.
    fn cut(&self, name: &str, keep_v2: Option<&Path>) -> anyhow::Result<()> {
        for file in self.files() {
            let text = match std::fs::read_to_string(&file) {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e).with_context(|| format!("ler {}", file.display())),
            };
            let cut_v2 = keep_v2 != Some(file.as_path());
            if let Some(edited) = without_entry(&text, name, cut_v2)
                .with_context(|| format!("{} invalido", file.display()))?
            {
                crate::atomic::write_atomic(&file, edited.as_bytes())?;
            }
        }
        Ok(())
    }

    fn written_hash(&self, name: &str) -> anyhow::Result<Option<String>> {
        Ok(self.servers()?.get(name).map(entry_hash))
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.exe);
        cmd.env("OPENCODE_CONFIG_DIR", &self.dir).args(args);
        cmd
    }

    /// Plugin specifiers in the config, from both the v2 `plugins` array and the v1
    /// `plugin` array in either file.
    fn plugin_config(&self) -> anyhow::Result<BTreeSet<String>> {
        let mut found = BTreeSet::new();
        for file in self.files() {
            let Some(root) = read(&file)? else {
                continue;
            };
            for key in ["plugins", "plugin"] {
                for item in root
                    .get(key)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(text) = item.as_str() {
                        found.insert(text.to_string());
                    }
                }
            }
        }
        Ok(found)
    }
}

impl Adapter for OpenCode {
    fn mcps(&self) -> anyhow::Result<BTreeMap<String, String>> {
        let servers = self.servers()?;
        Ok(servers
            .iter()
            .map(|(name, value)| (name.clone(), entry_hash(value)))
            .collect())
    }

    fn mcp_spec(&self, name: &str) -> anyhow::Result<Option<McpSpec>> {
        Ok(self.servers()?.get(name).and_then(to_spec))
    }

    fn skills_dir(&self) -> PathBuf {
        self.dir.join("skills")
    }

    fn backup(&self, dir: &Path) -> anyhow::Result<()> {
        for (file, label) in self.files().iter().zip(FILES) {
            backup::copy(file, dir, label)?;
        }
        Ok(())
    }

    fn write_mcp(&self, name: &str, spec: &McpSpec) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("criar {}", self.dir.display()))?;
        let target = self.add_target();
        let mut cmd = Command::new(&self.exe);
        cmd.env("OPENCODE_CONFIG_DIR", &self.dir)
            .args(["mcp", "add", "--global", name])
            .args(add_args(spec));
        cli::run(cmd, "opencode mcp add", false)?;
        self.cut(name, Some(&target))?;
        if self.written_hash(name)?.as_deref() != Some(spec.hash().as_str()) {
            anyhow::bail!("opencode nao gravou o mcp '{name}' como esperado");
        }
        Ok(())
    }

    fn remove_mcp(&self, name: &str) -> anyhow::Result<()> {
        if self.written_hash(name)?.is_none() {
            return Ok(());
        }
        self.cut(name, None)?;
        if self.written_hash(name)?.is_some() {
            anyhow::bail!("mcp '{name}' continua na config do opencode");
        }
        Ok(())
    }

    fn skill_dirs_read(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.skills_dir()];
        if let Ok(home) = crate::paths::home() {
            dirs.push(home.join(".claude").join("skills"));
            dirs.push(home.join(".agents").join("skills"));
        }
        dirs
    }

    fn plugin_support(&self) -> Result<(), String> {
        cli::output(
            self.command(&["plugin", "--help"]),
            "opencode plugin --help",
        )
        .map(|_| ())
        .map_err(|e| format!("`opencode plugin` indisponivel: {e:#}"))
    }

    fn plugin_installed(&self, spec: &PluginSpec) -> anyhow::Result<Option<String>> {
        let package = spec.opencode_package();
        Ok(self.plugin_config()?.contains(&package).then_some(package))
    }

    fn plugin_present(&self, id: &str) -> anyhow::Result<bool> {
        Ok(self.plugin_config()?.contains(id))
    }

    fn install_plugin(
        &self,
        spec: &PluginSpec,
        _managed: &BTreeSet<String>,
    ) -> anyhow::Result<String> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("criar {}", self.dir.display()))?;
        let package = spec.opencode_package();
        cli::run(
            self.command(&["plugin", "add", &package]),
            "opencode plugin add",
            false,
        )?;
        if !self.plugin_present(&package)? {
            anyhow::bail!("opencode nao gravou o plugin '{package}' como esperado");
        }
        Ok(package)
    }

    fn uninstall_plugin(&self, id: &str) -> anyhow::Result<()> {
        if !self.plugin_present(id)? {
            return Ok(());
        }
        cli::run(
            self.command(&["plugin", "remove", id]),
            "opencode plugin remove",
            false,
        )?;
        if self.plugin_present(id)? {
            anyhow::bail!("plugin '{id}' continua na config do opencode");
        }
        Ok(())
    }

    fn reload_hint(&self) -> Option<&'static str> {
        Some(
            "opencode: rode `opencode reload` ou reinicie o servico \
             (`opencode service restart`) para carregar as mudancas",
        )
    }
}

fn config_dir() -> anyhow::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("AGSYNC_OPENCODE_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let xdg = std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty());
    if let Some(dir) = xdg {
        return Ok(PathBuf::from(dir).join("opencode"));
    }
    Ok(crate::paths::home()?.join(".config").join("opencode"))
}

/// JSONC file as a JSON value; `None` when the file or its value is missing.
fn read(file: &Path) -> anyhow::Result<Option<Value>> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("ler {}", file.display())),
    };
    let root = CstRootNode::parse(&text, &ParseOptions::default())
        .map_err(|e| anyhow::anyhow!("{} invalido: {e}", file.display()))?;
    Ok(root.to_serde_value())
}

/// `text` without the v1 entry `mcp.<name>` and, if `cut_v2`, without `mcp.servers.<name>`.
/// `None` when nothing was there.
fn without_entry(text: &str, name: &str, cut_v2: bool) -> anyhow::Result<Option<String>> {
    let root =
        CstRootNode::parse(text, &ParseOptions::default()).map_err(|e| anyhow::anyhow!("{e}"))?;
    let Some(mcp) = root.object_value().and_then(|o| o.object_value("mcp")) else {
        return Ok(None);
    };
    let mut changed = false;
    let v1 = mcp.get(name).filter(|p| p.object_value().is_some());
    if let Some(prop) = v1.filter(|_| name != "servers") {
        prop.remove();
        changed = true;
    }
    let v2 = mcp.object_value("servers").and_then(|s| s.get(name));
    if let Some(prop) = v2.filter(|_| cut_v2) {
        prop.remove();
        changed = true;
    }
    Ok(changed.then(|| root.to_string()))
}

fn entry_hash(value: &Value) -> String {
    match to_spec(value) {
        Some(spec) => spec.hash(),
        None => hash_bytes(value.to_string().as_bytes()),
    }
}

/// The neutral form of an OpenCode entry, with `{env:VAR}` read back as `${VAR}`.
fn to_spec(value: &Value) -> Option<McpSpec> {
    let text = |v: &Value| v.as_str().map(from_native);
    match value.get("type")?.as_str()? {
        "local" => {
            let parts = value.get("command")?.as_array()?;
            let mut parts = parts.iter().map(text).collect::<Option<Vec<_>>>()?;
            if parts.is_empty() {
                return None;
            }
            let command = parts.remove(0);
            let env = strings(value.get("environment"))?;
            Some(McpSpec::Stdio {
                command,
                args: parts,
                env,
            })
        }
        "remote" => {
            let url = text(value.get("url")?)?;
            let headers = strings(value.get("headers"))?;
            Some(McpSpec::Http { url, headers })
        }
        _ => None,
    }
}

/// `opencode mcp add` arguments after the name, with `${VAR}` as `{env:VAR}`.
fn add_args(spec: &McpSpec) -> Vec<String> {
    let mut out = Vec::new();
    match spec {
        McpSpec::Stdio { command, args, env } => {
            for (key, value) in env {
                out.push("--env".to_string());
                out.push(format!("{key}={}", to_native(value)));
            }
            out.push("--".to_string());
            out.push(to_native(command));
            out.extend(args.iter().map(|a| to_native(a)));
        }
        McpSpec::Http { url, headers } => {
            out.push("--url".to_string());
            out.push(to_native(url));
            for (key, value) in headers {
                out.push("--header".to_string());
                out.push(format!("{key}={}", to_native(value)));
            }
        }
    }
    out
}

/// String map of an optional JSON object; `None` if any value is not a string.
fn strings(value: Option<&Value>) -> Option<BTreeMap<String, String>> {
    let Some(value) = value else {
        return Some(BTreeMap::new());
    };
    value
        .as_object()?
        .iter()
        .map(|(k, v)| v.as_str().map(|v| (k.clone(), from_native(v))))
        .collect()
}

/// `${VAR}` to OpenCode's `{env:VAR}`.
fn to_native(text: &str) -> String {
    substitute(text, |name| Some(format!("{{env:{name}}}")))
}

/// OpenCode's `{env:VAR}` back to `${VAR}`.
fn from_native(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{env:") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 5..];
        match after.find('}').map(|end| &after[..end]) {
            Some(name) if valid_name(name) => {
                out.push_str(&format!("${{{name}}}"));
                rest = &after[name.len() + 1..];
            }
            _ => {
                out.push_str("{env:");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    const JSONC: &str = r#"{
  // keep me
  "theme": "dark", /* and me */
  "mcp": {
    "legacy": { "type": "local", "command": ["old-tool"] },
    "servers": {
      "keep": { "type": "local", "command": ["keep", "{env:TOKEN}"], },
      "gone": { "type": "local", "command": ["gone"] },
    },
  },
}
"#;

    #[test]
    fn env_syntax_round_trips() {
        assert_eq!(to_native("a ${X} ${1} b"), "a {env:X} ${1} b");
        assert_eq!(
            from_native("a {env:X} {env:} {env:1A} b"),
            "a ${X} {env:} {env:1A} b"
        );
        assert_eq!(from_native(&to_native("--t=${TOK}")), "--t=${TOK}");
    }

    #[test]
    fn reads_v1_and_v2_from_both_files() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let oc = OpenCode::at(PathBuf::from("opencode-not-called"), tmp.path().into());
        assert!(oc.mcps().unwrap().is_empty());
        std::fs::write(tmp.path().join("opencode.jsonc"), JSONC).unwrap();
        let json = r#"{"mcp":{"servers":{"gone":{"type":"local","command":["other"]}}}}"#;
        std::fs::write(tmp.path().join("opencode.json"), json).unwrap();

        let mcps = oc.mcps().unwrap();
        assert_eq!(mcps.keys().collect::<Vec<_>>(), ["gone", "keep", "legacy"]);
        let keep = McpSpec::Stdio {
            command: "keep".into(),
            args: vec!["${TOKEN}".into()],
            env: BTreeMap::new(),
        };
        assert_eq!(mcps["keep"], keep.hash());
        let gone = McpSpec::Stdio {
            command: "gone".into(),
            args: vec![],
            env: BTreeMap::new(),
        };
        assert_eq!(mcps["gone"], gone.hash());
        assert_eq!(oc.add_target(), tmp.path().join("opencode.json"));
    }

    #[test]
    fn reads_plugins_from_both_keys_and_files() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let oc = OpenCode::at(PathBuf::from("opencode-not-called"), tmp.path().into());
        assert!(oc.plugin_config().unwrap().is_empty());

        let jsonc = r#"{"plugins": ["git+https://x/v2.git#v2", 7]}"#;
        let json = r#"{"plugin": ["git+https://x/v1.git#v1"]}"#;
        std::fs::write(tmp.path().join("opencode.jsonc"), jsonc).unwrap();
        std::fs::write(tmp.path().join("opencode.json"), json).unwrap();
        let found = oc.plugin_config().unwrap();
        assert_eq!(
            found.iter().map(String::as_str).collect::<Vec<_>>(),
            ["git+https://x/v1.git#v1", "git+https://x/v2.git#v2"]
        );
    }

    #[test]
    fn cut_keeps_comments_and_other_entries() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let file = tmp.path().join("opencode.jsonc");
        std::fs::write(&file, JSONC).unwrap();
        let oc = OpenCode::at(PathBuf::from("opencode-not-called"), tmp.path().into());
        assert_eq!(oc.add_target(), file);

        oc.remove_mcp("gone").unwrap();
        oc.remove_mcp("legacy").unwrap();
        oc.remove_mcp("never-there").unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("// keep me"), "{text}");
        assert!(text.contains("/* and me */"), "{text}");
        assert!(text.contains(r#""keep": { "type": "local""#), "{text}");
        assert!(!text.contains("gone") && !text.contains("legacy"), "{text}");
        assert_eq!(oc.mcps().unwrap().keys().collect::<Vec<_>>(), ["keep"]);

        assert_eq!(without_entry(&text, "keep", false).unwrap(), None);
    }

    #[test]
    fn reads_one_local_entry() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let opencode = OpenCode::at(PathBuf::from("opencode-not-called"), tmp.path().into());
        let file = r#"{"mcp": {"1mcp": {"type": "local", "command": ["1mcp", "proxy"]}}}"#;
        std::fs::write(tmp.path().join("opencode.json"), file).unwrap();
        let legacy = McpSpec::Stdio {
            command: "1mcp".into(),
            args: vec!["proxy".into()],
            env: BTreeMap::new(),
        };
        assert_eq!(opencode.mcp_spec("1mcp").unwrap(), Some(legacy));
        assert_eq!(opencode.mcp_spec("other").unwrap(), None);
    }
}
