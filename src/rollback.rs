//! `agsync rollback` on the terminal: the target, the plan, each step as it runs, then
//! how to go back to the remote head.

use agsync_core::prompt::Confirm;

/// Plans the previous commit, shows the diff, asks and applies it.
pub fn rollback(confirm: &dyn Confirm) -> anyhow::Result<()> {
    let target = agsync_core::rollback::target()?;
    println!("rollback: {} -> {}", target.from, target.to);
    let planned = target.plan()?;
    planned.report.print_plan();
    let to = planned.to.clone();
    let applied = agsync_core::rollback::run(planned, confirm, &mut |event| event.print())?;
    applied.finish()?;
    if !applied.confirmed {
        return Ok(());
    }
    println!("aplicado o commit anterior {to}");
    println!(
        "o `agsync check` volta a acusar o commit remoto como pendente; isso e esperado. Para voltar a ele: `agsync apply`"
    );
    Ok(())
}
