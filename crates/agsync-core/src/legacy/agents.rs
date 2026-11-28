//! The `1mcp` proxy entry in each detected agent. It is not in `state.json`: this is
//! the one place agsync removes an entry it did not write, and only `1mcp` running
//! `command: 1mcp`.

use std::path::Path;

use super::Leftover;
use crate::agent::{AGENTS, Adapter};
use crate::mcp::McpSpec;

const NAME: &str = "1mcp";

struct ProxyEntry {
    agent: &'static str,
    adapter: Box<dyn Adapter>,
}

impl Leftover for ProxyEntry {
    fn category(&self) -> &'static str {
        "MCP `1mcp` nos agentes"
    }

    fn describe(&self) -> String {
        format!("{}: mcp {NAME} (1mcp proxy)", self.agent)
    }

    fn remove(&self, backup: &Path) -> anyhow::Result<()> {
        self.adapter.backup(backup)?;
        self.adapter.remove_mcp(NAME)
    }
}

/// The agents found here whose `1mcp` entry runs `1mcp`.
pub fn leftovers() -> Vec<Box<dyn Leftover>> {
    let mut found: Vec<Box<dyn Leftover>> = Vec::new();
    for (agent, detect) in AGENTS {
        let Ok(adapter) = detect() else {
            continue;
        };
        match adapter.mcp_spec(NAME) {
            Ok(Some(spec)) if is_proxy(&spec) => {
                found.push(Box::new(ProxyEntry { agent, adapter }))
            }
            Ok(_) => {}
            Err(e) => eprintln!("aviso: {agent}: ler a config falhou: {e:#}"),
        }
    }
    found
}

fn is_proxy(spec: &McpSpec) -> bool {
    matches!(spec, McpSpec::Stdio { command, .. } if command == NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn only_a_stdio_entry_running_1mcp_is_the_proxy() {
        let stdio = |command: &str| McpSpec::Stdio {
            command: command.into(),
            args: vec!["proxy".into()],
            env: BTreeMap::new(),
        };
        assert!(is_proxy(&stdio("1mcp")));
        assert!(!is_proxy(&stdio("node")));
        let http = McpSpec::Http {
            url: "http://127.0.0.1:3050/mcp".into(),
            headers: BTreeMap::new(),
        };
        assert!(!is_proxy(&http));
    }
}
