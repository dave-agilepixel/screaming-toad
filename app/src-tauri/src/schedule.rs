//! File > Scheduling: tasks registered with the OS scheduler, which runs the
//! `toad` CLI headless. The CLI is bundled as a Tauri sidecar
//! (`bundle.externalBin`, built by app/scripts/sidecar.mjs), so it is
//! installed next to the app executable.

use crate::{err, CmdResult};
use std::path::PathBuf;
use toad_sched::{RunRecord, ScheduledTask};

/// The CLI next to this executable.
fn toad_cli() -> CmdResult<PathBuf> {
    let exe = std::env::current_exe().map_err(err)?;
    let name = if cfg!(windows) { "toad.exe" } else { "toad" };
    let path = exe.parent().map(|d| d.join(name)).ok_or("no app folder")?;
    if path.exists() {
        Ok(path)
    } else {
        Err(format!(
            "Scheduling runs the toad command-line tool, which ships with the app but wasn't found at {}. Reinstall Screaming Toad (or, in development, run `npm run sidecar:dev` in app/).",
            path.display()
        ))
    }
}

#[tauri::command]
pub fn get_tasks() -> Vec<ScheduledTask> {
    toad_sched::load_tasks()
}

#[tauri::command]
pub fn get_task_history(limit: usize) -> Vec<RunRecord> {
    toad_sched::history(limit)
}

/// Add or update a task and (re-)register it with the OS scheduler.
#[tauri::command]
pub fn save_task(mut task: ScheduledTask) -> CmdResult<ScheduledTask> {
    if task.id.is_empty() {
        task.id = toad_sched::new_id();
    }
    task.validate().map_err(err)?;
    let toad = toad_cli()?;
    toad_sched::register(&task, &toad).map_err(err)?;
    let mut tasks = toad_sched::load_tasks();
    match tasks.iter_mut().find(|t| t.id == task.id) {
        Some(t) => *t = task.clone(),
        None => tasks.push(task.clone()),
    }
    toad_sched::save_tasks(&tasks).map_err(err)?;
    Ok(task)
}

#[tauri::command]
pub fn delete_task(id: String) -> CmdResult<()> {
    let mut tasks = toad_sched::load_tasks();
    if let Some(t) = tasks.iter().find(|t| t.id == id) {
        toad_sched::unregister(t).map_err(err)?;
    }
    tasks.retain(|t| t.id != id);
    toad_sched::save_tasks(&tasks).map_err(err)
}

/// Run a task now, in the background (its result appears in the history).
#[tauri::command]
pub fn run_task_now(id: String) -> CmdResult<()> {
    let toad = toad_cli()?;
    let mut cmd = std::process::Command::new(toad);
    cmd.args(["run-task", &id]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console window pops up.
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn().map_err(err)?;
    Ok(())
}
