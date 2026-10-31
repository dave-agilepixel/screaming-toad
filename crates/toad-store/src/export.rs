//! Export > Multi Export: several tab filters, reports and bulk exports at
//! once, as CSV files or XLSX workbooks (optionally consolidated into one
//! workbook), skipping empty ones if asked. Presets are shared by the
//! desktop app and the CLI.

use crate::{RowQuery, Store, Table};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    #[default]
    Csv,
    Xlsx,
}

impl ExportFormat {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "csv" => Ok(Self::Csv),
            "xlsx" | "excel" => Ok(Self::Xlsx),
            other => anyhow::bail!("unknown export format '{other}' (csv or xlsx)"),
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
        }
    }
}

/// One thing to export: "Tab:Filter", or "report:Name" for a report or bulk
/// export (Reports / Bulk Export menus).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportItem(pub String);

impl ExportItem {
    pub fn tab(tab: &str, filter: &str) -> Self {
        Self(format!("{tab}:{filter}"))
    }

    pub fn report(name: &str) -> Self {
        Self(format!("report:{name}"))
    }

    /// The file / sheet name.
    pub fn title(&self) -> String {
        match self.0.strip_prefix("report:") {
            Some(r) => r.trim().to_string(),
            None => {
                let (tab, filter) = self.0.split_once(':').unwrap_or((&self.0, "All"));
                format!("{} {}", tab.trim(), filter.trim())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MultiExport {
    pub items: Vec<ExportItem>,
    pub format: ExportFormat,
    /// XLSX: one workbook with a sheet per item.
    pub consolidate: bool,
    pub skip_empty: bool,
}

/// What a Multi Export wrote, and the items it skipped (title, reason).
#[derive(Debug, Clone, Default)]
pub struct ExportOutcome {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<(String, String)>,
}

/// A saved Multi Export selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportPreset {
    pub name: String,
    pub export: MultiExport,
}

/// Presets live in ~/.toad/export_presets.json, shared with the CLI.
pub fn presets_path() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    home.join(".toad").join("export_presets.json")
}

pub fn load_presets() -> Vec<ExportPreset> {
    std::fs::read_to_string(presets_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_presets(presets: &[ExportPreset]) -> anyhow::Result<()> {
    let p = presets_path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(p, serde_json::to_string_pretty(presets)?)?;
    Ok(())
}

/// A file-system-safe name.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

impl MultiExport {
    /// The files a run would write (to check before overwriting).
    pub fn file_names(&self) -> Vec<String> {
        if self.items.is_empty() {
            return vec![];
        }
        if self.format == ExportFormat::Xlsx && self.consolidate {
            return vec!["export.xlsx".into()];
        }
        self.items.iter().map(|i| format!("{}.{}", slug(&i.title()), self.format.ext())).collect()
    }
}

impl Store {
    /// The table one export item stands for.
    pub fn export_table(&self, item: &ExportItem) -> anyhow::Result<Table> {
        match item.0.strip_prefix("report:") {
            Some(name) => self.report(name.trim()),
            None => {
                let (tab, filter) = item.0.split_once(':').unwrap_or((&item.0, "All"));
                self.rows(&RowQuery { tab: tab.trim().into(), filter: filter.trim().into(), ..Default::default() })
            }
        }
    }

    /// Run a Multi Export into `dir`. An item that can't be built (an
    /// unknown report, tab or filter) is skipped, so one bad name doesn't
    /// lose the rest.
    pub fn multi_export(&self, spec: &MultiExport, dir: &Path) -> anyhow::Result<ExportOutcome> {
        std::fs::create_dir_all(dir)?;
        let mut tables = Vec::new();
        let mut skipped = Vec::new();
        for item in &spec.items {
            let t = match self.export_table(item) {
                Ok(t) => t,
                Err(e) => {
                    skipped.push((item.title(), format!("{e:#}")));
                    continue;
                }
            };
            if spec.skip_empty && t.rows.is_empty() {
                continue;
            }
            tables.push((item.title(), t));
        }
        let mut written = Vec::new();
        match (spec.format, spec.consolidate) {
            (ExportFormat::Xlsx, true) => {
                let path = dir.join("export.xlsx");
                write_xlsx(&tables.iter().map(|(n, t)| (n.as_str(), t)).collect::<Vec<_>>(), &path)?;
                written.push(path);
            }
            (format, _) => {
                for (name, t) in &tables {
                    let path = dir.join(format!("{}.{}", slug(name), format.ext()));
                    match format {
                        ExportFormat::Csv => crate::write_csv(t, &path)?,
                        ExportFormat::Xlsx => write_xlsx(&[(name.as_str(), t)], &path)?,
                    }
                    written.push(path);
                }
            }
        }
        Ok(ExportOutcome { written, skipped })
    }
}

/// Write one table as CSV, or XLSX when the path ends in .xlsx.
pub fn write_table(table: &Table, path: &Path, sheet: &str) -> anyhow::Result<()> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xlsx")) {
        write_xlsx(&[(sheet, table)], path)
    } else {
        crate::write_csv(table, path)
    }
}

/// Excel sheet names: at most 31 characters, none of []:*?/\, unique.
fn sheet_name(name: &str, used: &mut Vec<String>) -> String {
    let clean: String = name.chars().map(|c| if "[]:*?/\\".contains(c) { '-' } else { c }).collect();
    let mut base: String = clean.chars().take(31).collect();
    if base.trim().is_empty() {
        base = "Sheet".into();
    }
    let mut candidate = base.clone();
    let mut n = 2;
    while used.iter().any(|u| u.eq_ignore_ascii_case(&candidate)) {
        let suffix = format!(" ({n})");
        candidate = format!("{}{suffix}", base.chars().take(31 - suffix.len()).collect::<String>());
        n += 1;
    }
    used.push(candidate.clone());
    candidate
}

/// Write tables to an XLSX workbook, one sheet each. Numbers are written as
/// numbers (except ones with leading zeros), a bold frozen header row, and
/// columns sized to their headers.
pub fn write_xlsx(tables: &[(&str, &Table)], path: &Path) -> anyhow::Result<()> {
    use rust_xlsxwriter::{Format, Workbook};
    let mut wb = Workbook::new();
    let bold = Format::new().set_bold();
    let mut used = Vec::new();
    if tables.is_empty() {
        wb.add_worksheet().set_name("Empty")?;
    }
    for (name, t) in tables {
        let ws = wb.add_worksheet();
        ws.set_name(sheet_name(name, &mut used))?;
        for (c, h) in t.columns.iter().enumerate() {
            ws.write_string_with_format(0, c as u16, h, &bold)?;
            ws.set_column_width(c as u16, (h.chars().count().max(10) + 2).min(60) as f64)?;
        }
        ws.set_freeze_panes(1, 0)?;
        for (r, row) in t.rows.iter().enumerate() {
            let r = (r + 1) as u32;
            if r > 1_048_575 {
                break;
            }
            for (c, v) in row.iter().enumerate() {
                let numeric = !v.is_empty() && !(v.len() > 1 && v.starts_with('0') && !v.starts_with("0.")) && v.parse::<f64>().is_ok_and(f64::is_finite);
                if numeric {
                    ws.write_number(r, c as u16, v.parse::<f64>().unwrap_or(0.0))?;
                } else if !v.is_empty() {
                    // Excel caps a cell at 32,767 characters.
                    let s: String = v.chars().take(32_767).collect();
                    ws.write_string(r, c as u16, &s)?;
                }
            }
        }
    }
    wb.save(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_names() {
        let mut used = Vec::new();
        assert_eq!(sheet_name("Response Codes: Client Error (4XX)", &mut used), "Response Codes- Client Error (4");
        assert_eq!(sheet_name("Response Codes: Client Error (4XX)", &mut used), "Response Codes- Client Erro (2)");
        assert_eq!(ExportItem::tab("Internal", "All").title(), "Internal All");
        assert_eq!(ExportItem::report("Redirect Chains").title(), "Redirect Chains");
    }

    #[test]
    fn xlsx_round_trip_shape() {
        let t = Table { columns: vec!["Address".into(), "Status Code".into(), "Zip".into()], rows: vec![vec!["https://x.com/".into(), "200".into(), "01234".into()]], total: 1 };
        let path = std::env::temp_dir().join(format!("toad-xlsx-{}.xlsx", std::process::id()));
        write_xlsx(&[("Internal: All", &t)], &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"PK"), "an XLSX is a zip");
    }
}
