//! File > Scheduling. Tasks are saved in ~/.toad/schedules.json and
//! registered with the OS scheduler (Windows Task Scheduler, launchd on
//! macOS, cron elsewhere), which runs `toad run-task <id>` headless at the
//! set times. Each run is appended to ~/.toad/schedule_history.jsonl.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use toad_store::export::MultiExport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    #[default]
    Once,
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScheduledTask {
    pub id: String,
    pub name: String,
    pub project: String,
    pub enabled: bool,
    /// Spider mode start URL.
    pub url: String,
    /// List mode: a file of URLs instead of a start URL.
    pub list_file: String,
    /// A configuration profile (.toadconfig); empty for the defaults.
    pub config_profile: String,
    pub frequency: Frequency,
    /// First run, local time, "YYYY-MM-DD HH:MM".
    pub start: String,
    /// Weekly: 0 = Monday … 6 = Sunday (defaults to the start date's day).
    pub weekday: Option<u8>,
    pub output_folder: String,
    pub exports: MultiExport,
    pub create_sitemap: bool,
    pub create_images_sitemap: bool,
    /// API Access as set in the profile (otherwise APIs are off).
    pub use_apis: bool,
    /// Email the summary (to these addresses, or the default recipients).
    pub email: bool,
    pub email_to: Vec<String>,
    /// Compare with this task's previous crawl; the email shows the changes.
    pub auto_compare: bool,
    pub overwrite: bool,
}

impl Default for ScheduledTask {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            project: String::new(),
            enabled: true,
            url: String::new(),
            list_file: String::new(),
            config_profile: String::new(),
            frequency: Frequency::Weekly,
            start: String::new(),
            weekday: None,
            output_folder: String::new(),
            exports: MultiExport::default(),
            create_sitemap: false,
            create_images_sitemap: false,
            use_apis: false,
            email: false,
            email_to: vec![],
            auto_compare: true,
            overwrite: false,
        }
    }
}

/// When a task ran and how it went.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RunRecord {
    pub task_id: String,
    pub task_name: String,
    /// Unix seconds.
    pub started: u64,
    pub finished: u64,
    pub ok: bool,
    pub message: String,
    pub crawl: String,
}

