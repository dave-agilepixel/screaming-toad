import { useCallback, useEffect, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  api,
  compareApi,
  COMPARE_STATES,
  COMPARE_STATE_TEXT,
  events,
  type ChangeState,
  type CompareState,
  type CompareSummary,
  type CrawlInfo,
  type Progress,
  type ReportDef,
  type TabDef,
} from "./api";
import { DataGrid } from "./components/DataGrid";
import { SidePanel } from "./components/SidePanel";
import { DetailPanel } from "./components/DetailPanel";
import { ConfigDialog } from "./components/ConfigDialog";
import { SitemapDialog } from "./components/SitemapDialog";
import { CompareSidePanel } from "./components/CompareSidePanel";
import { CompareDetailPanel } from "./components/CompareDetailPanel";
import { CompareConfigDialog } from "./components/CompareConfigDialog";

type Status = "idle" | "crawling" | "paused" | "analysing" | "done";
type Mode = "spider" | "compare";
type CompareStatus = "idle" | "comparing" | "ready";

const BROWSE = "__browse__";

function crawlLabel(c: CrawlInfo): string {
  const when = c.modified ? new Date(c.modified * 1000).toLocaleString() : "";
  const site = c.start_url ? c.start_url.replace(/^https?:\/\//, "").replace(/\/$/, "") : c.name;
  return `${site} / ${when} (${c.urls.toLocaleString()} URLs)`;
}

export default function App() {
  const [mode, setMode] = useState<Mode>("spider");
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

  // Compare mode.
  const [crawls, setCrawls] = useState<CrawlInfo[]>([]);
  const [currentPath, setCurrentPath] = useState("");
  const [previousPath, setPreviousPath] = useState("");
  const [compareStatus, setCompareStatus] = useState<CompareStatus>("idle");
  const [summary, setSummary] = useState<CompareSummary | null>(null);
  const [compareTabs, setCompareTabs] = useState<TabDef[]>([]);
  const [compareState, setCompareState] = useState<CompareState>("Current");
  const [changeState, setChangeState] = useState<ChangeState>("Changed");
  const [showCompareConfig, setShowCompareConfig] = useState(false);

  const comparing = mode === "compare";
  const activeTabs = comparing ? compareTabs : tabs;
  const filter = filters[tab] ?? "All";
  const tabDef = activeTabs.find((t) => t.name === tab);
  const isChangeDetection = comparing && tab === "Change Detection";
  const gridState = comparing && summary ? (isChangeDetection ? changeState : compareState) : undefined;
  const previousSide = comparing && !isChangeDetection && ["Previous", "Removed", "Missing"].includes(compareState);

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

  const loadCrawls = useCallback(async () => {
    const list = await compareApi.listCrawls();
    setCrawls((prev) => {
      // Keep crawls picked with Browse… that live outside the app folder.
      const extra = prev.filter((p) => !list.some((c) => c.path === p.path));
      return [...list, ...extra];
    });
    // Default: newest crawl is Current, the one before it Previous.
    setCurrentPath((cur) => cur || list[0]?.path || "");
    setPreviousPath((prev) => prev || list[1]?.path || "");
  }, []);

  const switchMode = async (m: Mode) => {
    setMode(m);
    setSelected(null);
    setSort(null);
    setMessage(null);
    if (m === "compare") {
      loadCrawls().catch((e) => setMessage(String(e)));
      const s = await compareApi.summary().catch(() => null);
      if (s) {
        setSummary(s);
        setCompareTabs(await compareApi.tabs());
        setCompareStatus("ready");
      }
    }
    setTab("Internal");
    refresh();
  };

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
      defaultPath: `${tab}-${filter}${gridState ? `-${gridState}` : ""}.csv`.replace(/[^\w.-]+/g, "_").toLowerCase(),
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    try {
      const n = gridState
        ? await compareApi.exportCsv({ tab, filter, state: gridState, search: search || undefined, offset: 0 }, path)
        : await api.exportCsv(tab, filter, search || undefined, path);
      setMessage(`Exported ${n.toLocaleString()} rows to ${path}`);
    } catch (e) {
      setMessage(String(e));
    }
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

  const pick = (t: string, f: string, state?: CompareState) => {
    setTab(t);
    setFilters((prev) => ({ ...prev, [t]: f }));
    setSort(null);
    if (state) {
      if (t === "Change Detection") setChangeState("Changed");
      else setCompareState(state);
    }
  };

  const onSort = (column: string) =>
    setSort((prev) => (prev?.column === column ? (prev.desc ? null : { column, desc: true }) : { column, desc: false }));

  // ------------------------------------------------------------ Compare actions

  const chooseCrawl = async (which: "current" | "previous", value: string) => {
    let path = value;
    if (value === BROWSE) {
      const picked = await open({ filters: [{ name: "Screaming Toad crawl", extensions: ["toad"] }] });
      if (typeof picked !== "string") return;
      try {
        const info = await compareApi.crawlInfo(picked);
        setCrawls((prev) => (prev.some((c) => c.path === info.path) ? prev : [...prev, info]));
        path = info.path;
      } catch (e) {
        setMessage(String(e));
        return;
      }
    }
    (which === "current" ? setCurrentPath : setPreviousPath)(path);
  };

  const runCompare = async () => {
    if (!currentPath || !previousPath) return;
    if (currentPath === previousPath) {
      setMessage("Choose two different crawls.");
      return;
    }
    setMessage(null);
    setCompareStatus("comparing");
    try {
      const s = await compareApi.run(currentPath, previousPath);
      setSummary(s);
      setCompareTabs(await compareApi.tabs());
      setCompareStatus("ready");
      setSelected(null);
      if (s.config.elements.includes("Content") && !s.content_available) {
        setMessage("Content change detection needs Near Duplicates switched on in both crawls.");
      }
      refresh();
    } catch (e) {
      setCompareStatus(summary ? "ready" : "idle");
      setMessage(String(e));
    }
  };

  const swap = () => {
    setCurrentPath(previousPath);
    setPreviousPath(currentPath);
  };

  const clearCompare = async () => {
    await compareApi.close();
    setSummary(null);
    setCompareTabs([]);
    setCompareStatus("idle");
    setSelected(null);
    refresh();
  };

  const openComparison = async () => {
    const path = await open({ filters: [{ name: "Screaming Toad comparison", extensions: ["toadcmp"] }] });
    if (typeof path !== "string") return;
    setCompareStatus("comparing");
    try {
      const s = await compareApi.open(path);
      setSummary(s);
      setCurrentPath(s.current_path);
      setPreviousPath(s.previous_path);
      await loadCrawls();
      setCompareTabs(await compareApi.tabs());
      setCompareStatus("ready");
      refresh();
    } catch (e) {
      setCompareStatus(summary ? "ready" : "idle");
      setMessage(String(e));
    }
  };

  const exportCompareOverview = async () => {
    const path = await save({ defaultPath: "crawl_overview_comparison.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    const n = await compareApi.exportOverview(path);
    setMessage(`Crawl Overview: ${n.toLocaleString()} rows → ${path}`);
  };

  const crawlOptions = (value: string) => (
    <>
      {!value && <option value="">Select Crawl</option>}
      {crawls.map((c) => (
        <option key={c.path} value={c.path} title={c.path}>
          {crawlLabel(c)}
        </option>
      ))}
      <option value={BROWSE}>Browse…</option>
    </>
  );

  const crawling = status === "crawling" || status === "paused";
  const done = progress?.crawled ?? 0;
  const pct = progress && done + progress.queued > 0 ? (done / (done + progress.queued)) * 100 : 0;
  const filterDesc = tabDef?.filters.find((f) => f.name === filter)?.description;

  return (
    <div className="app">
      <header className="toolbar">
        <div className="brand">🐸 Screaming Toad</div>
        <select className="mode" value={mode} onChange={(e) => switchMode(e.target.value as Mode)} disabled={crawling} title="Mode">
          <option value="spider">Spider</option>
          <option value="compare">Compare</option>
        </select>

        {!comparing && (
          <>
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
          </>
        )}

        {comparing && (
          <>
            <label className="crawl-pick">
              Current:
              <select value={currentPath} onChange={(e) => chooseCrawl("current", e.target.value)}>
                {crawlOptions(currentPath)}
              </select>
            </label>
            <button onClick={swap} disabled={!currentPath || !previousPath} title="Swap current and previous">
              ⇄
            </button>
            <label className="crawl-pick">
              Previous:
              <select value={previousPath} onChange={(e) => chooseCrawl("previous", e.target.value)}>
                {crawlOptions(previousPath)}
              </select>
            </label>
            <button className="primary" onClick={runCompare} disabled={!currentPath || !previousPath || compareStatus === "comparing"}>
              Compare
            </button>
            <button onClick={clearCompare} disabled={!summary}>
              Clear
            </button>
            <button onClick={() => setShowCompareConfig(true)} title="Config > Compare: Change Detection and URL Mapping">
              ⚙ Compare Config
            </button>
            <button onClick={openComparison}>Open Comparison</button>
            <button onClick={exportCompareOverview} disabled={!summary} title="Reports > Crawl Overview for this comparison">
              Export Overview
            </button>
            <div className="progress">
              <div className="bar" style={{ width: compareStatus === "comparing" ? "50%" : compareStatus === "ready" ? "100%" : "0%" }} />
              <span>
                {compareStatus === "idle" && "Compare Mode: Idle"}
                {compareStatus === "comparing" && "Comparing…"}
                {compareStatus === "ready" &&
                  summary &&
                  `${summary.current_urls.toLocaleString()} vs ${summary.previous_urls.toLocaleString()} URLs · ${summary.in_both.toLocaleString()} in both`}
              </span>
            </div>
          </>
        )}
      </header>

      <nav className="tabbar">
        {activeTabs.map((t) => (
          <button key={t.name} className={t.name === tab ? "active" : ""} onClick={() => pick(t.name, filters[t.name] ?? "All")}>
            {t.name}
          </button>
        ))}
        {comparing && activeTabs.length === 0 && <span className="muted tab-hint">Choose a current and previous crawl, then press Compare.</span>}
      </nav>

      <div className="filterbar">
        <select value={filter} onChange={(e) => pick(tab, e.target.value)}>
          {tabDef?.filters.map((f) => (
            <option key={f.name} value={f.name}>
              {f.name}
            </option>
          ))}
        </select>
        {comparing && summary && !isChangeDetection && (
          <select className="state" value={compareState} onChange={(e) => setCompareState(e.target.value as CompareState)}>
            {COMPARE_STATES.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </select>
        )}
        {isChangeDetection && filter !== "All" && (
          <select className="state" value={changeState} onChange={(e) => setChangeState(e.target.value as ChangeState)}>
            <option value="Changed">Changed</option>
            <option value="Not Changed">Not Changed</option>
          </select>
        )}
        <input className="search" placeholder="Search address…" value={search} onChange={(e) => setSearch(e.target.value)} spellCheck={false} />
        <button onClick={exportCsv} disabled={comparing ? !summary : status === "idle"}>
          Export
        </button>
        {comparing && summary && !isChangeDetection ? (
          <span className="filter-desc">Showing: {COMPARE_STATE_TEXT[compareState]}</span>
        ) : (
          filterDesc && <span className="filter-desc">{filterDesc}</span>
        )}
        {message && (
          <span className="message" onClick={() => setMessage(null)}>
            {message}
          </span>
        )}
      </div>

      <main className="main">
        <div className="center">
          {comparing && !summary ? (
            <div className="grid">
              <p className="empty">
                Compare mode diffs two saved crawls. Pick the <b>Current</b> and <b>Previous</b> crawls above, then press <b>Compare</b>.
                Use ⚙ Compare Config for Change Detection and URL Mapping (for example staging → live).
              </p>
            </div>
          ) : (
            <DataGrid
              tab={tab}
              filter={filter}
              search={search}
              sort={sort}
              onSort={onSort}
              selected={selected}
              onSelect={setSelected}
              refreshKey={refreshKey}
              compareState={gridState}
            />
          )}
          {comparing ? (
            <CompareDetailPanel url={summary ? selected : null} refreshKey={refreshKey} previousSide={previousSide} />
          ) : (
            <DetailPanel url={selected} refreshKey={refreshKey} />
          )}
        </div>
        {comparing ? (
          summary ? (
            <CompareSidePanel refreshKey={refreshKey} onPick={pick} />
          ) : (
            <aside className="side">
              <p className="empty">The Overview, Issues and Site Structure comparisons appear here after Compare.</p>
            </aside>
          )
        ) : (
          <SidePanel refreshKey={refreshKey} onPick={pick} />
        )}
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
      {showCompareConfig && (
        <CompareConfigDialog
          onClose={(saved) => {
            setShowCompareConfig(false);
            if (saved && summary) setMessage("Compare configuration saved. Press Compare to apply it.");
          }}
        />
      )}
    </div>
  );
}
