//! agsync: applies MCP servers and skills from a remote git config repo
//! to the user-level configuration of coding agents.

pub mod agent;
pub mod apply;
pub mod atomic;
pub mod background;
pub mod check;
pub mod checklog;
pub mod cleanup;
pub mod cmd;
pub mod deliver;
pub mod desired;
pub mod doctor;
pub mod git;
pub mod hash;
pub mod hook;
pub mod lock;
pub mod machine;
pub mod manifest;
pub mod mcp;
pub mod notify;
pub mod paths;
pub mod pending;
pub mod plan;
pub mod prompt;
pub mod report;
pub mod resolved;
pub mod schedule;
pub mod secret;
pub mod shell;
pub mod source;
pub mod state;
pub mod tmp;
pub mod watch;
pub mod which;
