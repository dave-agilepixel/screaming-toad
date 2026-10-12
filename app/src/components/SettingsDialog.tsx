import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { crawlsApi, type AppSettings } from "../api";

const PRESETS: [string, number | null][] = [
  ["Never", null],
  ["7 days", 7],
  ["30 days", 30],
];

/** File > Settings: storage folder and crawl retention. */
export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const [s, setS] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    crawlsApi.getSettings().then(setS).catch((e) => setError(String(e)));
  }, []);

  if (!s) return null;
  const preset = PRESETS.find(([, d]) => d === s.retention_days)?.[0] ?? "Custom";

  const pickFolder = async () => {
    const dir = await open({ directory: true, title: "Folder for saved crawls" });
    if (typeof dir === "string") setS({ ...s, storage_dir: dir });
  };

  const apply = async () => {
    try {
      await crawlsApi.setSettings(s);
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>Settings</h2>
        <fieldset className="config-group">
          <legend>Storage</legend>
          <p className="hint">
            Every crawl is saved automatically as its own database file (<code>.toad</code>), so crawls can be reopened, resumed and
            compared. Use a local SSD; network and cloud-synced folders are not supported.
          </p>
          <div className="field">
            <label className="stack">
              Directory
              <span className="button-row">
                <input className="grow" value={s.storage_dir ?? ""} placeholder="Default (the app's data folder)" readOnly />
                <button onClick={pickFolder}>Browse…</button>
                <button onClick={() => setS({ ...s, storage_dir: null })} disabled={!s.storage_dir}>
                  Default
                </button>
              </span>
            </label>
            <p className="hint">Changing the folder doesn't move existing crawls; File &gt; Crawls lists the crawls in the current folder.</p>
          </div>
        </fieldset>
        <fieldset className="config-group">
          <legend>Crawl Retention</legend>
          <div className="field">
            <label className="row">
              Delete crawls older than
              <span className="input-unit">
                <select
                  value={preset}
                  onChange={(e) => {
                    const p = PRESETS.find(([n]) => n === e.target.value);
                    setS({ ...s, retention_days: p ? p[1] : (s.retention_days ?? 14) });
                  }}
                >
                  {PRESETS.map(([n]) => (
                    <option key={n}>{n}</option>
                  ))}
                  <option>Custom</option>
                </select>
                {preset === "Custom" && (
                  <>
                    <input type="number" min={1} value={s.retention_days ?? 14} onChange={(e) => setS({ ...s, retention_days: Math.max(1, Number(e.target.value)) })} />
                    <span className="unit">days</span>
                  </>
                )}
              </span>
            </label>
            <p className="hint">Checked at startup and after every crawl. Locked crawls, the open crawl and crawls in progress are never deleted.</p>
          </div>
        </fieldset>
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <span className="spacer" />
          <button onClick={onClose}>Cancel</button>
          <button className="primary" onClick={apply}>
            OK
          </button>
        </div>
      </div>
    </div>
  );
}
