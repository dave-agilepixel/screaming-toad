import { useCallback, useEffect, useMemo, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { confirmAction, crawlsApi, type CrawlEntry } from "../api";

interface Props {
  onClose: () => void;
  onOpen: (path: string) => void;
  onResume: (path: string) => void;
  /** Two crawls picked with "Select To Compare": current, then previous. */
  onCompare: (current: string, previous: string) => void;
}

function size(bytes: number): string {
  if (bytes >= 1 << 30) return `${(bytes / (1 << 30)).toFixed(1)} GB`;
  if (bytes >= 1 << 20) return `${(bytes / (1 << 20)).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

function when(secs: number): string {
  return secs ? new Date(secs * 1000).toLocaleString() : "";
}

const NO_PROJECT = "";

/** File > Crawls: every saved crawl, grouped by project. */
export function CrawlsDialog({ onClose, onOpen, onResume, onCompare }: Props) {
  const [crawls, setCrawls] = useState<CrawlEntry[]>([]);
  const [openPath, setOpenPath] = useState<string | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [search, setSearch] = useState("");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [renaming, setRenaming] = useState<string | null>(null);
  const [nameDraft, setNameDraft] = useState("");
  const [projectDraft, setProjectDraft] = useState("");
  const [message, setMessage] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      const [list, current] = await Promise.all([crawlsApi.list(), crawlsApi.current()]);
      setCrawls(list);
      setOpenPath(current);
      setSelected((sel) => sel.filter((p) => list.some((c) => c.path === p)));
    } catch (e) {
      setMessage(String(e));
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const run = async (label: string, action: () => Promise<unknown>) => {
    try {
      await action();
      setMessage(null);
      await reload();
    } catch (e) {
      setMessage(`${label}: ${e}`);
    }
  };

  const q = search.trim().toLowerCase();
  const visible = crawls.filter((c) => !q || [c.name, c.project, c.start_url ?? "", c.mode].some((v) => v.toLowerCase().includes(q)));
  const groups = useMemo(() => {
    const m = new Map<string, CrawlEntry[]>();
    for (const c of visible) {
      if (!m.has(c.project)) m.set(c.project, []);
      m.get(c.project)!.push(c);
    }
    // Named projects first (alphabetical), then crawls with no project.
    return [...m.entries()].sort(([a], [b]) => (a === NO_PROJECT ? 1 : b === NO_PROJECT ? -1 : a.localeCompare(b)));
  }, [visible]);
  const projects = [...new Set(crawls.map((c) => c.project).filter(Boolean))].sort();

  const one = selected.length === 1 ? crawls.find((c) => c.path === selected[0]) : undefined;
  const chosen = crawls.filter((c) => selected.includes(c.path));
  const resumable = one && one.status !== "Complete";

  const toggle = (path: string, additive: boolean) =>
    setSelected((sel) => (additive ? (sel.includes(path) ? sel.filter((p) => p !== path) : [...sel, path]) : [path]));

  const startRename = (c: CrawlEntry) => {
    setRenaming(c.path);
    setNameDraft(c.name);
  };
  const commitRename = async () => {
    if (renaming && nameDraft.trim()) await run("Rename", () => crawlsApi.rename(renaming, nameDraft));
    setRenaming(null);
  };

  const remove = async () => {
    const names = chosen.map((c) => `• ${c.name}`).join("\n");
    if (!(await confirmAction(`Permanently delete ${chosen.length === 1 ? "this crawl" : `these ${chosen.length} crawls`}?\n\n${names}`))) return;
    for (const c of chosen) await run(`Delete ${c.name}`, () => crawlsApi.remove(c.path));
  };

  const exportCrawl = async () => {
    if (!one) return;
    const dest = await save({ defaultPath: `${one.name.replace(/[^\w.-]+/g, "_")}.toad`, filters: [{ name: "Screaming Toad crawl", extensions: ["toad"] }] });
    if (dest) await run("Export", () => crawlsApi.exportTo(one.path, dest));
  };

  const compare = () => {
    // The later crawl is Current, as in Screaming Frog.
    const [a, b] = [...chosen].sort((x, y) => y.created - x.created);
    onCompare(a.path, b.path);
  };

  const totalSize = crawls.reduce((n, c) => n + c.size_bytes, 0);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal crawls-modal" onClick={(e) => e.stopPropagation()}>
        <div className="config-head">
          <h2>Crawls</h2>
          <button onClick={() => setCollapsed(new Set())}>Expand all</button>
          <button onClick={() => setCollapsed(new Set(groups.map(([p]) => p)))}>Collapse all</button>
          <input className="config-search" placeholder="Search…" value={search} onChange={(e) => setSearch(e.target.value)} spellCheck={false} />
        </div>

        <div className="crawls-table-wrap">
          <table className="crawls-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>URL</th>
                <th>Mode</th>
                <th className="num">URLs</th>
                <th className="num">% Completed</th>
                <th>Modified</th>
                <th className="num">Size</th>
                <th>Version</th>
                <th title="Locked crawls can't be deleted and are kept by Crawl Retention">🔒</th>
              </tr>
            </thead>
            {groups.map(([project, rows]) => {
              const isCollapsed = collapsed.has(project);
              return (
                <tbody key={project || "(none)"}>
                  {project !== NO_PROJECT && (
                    <tr className="group-row">
                      <td colSpan={9}>
                        <button
                          className="tree-toggle"
                          onClick={() =>
                            setCollapsed((s) => {
                              const n = new Set(s);
                              n.has(project) ? n.delete(project) : n.add(project);
                              return n;
                            })
                          }
                        >
                          {isCollapsed ? "▸" : "▾"} 📁 {project} <span className="muted">({rows.length})</span>
                        </button>
                      </td>
                    </tr>
                  )}
                  {!isCollapsed &&
                    rows.map((c) => (
                      <tr
                        key={c.path}
                        className={`${selected.includes(c.path) ? "selected" : ""}${c.path === openPath ? " open-crawl" : ""}`}
                        onClick={(e) => toggle(c.path, e.ctrlKey || e.metaKey || e.shiftKey)}
                        onDoubleClick={() => onOpen(c.path)}
                        title={c.path}
                      >
                        <td className={project ? "indent" : ""}>
                          {renaming === c.path ? (
                            <input
                              autoFocus
                              value={nameDraft}
                              onChange={(e) => setNameDraft(e.target.value)}
                              onBlur={commitRename}
                              onKeyDown={(e) => {
                                if (e.key === "Enter") commitRename();
                                if (e.key === "Escape") setRenaming(null);
                              }}
                              onClick={(e) => e.stopPropagation()}
                            />
                          ) : (
                            <>
                              {c.name}
                              {c.path === openPath && <span className="tag">open</span>}
                              {c.status === "Crawling" && <span className="tag warn" title="Interrupted or still running; it can be resumed">interrupted</span>}
                              {c.status === "Stopped" && <span className="tag" title="Stopped before the queue was empty; it can be resumed">stopped</span>}
                            </>
                          )}
                        </td>
                        <td className="url-cell">{c.start_url}</td>
                        <td>{c.mode === "list" ? "List" : c.mode === "spider" ? "Spider" : c.mode}</td>
                        <td className="num">{c.urls.toLocaleString()}</td>
                        <td className="num">{c.pct_complete.toFixed(1)}%</td>
                        <td>{when(c.modified)}</td>
                        <td className="num">{size(c.size_bytes)}</td>
                        <td>{c.version}</td>
                        <td>{c.locked ? "🔒" : ""}</td>
                      </tr>
                    ))}
                </tbody>
              );
            })}
          </table>
          {crawls.length === 0 && <p className="empty">No saved crawls yet. Crawls are saved automatically as they run.</p>}
        </div>

        <div className="crawls-actions">
          <button className="primary" disabled={!one} onClick={() => one && onOpen(one.path)}>
            Open
          </button>
          <button disabled={!resumable || one?.path === openPath} onClick={() => one && onResume(one.path)} title="Continue a stopped or interrupted crawl">
            Resume
          </button>
          <button disabled={selected.length !== 2} onClick={compare} title="Compare two crawls (the later one is Current)">
            Select To Compare
          </button>
          <span className="sep" />
          <button disabled={!one} onClick={() => one && startRename(one)}>
            Rename
          </button>
          <button disabled={!one} onClick={() => one && run("Duplicate", () => crawlsApi.duplicate(one.path))}>
            Duplicate
          </button>
          <button disabled={!one} onClick={exportCrawl}>
            Export
          </button>
          <button
            disabled={chosen.length === 0}
            onClick={() => {
              const lock = !chosen.every((c) => c.locked);
              chosen.forEach((c) => run(lock ? "Lock" : "Unlock", () => crawlsApi.setLocked(c.path, lock)));
            }}
          >
            {chosen.length > 0 && chosen.every((c) => c.locked) ? "Unlock" : "Lock"}
          </button>
          <button className="danger" disabled={chosen.length === 0 || chosen.some((c) => c.locked)} onClick={remove} title={chosen.some((c) => c.locked) ? "Unlock first" : ""}>
            Delete
          </button>
          <span className="sep" />
          <input
            list="toad-projects"
            className="project-input"
            placeholder="Project name"
            value={projectDraft}
            onChange={(e) => setProjectDraft(e.target.value)}
            disabled={chosen.length === 0}
          />
          <datalist id="toad-projects">
            {projects.map((p) => (
              <option key={p} value={p} />
            ))}
          </datalist>
          <button disabled={chosen.length === 0} onClick={() => chosen.forEach((c) => run("Project", () => crawlsApi.setProject(c.path, projectDraft)))}>
            {projectDraft.trim() ? "Move to Project" : "Remove from Project"}
          </button>
        </div>

        {message && <p className="error">{message}</p>}
        <div className="modal-actions">
          <span className="muted">
            {crawls.length} crawl{crawls.length === 1 ? "" : "s"} · {size(totalSize)} · Ctrl+click to select several · double-click to open
          </span>
          <span className="spacer" />
          <button onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}
