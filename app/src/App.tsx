import { useCallback, useEffect, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { api, events, type Progress, type ReportDef, type TabDef } from "./api";
import { DataGrid } from "./components/DataGrid";
import { SidePanel } from "./components/SidePanel";
import { DetailPanel } from "./components/DetailPanel";
import { ConfigDialog } from "./components/ConfigDialog";
import { SitemapDialog } from "./components/SitemapDialog";

type Status = "idle" | "crawling" | "paused" | "analysing" | "done";

export default function App() {
  const [url, setUrl] = useState("");
  const [status, setStatus] = useState<Status>("idle");
  const [progress, setProgress] = useState<Progress | null>(null);
  const [tabs, setTabs] = useState<TabDef[]>([]);
  const [tab, setTab] = useState("Internal");
  const [filters, setFilters] = useState<Record<string, string>>({});
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<{ column: string; desc: boolean } | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [showConfig, setShowConfig] = useState(false);
  const [showSitemap, setShowSitemap] = useState(false);
  const [reports, setReports] = useState<ReportDef[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const lastRefresh = useRef(0);

  const filter = filters[tab] ?? "All";
  const tabDef = tabs.find((t) => t.name === tab);

  const refresh = useCallback(() => setRefreshKey((k) => k + 1), []);

  useEffect(() => {
    api.tabs().then(setTabs).catch(console.error);
    const unlisten = [
      events.onProgress((p) => {
        setProgress(p);
        // Re-query the grid and panels at most once a second while crawling.
        if (Date.now() - lastRefresh.current > 1000) {
          lastRefresh.current = Date.now();
          refresh();
        }
      }),
      events.onAnalysing(() => setStatus("analysing")),
      events.onFinished(() => {
        setStatus("done");
        refresh();
      }),
    ];
    return () => {
      unlisten.forEach((p) => p.then((f) => f()));
    };
  }, [refresh]);

  const start = async () => {
    setMessage(null);
    setSelected(null);
    try {
      await api.startCrawl(url);
      setProgress(null);
      setStatus("crawling");
      setTabs(await api.tabs());
      refresh();
    } catch (e) {
      setMessage(String(e));
    }
  };

  const pauseResume = async () => {
    if (status === "crawling") {
      await api.pause();
      setStatus("paused");
    } else if (status === "paused") {
      await api.resume();
      setStatus("crawling");
    }
  };

  const stop = async () => {
    await api.stop();
  };

  const openCrawl = async () => {
    const path = await open({ filters: [{ name: "Screaming Toad crawl", extensions: ["toad"] }] });
    if (typeof path !== "string") return;
    await api.openCrawl(path);
    setTabs(await api.tabs());
    setStatus("done");
    setSelected(null);
    refresh();
  };

  const saveCrawl = async () => {
    const path = await save({ filters: [{ name: "Screaming Toad crawl", extensions: ["toad"] }] });
    if (!path) return;
    await api.saveCrawlAs(path);
    setMessage(`Saved crawl to ${path}`);
  };

  const exportCsv = async () => {
    const path = await save({
      defaultPath: `${tab}-${filter}.csv`.replace(/[^\w.-]+/g, "_").toLowerCase(),
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    const n = await api.exportCsv(tab, filter, search || undefined, path);
    setMessage(`Exported ${n.toLocaleString()} rows to ${path}`);
  };

  useEffect(() => {
    if (status === "done") api.reports().then(setReports).catch(console.error);
  }, [status]);

  const runReport = async (name: string) => {
    if (!name) return;
    const path = await save({
      defaultPath: `${name}.csv`.replace(/[^\w.-]+/g, "_").toLowerCase(),
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    try {
      const n = await api.exportReport(name, path);
      setMessage(`${name}: ${n.toLocaleString()} rows → ${path}`);
    } catch (e) {
      setMessage(String(e));
    }
  };

  const pick = (t: string, f: string) => {
    setTab(t);
    setFilters((prev) => ({ ...prev, [t]: f }));
    setSort(null);
  };

  const onSort = (column: string) =>
    setSort((prev) => (prev?.column === column ? (prev.desc ? null : { column, desc: true }) : { column, desc: false }));

  const crawling = status === "crawling" || status === "paused";
  const done = progress?.crawled ?? 0;
  const pct = progress && done + progress.queued > 0 ? (done / (done + progress.queued)) * 100 : 0;

  return (
    <div className="app">
      <header className="toolbar">
        <div className="brand">🐸 Screaming Toad</div>
        <input
          className="url"
          placeholder="Enter URL to spider"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && !crawling && start()}
          spellCheck={false}
        />
        {!crawling ? (
          <button className="primary" onClick={start} disabled={!url.trim()}>
            Start
          </button>
        ) : (
          <>
            <button onClick={pauseResume}>{status === "paused" ? "Resume" : "Pause"}</button>
            <button onClick={stop}>Stop</button>
          </>
        )}
        <button onClick={() => setShowConfig(true)} disabled={crawling}>
          Configuration
        </button>
        <button onClick={openCrawl} disabled={crawling}>
          Open
        </button>
        <button onClick={saveCrawl} disabled={status !== "done"}>
          Save As
        </button>
        <button onClick={() => setShowSitemap(true)} disabled={status !== "done"}>
          XML Sitemap
        </button>
        <select
          className="reports"
          value=""
          disabled={status !== "done" || reports.length === 0}
          onChange={(e) => runReport(e.target.value)}
          title="Reports and bulk exports (saved as CSV)"
        >
          <option value="">Reports…</option>
          <optgroup label="Reports">
            {reports.filter((r) => r.kind === "Report").map((r) => (
              <option key={r.name} value={r.name} title={r.description}>
                {r.name}
              </option>
            ))}
          </optgroup>
          <optgroup label="Bulk Export">
            {reports.filter((r) => r.kind === "BulkExport").map((r) => (
              <option key={r.name} value={r.name} title={r.description}>
                {r.name}
              </option>
            ))}
          </optgroup>
        </select>
        <div className="progress" title={progress ? `${done} crawled, ${progress.queued} queued` : ""}>
          <div className="bar" style={{ width: `${status === "done" ? 100 : pct}%` }} />
          <span>
            {status === "idle" && "Ready"}
            {status === "crawling" && `Crawling: ${done.toLocaleString()} URLs`}
            {status === "paused" && `Paused at ${done.toLocaleString()} URLs`}
            {status === "analysing" && "Crawl analysis…"}
            {status === "done" && `Complete: ${done.toLocaleString()} URLs`}
          </span>
        </div>
      </header>

      <nav className="tabbar">
        {tabs.map((t) => (
          <button key={t.name} className={t.name === tab ? "active" : ""} onClick={() => pick(t.name, filters[t.name] ?? "All")}>
            {t.name}
          </button>
        ))}
      </nav>

      <div className="filterbar">
        <select value={filter} onChange={(e) => pick(tab, e.target.value)}>
          {tabDef?.filters.map((f) => (
            <option key={f.name} value={f.name}>
              {f.name}
            </option>
          ))}
        </select>
        <input className="search" placeholder="Search address…" value={search} onChange={(e) => setSearch(e.target.value)} spellCheck={false} />
        <button onClick={exportCsv} disabled={status === "idle"}>
          Export
        </button>
        {tabDef?.filters.find((f) => f.name === filter)?.description && (
          <span className="filter-desc">{tabDef.filters.find((f) => f.name === filter)!.description}</span>
        )}
        {message && (
          <span className="message" onClick={() => setMessage(null)}>
            {message}
          </span>
        )}
      </div>

      <main className="main">
        <div className="center">
          <DataGrid
            tab={tab}
            filter={filter}
            search={search}
            sort={sort}
            onSort={onSort}
            selected={selected}
            onSelect={setSelected}
            refreshKey={refreshKey}
          />
          <DetailPanel url={selected} refreshKey={refreshKey} />
        </div>
        <SidePanel refreshKey={refreshKey} onPick={pick} />
      </main>

      {showConfig && (
        <ConfigDialog
          onClose={() => {
            setShowConfig(false);
            // Custom Search/Extraction tabs depend on the configuration.
            api.tabs().then(setTabs).catch(console.error);
          }}
        />
      )}
      {showSitemap && <SitemapDialog onClose={() => setShowSitemap(false)} onDone={setMessage} />}
    </div>
  );
}
