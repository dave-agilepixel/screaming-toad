//! Registers a disabled task with the OS scheduler, shows it, removes it.
fn main() -> anyhow::Result<()> {
    let task = toad_sched::ScheduledTask {
        id: "selftest".into(),
        name: "Registration self-test".into(),
        enabled: false,
        url: "https://example.com/".into(),
        start: "2030-01-07 06:30".into(),
        output_folder: std::env::temp_dir().display().to_string(),
        ..Default::default()
    };
    toad_sched::register(&task, &std::env::current_exe()?)?;
    let out = std::process::Command::new("schtasks").args(["/Query", "/TN", &task.os_name(), "/XML"]).output()?;
    let xml = String::from_utf8_lossy(&out.stdout);
    println!("registered: {}", out.status.success());
    println!("has trigger: {}", xml.contains("2030-01-07T06:30:00") && xml.contains("Monday"));
    toad_sched::unregister(&task)?;
    let gone = !std::process::Command::new("schtasks").args(["/Query", "/TN", &task.os_name()]).output()?.status.success();
    println!("removed: {gone}");
    Ok(())
}
