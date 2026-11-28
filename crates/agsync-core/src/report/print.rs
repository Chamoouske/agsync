//! Terminal text of `status`, `diff` and the plan apply shows before asking.

use super::view::{ChangeView, DiffView, StatusView};
use crate::plan::Action;

impl StatusView {
    pub fn print(&self) {
        let applied = self.applied.as_deref().unwrap_or("nenhum");
        println!("repo: {} ({})", self.repo, self.git_ref);
        println!("perfis ativos: {}", self.profiles.join(", "));
        println!("commit aplicado: {applied}");
        println!("commit remoto:   {}", self.remote);
        self.diff.print_plan(Some(&self.skipped));
    }
}

impl DiffView {
    pub fn print(&self) {
        self.print_plan(None);
    }

    /// Changes, manual installs, builds, then `skipped` when given, then warnings.
    pub fn print_plan(&self, skipped: Option<&[String]>) {
        self.print_changes();
        self.print_manual();
        self.print_builds();
        if let Some(skipped) = skipped {
            print_skipped(skipped);
        }
        for line in &self.warnings {
            println!("aviso: {line}");
        }
    }

    fn print_changes(&self) {
        for agent in &self.agents {
            println!("{}:", agent.agent);
            for change in &agent.changes {
                println!(
                    "  {} {}: {}{}{}",
                    change.kind,
                    change.name,
                    change.action.label(),
                    detail_note(change),
                    origin_note(change.origin.as_deref())
                );
            }
            if agent.changes.is_empty() {
                println!("  nada a mudar");
            }
        }
    }

    /// Lists manual installs that match a declared item by name, kind and agent: the
    /// ones apply replaces (`--adopt`) or the ones it leaves alone.
    fn print_manual(&self) {
        let replaced = self.manual(Action::Adopt);
        if !replaced.is_empty() {
            println!();
            println!(
                "!!! INSTALACOES MANUAIS que serao substituidas pela definicao do agsync.yaml:"
            );
            replaced.iter().for_each(|line| println!("{line}"));
        }
        let kept = self.manual(Action::Conflict);
        if !kept.is_empty() {
            println!();
            println!(
                "instalacoes manuais com o mesmo nome no agsync.yaml (`agsync apply --adopt` substitui):"
            );
            kept.iter().for_each(|line| println!("{line}"));
        }
    }

    fn manual(&self, action: Action) -> Vec<String> {
        let mut lines = Vec::new();
        for agent in &self.agents {
            for change in agent.changes.iter().filter(|c| c.action == action) {
                lines.push(format!(
                    "  {}: {} {}{}",
                    agent.agent,
                    change.kind,
                    change.name,
                    detail_note(change)
                ));
            }
        }
        lines
    }

    fn print_builds(&self) {
        if self.builds.is_empty() {
            return;
        }
        println!();
        println!("!!! BUILDS: estes comandos executam codigo de terceiros nesta maquina !!!");
        for job in &self.builds {
            println!(
                "  mcp {}: {} @ {} ({}){}",
                job.name,
                job.repo,
                job.sha,
                job.git_ref,
                origin_note(job.origin.as_deref())
            );
            if job.commands.is_empty() {
                println!("    (so checkout, nenhum comando)");
            }
            for command in &job.commands {
                println!("    $ {command}");
            }
        }
        println!();
    }
}

fn print_skipped(skipped: &[String]) {
    if skipped.is_empty() {
        return;
    }
    println!("pulados:");
    for line in skipped {
        println!("  {line}");
    }
}

fn detail_note(change: &ChangeView) -> String {
    change
        .detail
        .as_ref()
        .map(|detail| format!(" [{detail}]"))
        .unwrap_or_default()
}

/// ` (de imports.<kind> <git>@<ref>)` for an imported item, empty for the main repo's.
pub fn origin_note(origin: Option<&str>) -> String {
    origin
        .map(|origin| format!(" (de {origin})"))
        .unwrap_or_default()
}
