//! Terminal text of `apply`: each step as it happens, then the closing lines.

use super::outcome::{Applied, Event};

impl Event {
    pub fn print(&self) {
        match self {
            Event::BuildStarted { name, sha } => println!("build mcp {name} ({sha})..."),
            Event::BuildDone { name } => println!("build mcp {name}: ok"),
            Event::BuildFailed { name, error } => eprintln!("mcp {name}: {error}; item pulado"),
            Event::BackupFailed { agent, error } => {
                eprintln!("{agent}: backup falhou, agente pulado: {error}")
            }
            Event::Written {
                agent,
                kind,
                name,
                action,
            } => println!("{agent}: {kind} {name}: ok ({})", action.label()),
            Event::ItemFailed {
                agent,
                kind,
                name,
                error,
            } => eprintln!("{agent}: {kind} {name} falhou: {error}"),
            Event::StateSaveFailed { error } => eprintln!("gravar state.json falhou: {error}"),
            Event::Cleaned { count } => println!("limpeza: {count} pasta(s) sem uso removida(s)"),
            Event::CleanupFailed { error } => {
                eprintln!("aviso: limpeza do cache falhou: {error}")
            }
        }
    }
}

impl Applied {
    /// Lines after the steps; an error names the failures, if any.
    pub fn finish(&self) -> anyhow::Result<()> {
        if !self.confirmed {
            println!("cancelado");
            return Ok(());
        }
        if !self.pending {
            println!("nada a aplicar");
        }
        if self.mcp_written {
            println!("Mudancas de MCP so valem apos reiniciar o agente.");
        }
        for hint in &self.hints {
            println!("{hint}");
        }
        if self.failures > 0 {
            anyhow::bail!(
                "{} item(ns) falharam; os demais foram aplicados",
                self.failures
            );
        }
        Ok(())
    }
}
