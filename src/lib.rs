//! agsync: applies MCP servers and skills from a remote git config repo
//! to the user-level configuration of coding agents.

pub mod agent;
pub mod atomic;
pub mod desired;
pub mod git;
pub mod hash;
pub mod machine;
pub mod manifest;
pub mod mcp;
pub mod paths;
pub mod plan;
pub mod state;
pub mod tmp;