/// ~/.toad, or $TOAD_HOME when set (tests, portable installs).
fn toad_dir() -> PathBuf {
    if let Some(h) = std::env::var_os("TOAD_HOME") {
        return PathBuf::from(h);
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    home.join(".toad")
}

pub fn tasks_path() -> PathBuf {
    toad_dir().join("schedules.json")
}

pub fn history_path() -> PathBuf {
    toad_dir().join("schedule_history.jsonl")
}

pub fn load_tasks() -> Vec<ScheduledTask> {
    std::fs::read_to_string(tasks_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_tasks(tasks: &[ScheduledTask]) -> anyhow::Result<()> {
    std::fs::create_dir_all(toad_dir())?;
    std::fs::write(tasks_path(), serde_json::to_string_pretty(tasks)?)?;
    Ok(())
}

pub fn append_history(r: &RunRecord) -> anyhow::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(toad_dir())?;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(history_path())?;
    writeln!(f, "{}", serde_json::to_string(r)?)?;
    Ok(())
}

/// Most recent first.
pub fn history(limit: usize) -> Vec<RunRecord> {
    let text = std::fs::read_to_string(history_path()).unwrap_or_default();
    let mut v: Vec<RunRecord> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    v.reverse();
    v.truncate(limit);
    v
}

/// A new task ID (time-based, unique enough for one machine).
pub fn new_id() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("t{:x}", nanos)
}

/// "YYYY-MM-DD HH:MM" → (year, month, day, hour, minute).
pub fn parse_start(s: &str) -> anyhow::Result<(i32, u32, u32, u32, u32)> {
    let s = s.trim().replace('T', " ");
    let (date, time) = s.split_once(' ').ok_or_else(|| anyhow::anyhow!("start must be YYYY-MM-DD HH:MM"))?;
    let d: Vec<&str> = date.split('-').collect();
    let t: Vec<&str> = time.split(':').collect();
    anyhow::ensure!(d.len() == 3 && t.len() >= 2, "start must be YYYY-MM-DD HH:MM");
    let parsed = (d[0].parse()?, d[1].parse()?, d[2].parse()?, t[0].parse()?, t[1].parse()?);
    anyhow::ensure!((1..=12).contains(&parsed.1) && (1..=31).contains(&parsed.2) && parsed.3 < 24 && parsed.4 < 60, "start is not a valid date and time");
    Ok(parsed)
}

/// Day of week for a date: 0 = Monday … 6 = Sunday.
pub fn weekday(y: i32, m: u32, d: u32) -> u8 {
    // Sakamoto's algorithm (0 = Sunday), shifted to Monday-first.
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    let sun0 = (y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d as i32).rem_euclid(7);
    ((sun0 + 6) % 7) as u8
}

const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

impl ScheduledTask {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.name.trim().is_empty(), "give the task a name");
        anyhow::ensure!(!self.url.trim().is_empty() || !self.list_file.trim().is_empty(), "enter a start URL or a list file");
        anyhow::ensure!(!self.output_folder.trim().is_empty(), "choose an output folder");
        parse_start(&self.start)?;
        anyhow::ensure!(!self.config_profile.trim().is_empty() || !self.use_apis, "API Access needs a configuration profile with the APIs set up");
        Ok(())
    }

    /// "Weekly on Monday at 06:00", for the task list.
    pub fn describe(&self) -> String {
        let Ok((y, m, d, h, min)) = parse_start(&self.start) else { return "Invalid start".into() };
        match self.frequency {
            Frequency::Once => format!("Once, {y:04}-{m:02}-{d:02} at {h:02}:{min:02}"),
            Frequency::Daily => format!("Daily at {h:02}:{min:02}"),
            Frequency::Weekly => format!("Weekly on {} at {h:02}:{min:02}", DAYS[self.weekday.unwrap_or_else(|| weekday(y, m, d)) as usize % 7]),
            Frequency::Monthly => format!("Monthly on day {d} at {h:02}:{min:02}"),
        }
    }

    /// The name registered with the OS scheduler.
    pub fn os_name(&self) -> String {
        let clean: String = self.name.chars().filter(|c| c.is_alphanumeric() || " -_.".contains(*c)).collect();
        format!("Screaming Toad - {} ({})", clean.trim(), self.id)
    }

    /// The `toad crawl` arguments this task runs with.
    pub fn crawl_args(&self) -> Vec<String> {
        let mut a: Vec<String> = vec!["crawl".into()];
        if self.list_file.trim().is_empty() {
            a.push(self.url.trim().into());
        } else {
            a.extend(["--list".into(), self.list_file.trim().into()]);
        }
        if !self.config_profile.trim().is_empty() {
            a.extend(["--config".into(), self.config_profile.trim().into()]);
        }
        a.extend(["--output-folder".into(), self.output_folder.trim().into(), "--timestamped-output".into(), "--task-name".into(), self.name.trim().into()]);
        if !self.project.trim().is_empty() {
            a.extend(["--project-name".into(), self.project.trim().into()]);
        }
        let tabs: Vec<&str> = self.exports.items.iter().filter(|i| !i.0.starts_with("report:")).map(|i| i.0.as_str()).collect();
        let reports: Vec<&str> = self.exports.items.iter().filter_map(|i| i.0.strip_prefix("report:")).collect();
        if !tabs.is_empty() {
            a.extend(["--export-tabs".into(), tabs.join(",")]);
        }
        if !reports.is_empty() {
            a.extend(["--save-report".into(), reports.join(",")]);
        }
        if !self.exports.items.is_empty() {
            a.extend(["--export-format".into(), self.exports.format.ext().into()]);
            if self.exports.consolidate {
                a.push("--consolidate".into());
            }
            if self.exports.skip_empty {
                a.push("--skip-empty".into());
            }
        }
        if self.create_sitemap {
            a.push("--create-sitemap".into());
        }
        if self.create_images_sitemap {
            a.push("--create-images-sitemap".into());
        }
        if self.use_apis {
            a.push("--use-profile-apis".into());
        }
        if self.auto_compare {
            a.push("--compare-with-previous".into());
        }
        if self.email {
            a.push(format!("--email-on-complete={}", self.email_to.join(",")));
        }
        if self.overwrite {
            a.push("--overwrite".into());
        }
        a
    }
}

/// Windows Task Scheduler definition (locale-independent, unlike
/// `schtasks /SC … /SD …`). Runs only while the user is logged on, so no
/// password is stored.
pub fn task_xml(task: &ScheduledTask, toad: &Path) -> anyhow::Result<String> {
    let (y, m, d, h, min) = parse_start(&task.start)?;
    let start = format!("{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:00");
    let trigger = match task.frequency {
        Frequency::Once => format!("<TimeTrigger><StartBoundary>{start}</StartBoundary><Enabled>true</Enabled></TimeTrigger>"),
        Frequency::Daily => format!("<CalendarTrigger><StartBoundary>{start}</StartBoundary><Enabled>true</Enabled><ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay></CalendarTrigger>"),
        Frequency::Weekly => {
            let day = DAYS[task.weekday.unwrap_or_else(|| weekday(y, m, d)) as usize % 7];
            format!("<CalendarTrigger><StartBoundary>{start}</StartBoundary><Enabled>true</Enabled><ScheduleByWeek><WeeksInterval>1</WeeksInterval><DaysOfWeek><{day} /></DaysOfWeek></ScheduleByWeek></CalendarTrigger>")
        }
        Frequency::Monthly => {
            let months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"]
                .iter()
                .map(|m| format!("<{m} />"))
                .collect::<String>();
            format!("<CalendarTrigger><StartBoundary>{start}</StartBoundary><Enabled>true</Enabled><ScheduleByMonth><DaysOfMonth><Day>{d}</Day></DaysOfMonth><Months>{months}</Months></ScheduleByMonth></CalendarTrigger>")
        }
    };
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>{}</Description></RegistrationInfo>
  <Triggers>{trigger}</Triggers>
  <Principals><Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <ExecutionTimeLimit>PT72H</ExecutionTimeLimit>
    <Enabled>{}</Enabled>
  </Settings>
  <Actions Context="Author"><Exec><Command>{}</Command><Arguments>run-task {}</Arguments></Exec></Actions>
</Task>
"#,
        esc(&format!("Screaming Toad scheduled crawl: {}", task.name)),
        task.enabled,
        esc(&toad.display().to_string()),
        esc(&task.id)
    ))
}

