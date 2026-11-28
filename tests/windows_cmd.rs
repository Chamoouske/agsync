//! Native Windows: an MCP whose `command` is a `.cmd` shim (`npx` is `npx.cmd`) starts
//! in Claude Code as written, without a `cmd /c` wrapper. The health check of
//! `claude mcp get` starts each server. Skipped (with a message) without `claude`,
//! `node` or `npx` in PATH.

#![cfg(windows)]

mod common;

use std::path::{Path, PathBuf};

use common::{Env, claude_missing, git, path_with};

const SERVER_JS: &str = r#"#!/usr/bin/env node
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

/// A local npm package with a `bin`, and `probemcp.cmd` in `bin` that starts it.
fn probe(root: &Path) -> (PathBuf, PathBuf) {
    let pkg = root.join("probe pkg");
    std::fs::create_dir_all(pkg.join("bin")).unwrap();
    std::fs::write(pkg.join("bin").join("server.js"), SERVER_JS).unwrap();
    std::fs::write(
        pkg.join("package.json"),
        r#"{"name":"agsync-probe-mcp","version":"1.0.0","bin":{"agsync-probe-mcp":"bin/server.js"}}"#,
    )
    .unwrap();
    let bin = root.join("cmdbin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = format!(
        "@node \"{}\" %*\r\n",
        pkg.join("bin").join("server.js").display()
    );
    std::fs::write(bin.join("probemcp.cmd"), script).unwrap();
    (pkg, bin)
}

#[test]
fn cmd_shims_and_npx_start_in_claude_without_a_wrapper() {
    if claude_missing("cmd_shims_and_npx_start_in_claude_without_a_wrapper") {
        return;
    }
    if agsync_core::which::find("npx").is_none() {
        eprintln!(
            "SKIP cmd_shims_and_npx_start_in_claude_without_a_wrapper: `npx` nao esta no PATH"
        );
        return;
    }
    let mut env = Env::new();
    let (pkg, bin) = probe(env.root.path());
    env.path = Some(path_with(&["git", "sh", "claude", "node", "npx"], &[bin]));
    let npm_cache = env.root.path().join("npm-cache");
    env.vars = vec![(
        "npm_config_cache".to_string(),
        npm_cache.to_string_lossy().to_string(),
    )];
    let pkg = pkg.to_string_lossy().replace('\\', "/");
    let yaml = format!(
        "version: 1\nmcps:\n  shim:\n    command: probemcp\n  viaNpx:\n    command: npx\n    args: [\"--yes\", \"{pkg}\"]\n  missing:\n    command: agsync-no-such-mcp\n"
    );
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&yaml);
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    env.ok(&["apply", "--yes"]);

    assert_eq!(env.servers()["shim"]["command"], "probemcp");
    assert_eq!(env.servers()["viaNpx"]["command"], "npx");
    for name in ["shim", "viaNpx"] {
        let out = env.claude(&["mcp", "get", name]);
        assert!(out.contains("Connected"), "{name}: {out}");
    }
    let out = env.claude(&["mcp", "get", "missing"]);
    assert!(out.contains("Failed to connect"), "{out}");
    assert!(env.ok(&["status"]).contains("nada a mudar"));
}
