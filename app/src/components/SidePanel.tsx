import { useEffect, useState } from "react";
import { api, type IssueRow, type OverviewRow, type Table } from "../api";

interface Props {
  refreshKey: number;
  onPick: (tab: string, filter: string) => void;
}

type View = "overview" | "issues" | "times" | "spelling";

/** Right-hand panel: Overview (every tab and filter with counts) and Issues. */
export function SidePanel({ refreshKey, onPick }: Props) {
  const [view, setView] = useState<View>("issues");
  const [overview, setOverview] = useState<OverviewRow[]>([]);
  const [issues, setIssues] = useState<IssueRow[]>([]);
  const [times, setTimes] = useState<Table | null>(null);
  const [spelling, setSpelling] = useState<Table | null>(null);
  const [bump, setBump] = useState(0);
  const [open, setOpen] = useState<Set<string>>(new Set(["Internal", "Response Codes"]));

  useEffect(() => {
    let live = true;
    const load =
      view === "overview"
        ? api.overview().then((r) => live && setOverview(r))
        : view === "issues"
          ? api.issues().then((r) => live && setIssues(r))
          : view === "times"
            ? api.report("Response Times").then((t) => live && setTimes(t))
            : api.topLanguageErrors().then((t) => live && setSpelling(t));
    load.catch(console.error);
    return () => {
      live = false;
    };
  }, [view, refreshKey, bump]);

  const grouped = new Map<string, OverviewRow[]>();
  for (const row of overview) {
    if (!grouped.has(row.tab)) grouped.set(row.tab, []);
    grouped.get(row.tab)!.push(row);
  }

  return (
    <aside className="side">
      <div className="side-tabs">
        <button className={view === "overview" ? "active" : ""} onClick={() => setView("overview")}>
          Overview
        </button>
        <button className={view === "issues" ? "active" : ""} onClick={() => setView("issues")}>
          Issues {issues.length > 0 && <span className="badge">{issues.length}</span>}
        </button>
        <button className={view === "times" ? "active" : ""} onClick={() => setView("times")}>
          Response Times
        </button>
        <button className={view === "spelling" ? "active" : ""} onClick={() => setView("spelling")}>
          Spelling & Grammar
        </button>
      </div>

      {view === "overview" && (
        <div className="side-body">
          {[...grouped.entries()].map(([tab, rows]) => (
            <div key={tab} className="tree-group">
              <button
                className="tree-head"
                onClick={() =>
                  setOpen((prev) => {
                    const next = new Set(prev);
                    next.has(tab) ? next.delete(tab) : next.add(tab);
                    return next;
                  })
                }
              >
                <span>{open.has(tab) ? "▾" : "▸"} {tab}</span>
                <span className="num">{rows.find((r) => r.filter === "All")?.urls.toLocaleString() ?? ""}</span>
              </button>
              {open.has(tab) &&
                rows
                  .filter((r) => r.filter !== "All")
                  .map((r) => (
                    <button key={r.filter} className={`tree-item${r.urls === 0 ? " muted" : ""}`} onClick={() => onPick(r.tab, r.filter)}>
                      <span>{r.filter}</span>
                      <span className="num">{r.urls.toLocaleString()}</span>
                    </button>
                  ))}
            </div>
          ))}
          {overview.length === 0 && <p className="empty">Start a crawl to see the overview.</p>}
        </div>
      )}

      {view === "times" && (
        <div className="side-body">
          {times && times.rows.length > 0 ? (
            <table className="times">
              <thead>
                <tr>
                  <th>Response Time (s)</th>
                  <th className="num">URLs</th>
                  <th className="num">% of Total</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {times.rows.map((r) => (
                  <tr key={r[0]}>
                    <td>{r[0]}</td>
                    <td className="num">{Number(r[1]).toLocaleString()}</td>
                    <td className="num">{r[2]}%</td>
                    <td className="bar-cell">
                      <span className="bar" style={{ width: `${Math.min(100, Number(r[2]))}%` }} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className="empty">Response times appear once URLs have been crawled.</p>
          )}
        </div>
      )}

      {view === "spelling" && (
        <div className="side-body">
          {spelling && spelling.rows.length > 0 ? (
            <table className="issues">
              <thead>
                <tr>
                  <th>Error</th>
                  <th>Suggestion</th>
                  <th className="num">URLs</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {spelling.rows.map((r) => {
                  const [kind, text, suggestion, urls, rule] = r;
                  const act = (action: "ignore" | "dictionary" | "ignore_rule") =>
                    api.dismissLanguageError(kind, kind === "Grammar" ? rule : text, action).then(() => setBump((b) => b + 1)).catch(console.error);
                  return (
                    <tr key={`${kind}/${text}`} title={kind === "Grammar" ? `Grammar rule ${rule}` : "Spelling"}>
                      <td>
                        <span className={`pill ${kind === "Grammar" ? "warning" : "issue"}`}>{kind === "Grammar" ? "G" : "S"}</span> {text}
                      </td>
                      <td className="muted">{suggestion}</td>
                      <td className="num">{urls}</td>
                      <td className="rule-buttons">
                        {kind === "Grammar" ? (
                          <button onClick={() => act("ignore_rule")} title="Ignore this grammar rule">
                            Ignore rule
                          </button>
                        ) : (
                          <>
                            <button onClick={() => act("ignore")} title="Ignore All: skip this word in this configuration">
                              Ignore All
                            </button>
                            <button onClick={() => act("dictionary")} title="Add to your dictionary for every crawl">
                              + Dictionary
                            </button>
                          </>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          ) : (
            <p className="empty">No spelling or grammar errors. Switch them on in Configuration &gt; Content &gt; Spelling &amp; Grammar.</p>
          )}
        </div>
      )}

      {view === "issues" && (
        <div className="side-body">
          <table className="issues">
            <thead>
              <tr>
                <th>Issue</th>
                <th>Type</th>
                <th>Priority</th>
                <th className="num">URLs</th>
                <th className="num">%</th>
              </tr>
            </thead>
            <tbody>
              {issues.map((i) => (
                <tr key={`${i.tab}/${i.filter}`} onClick={() => onPick(i.tab, i.filter)} title={i.description}>
                  <td>
                    {i.tab}: {i.filter}
                  </td>
                  <td>
                    <span className={`pill ${i.issue_type.toLowerCase()}`}>{i.issue_type}</span>
                  </td>
                  <td className={`prio ${i.priority.toLowerCase()}`}>{i.priority}</td>
                  <td className="num">{i.urls.toLocaleString()}</td>
                  <td className="num">{i.pct_of_total.toFixed(1)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {issues.length === 0 && <p className="empty">No issues yet.</p>}
        </div>
      )}
    </aside>
  );
}
