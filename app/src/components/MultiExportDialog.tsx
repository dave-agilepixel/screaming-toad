import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, exportApi, type ExportPreset, type MultiExport, type ReportDef, type TabDef } from "../api";

/**
 * Export > Multi Export: tab filters, reports and bulk exports in one go,
 * as CSVs or XLSX (optionally one workbook). Presets are shared with the
 * CLI (`toad crawl … --export-preset NAME`).
 */
export function MultiExportDialog({ onClose, onDone }: { onClose: () => void; onDone: (msg: string) => void }) {
  const [tabs, setTabs] = useState<TabDef[]>([]);
  const [reports, setReports] = useState<ReportDef[]>([]);
  const [spec, setSpec] = useState<MultiExport>({ items: [], format: "csv", consolidate: false, skip_empty: true });
  const [presets, setPresets] = useState<ExportPreset[]>([]);
  const [presetName, setPresetName] = useState("");
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.tabs().then(setTabs).catch(console.error);
    api.reports().then(setReports).catch(console.error);
    exportApi.presets().then(setPresets).catch(console.error);
  }, []);

  const chosen = useMemo(() => new Set(spec.items), [spec.items]);
  const toggle = (item: string, on: boolean) =>
    setSpec((s) => ({ ...s, items: on ? [...s.items.filter((i) => i !== item), item] : s.items.filter((i) => i !== item) }));
  const matches = (text: string) => !query.trim() || text.toLowerCase().includes(query.trim().toLowerCase());

  const run = async () => {
    const dir = await open({ directory: true, title: "Folder for the exports" });
    if (typeof dir !== "string") return;
    setBusy(true);
    setError(null);
    try {
      const files = await exportApi.multiExport(spec, dir);
      onDone(files.length ? `Exported ${files.length} file${files.length === 1 ? "" : "s"} to ${dir}` : "Nothing to export (every selection was empty).");
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const savePreset = async () => {
    const name = presetName.trim();
    if (!name) return;
    const next = [...presets.filter((p) => p.name !== name), { name, export: spec }];
    try {
      await exportApi.savePresets(next);
      setPresets(next);
    } catch (e) {
      setError(String(e));
    }
  };

  const deletePreset = async (name: string) => {
    const next = presets.filter((p) => p.name !== name);
    await exportApi.savePresets(next).catch((e) => setError(String(e)));
    setPresets(next);
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide-modal" onClick={(e) => e.stopPropagation()}>
        <h2>Multi Export</h2>
        <div className="button-row">
          <input className="grow" placeholder="Search tabs, filters and reports…" value={query} onChange={(e) => setQuery(e.target.value)} />
          <select
            value=""
            onChange={(e) => {
              const p = presets.find((x) => x.name === e.target.value);
              if (p) {
                setSpec(p.export);
                setPresetName(p.name);
              }
            }}
          >
            <option value="">Load preset…</option>
            {presets.map((p) => (
              <option key={p.name}>{p.name}</option>
            ))}
          </select>
        </div>
        <div className="multi-export-lists">
          <div>
            <h4>Tabs</h4>
            {tabs.map((t) => {
              const filters = t.filters.filter((f) => matches(`${t.name} ${f.name}`));
              if (!filters.length) return null;
              return (
                <details key={t.name} open={!!query.trim()}>
                  <summary>
                    {t.name} <span className="muted">{t.filters.filter((f) => chosen.has(`${t.name}:${f.name}`)).length || ""}</span>
                  </summary>
                  {filters.map((f) => {
                    const item = `${t.name}:${f.name}`;
                    return (
                      <label key={item} className="check wide">
                        <input type="checkbox" checked={chosen.has(item)} onChange={(e) => toggle(item, e.target.checked)} /> {f.name}
                      </label>
                    );
                  })}
                </details>
              );
            })}
          </div>
          <div>
            {(["Report", "BulkExport"] as const).map((kind) => (
              <div key={kind}>
                <h4>{kind === "Report" ? "Reports" : "Bulk Export"}</h4>
                {reports
                  .filter((r) => r.kind === kind && matches(r.name))
                  .map((r) => {
                    const item = `report:${r.name}`;
                    return (
                      <label key={item} className="check wide" title={r.description}>
                        <input type="checkbox" checked={chosen.has(item)} onChange={(e) => toggle(item, e.target.checked)} /> {r.name}
                      </label>
                    );
                  })}
              </div>
            ))}
          </div>
        </div>
        <div className="button-row">
          <label className="inline">
            Format
            <select value={spec.format} onChange={(e) => setSpec({ ...spec, format: e.target.value as MultiExport["format"] })}>
              <option value="csv">CSV</option>
              <option value="xlsx">Excel (XLSX)</option>
            </select>
          </label>
          <label className="check inline">
            <input type="checkbox" checked={spec.consolidate} disabled={spec.format !== "xlsx"} onChange={(e) => setSpec({ ...spec, consolidate: e.target.checked })} /> One
            workbook
          </label>
          <label className="check inline">
            <input type="checkbox" checked={spec.skip_empty} onChange={(e) => setSpec({ ...spec, skip_empty: e.target.checked })} /> Skip empty
          </label>
          <span className="spacer" />
          <span className="muted">{spec.items.length} selected</span>
        </div>
        <div className="button-row">
          <input placeholder="Preset name" value={presetName} onChange={(e) => setPresetName(e.target.value)} />
          <button onClick={savePreset} disabled={!presetName.trim() || spec.items.length === 0}>
            Save Preset
          </button>
          <button onClick={() => deletePreset(presetName.trim())} disabled={!presets.some((p) => p.name === presetName.trim())}>
            Delete Preset
          </button>
          <span className="hint">Use a preset from the CLI with --export-preset "{presetName.trim() || "name"}".</span>
        </div>
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <button onClick={() => setSpec({ ...spec, items: [] })} disabled={spec.items.length === 0}>
            Clear
          </button>
          <span className="spacer" />
          <button onClick={onClose}>Cancel</button>
          <button className="primary" onClick={run} disabled={busy || spec.items.length === 0}>
            Export…
          </button>
        </div>
      </div>
    </div>
  );
}