/// A crontab line (Linux): `m h dom mon dow command # toad-task:<id>`.
pub fn cron_line(task: &ScheduledTask, toad: &Path) -> anyhow::Result<String> {
    let (y, m, d, h, min) = parse_start(&task.start)?;
    let when = match task.frequency {
        // cron can't run once; a date-specific line runs every year on that day.
        Frequency::Once => format!("{min} {h} {d} {m} *"),
        Frequency::Daily => format!("{min} {h} * * *"),
        Frequency::Weekly => format!("{min} {h} * * {}", (task.weekday.unwrap_or_else(|| weekday(y, m, d)) + 1) % 7),
        Frequency::Monthly => format!("{min} {h} {d} * *"),
    };
    Ok(format!("{when} '{}' run-task {} # toad-task:{}", toad.display(), task.id, task.id))
}

/// A launchd agent (macOS).
pub fn launchd_plist(task: &ScheduledTask, toad: &Path) -> anyhow::Result<String> {
    let (y, m, d, h, min) = parse_start(&task.start)?;
    let mut interval = format!("<key>Hour</key><integer>{h}</integer><key>Minute</key><integer>{min}</integer>");
    match task.frequency {
        Frequency::Once => interval.push_str(&format!("<key>Day</key><integer>{d}</integer><key>Month</key><integer>{m}</integer>")),
        Frequency::Daily => {}
        Frequency::Weekly => interval.push_str(&format!("<key>Weekday</key><integer>{}</integer>", (task.weekday.unwrap_or_else(|| weekday(y, m, d)) + 1) % 7)),
        Frequency::Monthly => interval.push_str(&format!("<key>Day</key><integer>{d}</integer>")),
    }
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>com.screamingtoad.task.{}</string>
<key>ProgramArguments</key><array><string>{}</string><string>run-task</string><string>{}</string></array>
<key>StartCalendarInterval</key><dict>{interval}</dict>
</dict></plist>
"#,
        task.id,
        toad.display(),
        task.id
    ))
}

/// Register (or replace) a task with the OS scheduler.
pub fn register(task: &ScheduledTask, toad: &Path) -> anyhow::Result<()> {
    task.validate()?;
    #[cfg(windows)]
    {
        let xml = task_xml(task, toad)?;
        let file = std::env::temp_dir().join(format!("toad-task-{}.xml", task.id));
        // schtasks wants UTF-16 for a UTF-16-declared document.
        let mut bytes = vec![0xFF, 0xFE];
        for u in xml.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        std::fs::write(&file, bytes)?;
        let out = std::process::Command::new("schtasks").args(["/Create", "/F", "/TN", &task.os_name(), "/XML"]).arg(&file).output()?;
        let _ = std::fs::remove_file(&file);
        anyhow::ensure!(out.status.success(), "Task Scheduler: {}", String::from_utf8_lossy(&out.stderr).trim());
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let dir = PathBuf::from(std::env::var("HOME")?).join("Library/LaunchAgents");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("com.screamingtoad.task.{}.plist", task.id));
        let _ = std::process::Command::new("launchctl").arg("unload").arg(&path).output();
        std::fs::write(&path, launchd_plist(task, toad)?)?;
        if task.enabled {
            std::process::Command::new("launchctl").arg("load").arg(&path).output()?;
        }
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut lines = crontab_without(&task.id)?;
        if task.enabled {
            lines.push(cron_line(task, toad)?);
        }
        write_crontab(&lines)
    }
}

