//! agsync: applies MCP servers and skills from a remote git config repo
//! to the user-level configuration of coding agents.

pub mod agent;
pub mod apply;
pub mod atomic;
pub mod cleanup;
pub mod cmd;
pub mod deliver;
pub mod desired;
pub mod git;
pub mod hash;
pub mod machine;
pub mod manifest;
pub mod mcp;
pub mod paths;
pub mod plan;
pub mod report;
pub mod resolved;
pub mod secret;
pub mod source;
pub mod state;
pub mod tmp;
pub mod which;
