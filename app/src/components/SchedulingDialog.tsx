import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { exportApi, scheduleApi, type ExportPreset, type RunRecord, type ScheduledTask } from "../api";

const DAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

function describe(t: ScheduledTask): string {
  const [date, time] = t.start.split(" ");
  switch (t.frequency) {
    case "once":
      return `Once, ${date} at ${time}`;
    case "daily":
      return `Daily at ${time}`;
    case "weekly": {
      const d = t.weekday ?? (new Date(`${date}T12:00:00`).getDay() + 6) % 7;
      return `Weekly on ${DAYS[d]} at ${time}`;
    }
    case "monthly":
      return `Monthly on day ${Number(date?.split("-")[2])} at ${time}`;
  }
}

function blank(): ScheduledTask {
  const d = new Date(Date.now() + 3600_000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return {
    id: "",
    name: "",
    project: "",
    enabled: true,
    url: "",
    list_file: "",
    config_profile: "",
    frequency: "weekly",
    start: `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:00`,
    weekday: null,
    output_folder: "",
    exports: { items: [], format: "csv", consolidate: false, skip_empty: true },
    create_sitemap: false,
    create_images_sitemap: false,
    use_apis: false,
    email: false,
    email_to: [],
    auto_compare: true,
    overwrite: false,
  };
}

/**
 * File > Scheduling: crawls run by the OS scheduler (Task Scheduler on
 * Windows), headless, with exports, sitemaps, Auto Compare and an email.
 */
export function SchedulingDialog({ onClose }: { onClose: () => void }) {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [history, setHistory] = useState<RunRecord[]>([]);
  const [presets, setPresets] = useState<ExportPreset[]>([]);
  const [edit, setEdit] = useState<ScheduledTask | null>(null);
  const [view, setView] = useState<"tasks" | "history">("tasks");
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const reload = () => {
    scheduleApi.tasks().then(setTasks).catch((e) => setError(String(e)));
    scheduleApi.history().then(setHistory).catch((e) => setError(String(e)));
  };
  useEffect(() => {
    reload();
    exportApi.presets().then(setPresets).catch(console.error);
  }, []);

  const last = (id: string) => history.find((h) => h.task_id === id);
  const save = async () => {
    if (!edit) return;
    setError(null);
    try {
      await scheduleApi.save(edit);
      setEdit(null);
      reload();
    } catch (e) {
      setError(String(e));
    }
  };
  const pick = async (key: "config_profile" | "output_folder" | "list_file") => {
    if (!edit) return;
    const path =
      key === "output_folder"
        ? await open({ directory: true, title: "Output folder" })
        : await open({ filters: key === "config_profile" ? [{ name: "Configuration profile", extensions: ["toadconfig"] }] : [{ name: "URL list", extensions: ["txt", "csv"] }] });
    if (typeof path === "string") setEdit({ ...edit, [key]: path });
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide-modal" onClick={(e) => e.stopPropagation()}>
        <h2>Scheduling</h2>
        {!edit && (
          <>
            <div className="detail-tabs subtabs">
              <button className={view === "tasks" ? "active" : ""} onClick={() => setView("tasks")}>
                Tasks
              </button>
              <button className={view === "history" ? "active" : ""} onClick={() => setView("history")}>
                History
              </button>
            </div>
            {view === "tasks" ? (
              <table className="mapping-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Schedule</th>
                    <th>Last Run</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {tasks.map((t) => {
                    const l = last(t.id);
                    return (
                      <tr key={t.id}>
                        <td>
                          {t.name}
                          {!t.enabled && <span className="muted"> (off)</span>}
                          <div className="muted">{t.url || t.list_file}</div>
                        </td>
                        <td>{describe(t)}</td>
                        <td className={l && !l.ok ? "error" : ""} title={l?.message}>
                          {l ? `${new Date(l.started * 1000).toLocaleString()} · ${l.ok ? l.message : "Failed"}` : "Never"}
                        </td>
                        <td className="button-row">
                          <button onClick={() => setEdit({ ...t })}>Edit</button>
                          <button
                            onClick={() =>
                              scheduleApi
                                .runNow(t.id)
                                .then(() => setNote(`${t.name} started in the background; see History.`))
                                .catch((e) => setError(String(e)))
                            }
                          >
                            Run Now
                          </button>
                          <button onClick={() => scheduleApi.remove(t.id).then(reload).catch((e) => setError(String(e)))}>Delete</button>
                        </td>
                      </tr>
                    );
                  })}
                  {tasks.length === 0 && (
                    <tr>
                      <td colSpan={4} className="muted">
                        No scheduled crawls yet.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            ) : (
              <table className="mapping-table">
                <thead>
                  <tr>
                    <th>Started</th>
                    <th>Task</th>
                    <th>Result</th>
                    <th>Crawl</th>
                  </tr>
                </thead>
                <tbody>
                  {history.map((h, i) => (
                    <tr key={i}>
                      <td>{new Date(h.started * 1000).toLocaleString()}</td>
                      <td>{h.task_name}</td>
                      <td className={h.ok ? "" : "error"}>{h.message}</td>
                      <td title={h.crawl}>{h.crawl.split(/[\\/]/).slice(-2).join("/")}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
            <p className="hint">
              Tasks run through the operating system's scheduler while you're logged in, even when Screaming Toad is closed. Each run gets its own dated folder
              under the output folder.
            </p>
          </>
        )}

        {edit && (
          <div className="sched-form">
            <div className="button-row">
              <input className="grow" placeholder="Task name" value={edit.name} onChange={(e) => setEdit({ ...edit, name: e.target.value })} />
              <input placeholder="Project" value={edit.project} onChange={(e) => setEdit({ ...edit, project: e.target.value })} />
              <label className="check inline">
                <input type="checkbox" checked={edit.enabled} onChange={(e) => setEdit({ ...edit, enabled: e.target.checked })} /> Enabled
              </label>
            </div>
            <div className="button-row">
              <input className="grow" placeholder="Start URL (Spider mode)" value={edit.url} onChange={(e) => setEdit({ ...edit, url: e.target.value })} spellCheck={false} />
              <span className="muted">or</span>
              <input className="grow" placeholder="URL list file (List mode)" value={edit.list_file} readOnly />
              <button onClick={() => pick("list_file")}>Browse…</button>
            </div>
            <div className="button-row">
              <input className="grow" placeholder="Configuration profile (blank: defaults)" value={edit.config_profile} readOnly />
              <button onClick={() => pick("config_profile")}>Browse…</button>
              <button onClick={() => setEdit({ ...edit, config_profile: "" })} disabled={!edit.config_profile}>
                Clear
              </button>
            </div>
            <div className="button-row">
              <select value={edit.frequency} onChange={(e) => setEdit({ ...edit, frequency: e.target.value as ScheduledTask["frequency"] })}>
                <option value="once">Once</option>
                <option value="daily">Daily</option>
                <option value="weekly">Weekly</option>
                <option value="monthly">Monthly</option>
              </select>
              <input
                type="datetime-local"
                value={edit.start.replace(" ", "T")}
                onChange={(e) => setEdit({ ...edit, start: e.target.value.replace("T", " ").slice(0, 16), weekday: null })}
              />
              <span className="muted">{describe(edit)}</span>
            </div>
            <div className="button-row">
              <input className="grow" placeholder="Output folder" value={edit.output_folder} readOnly />
              <button onClick={() => pick("output_folder")}>Browse…</button>
            </div>
            <fieldset className="config-group">
              <legend>Export</legend>
              <div className="button-row">
                <select
                  value=""
                  onChange={(e) => {
                    const p = presets.find((x) => x.name === e.target.value);
                    if (p) setEdit({ ...edit, exports: p.export });
                  }}
                >
                  <option value="">Use a Multi Export preset…</option>
                  {presets.map((p) => (
                    <option key={p.name}>{p.name}</option>
                  ))}
                </select>
                <span className="muted">
                  {edit.exports.items.length ? `${edit.exports.items.length} exports as ${edit.exports.format.toUpperCase()}` : "No exports"}
                </span>
                {edit.exports.items.length > 0 && <button onClick={() => setEdit({ ...edit, exports: { ...edit.exports, items: [] } })}>Clear</button>}
              </div>
              <label className="check wide">
                <input type="checkbox" checked={edit.create_sitemap} onChange={(e) => setEdit({ ...edit, create_sitemap: e.target.checked })} /> Create XML sitemap
              </label>
              <label className="check wide">
                <input type="checkbox" checked={edit.create_images_sitemap} onChange={(e) => setEdit({ ...edit, create_images_sitemap: e.target.checked })} /> Create images
                sitemap
              </label>
            </fieldset>
            <fieldset className="config-group">
              <legend>After the crawl</legend>
              <label className="check wide">
                <input type="checkbox" checked={edit.use_apis} onChange={(e) => setEdit({ ...edit, use_apis: e.target.checked })} /> Request API data (as set up in the
                profile)
              </label>
              <label className="check wide">
                <input type="checkbox" checked={edit.auto_compare} onChange={(e) => setEdit({ ...edit, auto_compare: e.target.checked })} /> Auto Compare with this task's
                previous crawl
              </label>
              <label className="check wide">
                <input type="checkbox" checked={edit.email} onChange={(e) => setEdit({ ...edit, email: e.target.checked })} /> Email a summary (File &gt; Settings &gt;
                Notifications)
              </label>
              {edit.email && (
                <input
                  placeholder="To (blank: the default recipients)"
                  value={edit.email_to.join(", ")}
                  onChange={(e) => setEdit({ ...edit, email_to: e.target.value.split(",").map((x) => x.trim()).filter(Boolean) })}
                />
              )}
            </fieldset>
          </div>
        )}

        {note && <p className="hint">{note}</p>}
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          {edit ? (
            <>
              <button onClick={() => setEdit(null)}>Back</button>
              <span className="spacer" />
              <button className="primary" onClick={save}>
                Save Task
              </button>
            </>
          ) : (
            <>
              <button onClick={() => setEdit(blank())}>Add Task</button>
              <span className="spacer" />
              <button onClick={onClose}>Close</button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