/// Remove a task from the OS scheduler (missing is fine).
pub fn unregister(task: &ScheduledTask) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("schtasks").args(["/Delete", "/F", "/TN", &task.os_name()]).output()?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let path = PathBuf::from(std::env::var("HOME")?).join(format!("Library/LaunchAgents/com.screamingtoad.task.{}.plist", task.id));
        let _ = std::process::Command::new("launchctl").arg("unload").arg(&path).output();
        let _ = std::fs::remove_file(path);
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        write_crontab(&crontab_without(&task.id)?)
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn crontab_without(id: &str) -> anyhow::Result<Vec<String>> {
    let out = std::process::Command::new("crontab").arg("-l").output()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    Ok(text.lines().filter(|l| !l.ends_with(&format!("# toad-task:{id}"))).map(str::to_string).collect())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn write_crontab(lines: &[String]) -> anyhow::Result<()> {
    use std::io::Write;
    let mut child = std::process::Command::new("crontab").arg("-").stdin(std::process::Stdio::piped()).spawn()?;
    child.stdin.as_mut().ok_or_else(|| anyhow::anyhow!("crontab"))?.write_all(format!("{}\n", lines.join("\n")).as_bytes())?;
    anyhow::ensure!(child.wait()?.success(), "crontab failed");
    Ok(())
}

/// The latest finished crawl of a task before `exclude`, by modification
/// time, under its output folder (timestamped runs land in
/// `<output>/<task>/<timestamp>/crawl.toad`).
pub fn previous_crawl(output_folder: &Path, exclude: &Path) -> Option<PathBuf> {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    let mut stack = vec![output_folder.to_path_buf()];
    let exclude = std::fs::canonicalize(exclude).unwrap_or_else(|_| exclude.to_path_buf());
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "toad") && std::fs::canonicalize(&p).unwrap_or_else(|_| p.clone()) != exclude {
                let complete = toad_store::Store::open(&p).ok().and_then(|s| s.meta("status").ok().flatten()).as_deref() == Some("complete");
                if complete {
                    if let Ok(t) = e.metadata().and_then(|m| m.modified()) {
                        found.push((t, p));
                    }
                }
            }
        }
    }
    found.sort();
    found.pop().map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use toad_store::export::{ExportFormat, ExportItem};

    fn task() -> ScheduledTask {
        ScheduledTask {
            id: "t1".into(),
            name: "Weekly audit".into(),
            project: "Client".into(),
            url: "https://www.example.com/".into(),
            config_profile: "C:/p/audit.toadconfig".into(),
            frequency: Frequency::Weekly,
            start: "2026-10-19 06:30".into(),
            output_folder: "C:/crawls".into(),
            exports: MultiExport {
                items: vec![ExportItem::tab("Internal", "All"), ExportItem::report("Crawl Overview")],
                format: ExportFormat::Xlsx,
                consolidate: true,
                skip_empty: false,
            },
            email: true,
            email_to: vec!["a@x.com".into()],
            ..Default::default()
        }
    }

    #[test]
    fn weekdays() {
        assert_eq!(weekday(2026, 10, 19), 0, "a Monday");
        assert_eq!(weekday(2024, 2, 29), 3, "a Thursday");
        assert_eq!(weekday(2000, 1, 1), 5, "a Saturday");
    }

    #[test]
    fn describes_and_builds_args() {
        let t = task();
        t.validate().unwrap();
        assert_eq!(t.describe(), "Weekly on Monday at 06:30");
        let a = t.crawl_args().join(" ");
        assert!(a.starts_with("crawl https://www.example.com/ --config C:/p/audit.toadconfig --output-folder C:/crawls --timestamped-output --task-name Weekly audit"), "{a}");
        assert!(a.contains("--export-tabs Internal:All --save-report Crawl Overview --export-format xlsx --consolidate"));
        assert!(a.contains("--compare-with-previous --email-on-complete=a@x.com"));
        assert!(ScheduledTask { url: String::new(), ..t.clone() }.validate().is_err());
        assert!(ScheduledTask { start: "next tuesday".into(), ..t }.validate().is_err());
    }

    #[test]
    fn os_definitions() {
        let t = task();
        let xml = task_xml(&t, Path::new("C:/Program Files/Toad/toad.exe")).unwrap();
        assert!(xml.contains("<StartBoundary>2026-10-19T06:30:00</StartBoundary>"));
        assert!(xml.contains("<DaysOfWeek><Monday /></DaysOfWeek>"));
        assert!(xml.contains("<Arguments>run-task t1</Arguments>"));
        let monthly = task_xml(&ScheduledTask { frequency: Frequency::Monthly, ..t.clone() }, Path::new("toad")).unwrap();
        assert!(monthly.contains("<Day>19</Day>"));
        assert_eq!(cron_line(&t, Path::new("/usr/bin/toad")).unwrap(), "30 6 * * 1 '/usr/bin/toad' run-task t1 # toad-task:t1");
        assert!(launchd_plist(&t, Path::new("/usr/bin/toad")).unwrap().contains("<key>Weekday</key><integer>1</integer>"));
    }
}
