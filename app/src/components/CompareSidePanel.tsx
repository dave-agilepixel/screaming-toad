import { useEffect, useState } from "react";
import { compareApi, type CompareIssueRow, type CompareOverviewRow, type CompareState, type Table } from "../api";

interface Props {
  refreshKey: number;
  onPick: (tab: string, filter: string, state: CompareState) => void;
}

type View = "overview" | "issues" | "structure";

/**
 * Whether a rise is good news. Issue counts and crawl depth are better when
 * they fall; everything else in the Overview reads as growth.
 */
type Polarity = "up-good" | "down-good" | "neutral";

function Delta({ change, pct, polarity }: { change: number; pct?: number | null; polarity: Polarity }) {
  if (change === 0) return <span className="delta">0</span>;
  const up = change > 0;
  const good = polarity === "neutral" ? null : (polarity === "up-good") === up;
  return (
    <span className={`delta ${good === null ? "" : good ? "good" : "bad"}`}>
      {up ? "▲" : "▼"} {up ? "+" : ""}
      {change.toLocaleString()}
      {pct !== undefined && pct !== null && <span className="delta-pct"> ({pct > 0 ? "+" : ""}{pct.toFixed(2)}%)</span>}
    </span>
  );
}

/** A count that opens its filter in the grid with a compare state selected. */
function Count({ n, onClick, title }: { n: number | null; onClick: () => void; title: string }) {
  if (n === null) return <span className="muted">-</span>;
  if (n === 0) return <span className="muted">0</span>;
  return (
    <button className="count-link" onClick={onClick} title={title}>
      {n.toLocaleString()}
    </button>
  );
}

/** Right-hand panel in Compare mode: Overview, Issues and Site Structure with deltas. */
export function CompareSidePanel({ refreshKey, onPick }: Props) {
  const [view, setView] = useState<View>("overview");
  const [overview, setOverview] = useState<CompareOverviewRow[]>([]);
  const [issues, setIssues] = useState<CompareIssueRow[]>([]);
  const [structure, setStructure] = useState<Table | null>(null);
  const [depth, setDepth] = useState(2);
  const [open, setOpen] = useState<Set<string>>(new Set(["Internal", "Response Codes", "Change Detection"]));

  useEffect(() => {
    let live = true;
    const load =
      view === "overview"
        ? compareApi.overview().then((r) => live && setOverview(r))
        : view === "issues"
          ? compareApi.issues().then((r) => live && setIssues(r))
          : compareApi.siteStructure(depth).then((t) => live && setStructure(t));
    load.catch(console.error);
    return () => {
      live = false;
    };
  }, [view, refreshKey, depth]);

  const grouped = new Map<string, CompareOverviewRow[]>();
  for (const row of overview) {
    if (!grouped.has(row.tab)) grouped.set(row.tab, []);
    grouped.get(row.tab)!.push(row);
  }

  const chips = (["Issue", "Warning", "Opportunity"] as const).map((t) => {
    const rows = issues.filter((i) => i.issue_type === t);
    const current = rows.filter((i) => i.current > 0).length;
    const previous = rows.filter((i) => i.previous > 0).length;
    return { t, current, change: current - previous };
  });

  return (
    <aside className="side compare-side">
      <div className="side-tabs">
        <button className={view === "overview" ? "active" : ""} onClick={() => setView("overview")}>
          Overview
        </button>
        <button className={view === "issues" ? "active" : ""} onClick={() => setView("issues")}>
          Issues
        </button>
        <button className={view === "structure" ? "active" : ""} onClick={() => setView("structure")}>
          Site Structure
        </button>
      </div>

      {view === "overview" && (
        <div className="side-body">
          <table className="compare-table">
            <thead>
              <tr>
                <th></th>
                <th className="num">Current</th>
                <th className="num">Previous</th>
                <th className="num">Change</th>
                <th className="num" title="URLs in both crawls that entered the filter">Added</th>
                <th className="num" title="URLs only in the current crawl">New</th>
                <th className="num" title="URLs in both crawls that left the filter">Removed</th>
                <th className="num" title="URLs only in the previous crawl">Missing</th>
              </tr>
            </thead>
            {[...grouped.entries()].map(([tab, rows]) => {
              const all = rows.find((r) => r.filter === "All");
              const isOpen = open.has(tab);
              const isCd = tab === "Change Detection";
              return (
                <tbody key={tab}>
                  <tr className="group-row">
                    <td>
                      <button
                        className="tree-toggle"
                        onClick={() =>
                          setOpen((prev) => {
                            const next = new Set(prev);
                            next.has(tab) ? next.delete(tab) : next.add(tab);
                            return next;
                          })
                        }
                      >
                        {isOpen ? "▾" : "▸"} {tab}
                      </button>
                    </td>
                    <td className="num">{all?.current.toLocaleString()}</td>
                    <td className="num">{all?.previous.toLocaleString()}</td>
                    <td className="num">{all && !isCd && <Delta change={all.change} polarity="neutral" />}</td>
                    <td colSpan={4}></td>
                  </tr>
                  {isOpen &&
                    rows
                      .filter((r) => r.filter !== "All")
                      .map((r) => {
                        const pick = (s: CompareState) => () => onPick(r.tab, r.filter, s);
                        return (
                          <tr key={r.filter} className={r.current === 0 && r.previous === 0 ? "muted-row" : ""}>
                            <td className="filter-cell" title={r.filter}>
                              <button className="link-button" onClick={pick("Current")}>
                                {r.filter}
                              </button>
                            </td>
                            {isCd ? (
                              <>
                                <td className="num" colSpan={2}>
                                  <Count n={r.current} onClick={pick("Current")} title="Changed pages" />
                                </td>
                                <td className="num">{r.change_pct !== null && <span className="muted">{r.change_pct.toFixed(2)}%</span>}</td>
                                <td colSpan={4}></td>
                              </>
                            ) : (
                              <>
                                <td className="num">
                                  <Count n={r.current} onClick={pick("Current")} title="URLs in current crawl" />
                                </td>
                                <td className="num">
                                  <Count n={r.previous} onClick={pick("Previous")} title="URLs in previous crawl" />
                                </td>
                                <td className="num">
                                  <Delta change={r.change} polarity="neutral" />
                                </td>
                                <td className="num">
                                  <Count n={r.added} onClick={pick("Added")} title="Added" />
                                </td>
                                <td className="num">
                                  <Count n={r.new} onClick={pick("New")} title="New" />
                                </td>
                                <td className="num">
                                  <Count n={r.removed} onClick={pick("Removed")} title="Removed" />
                                </td>
                                <td className="num">
                                  <Count n={r.missing} onClick={pick("Missing")} title="Missing" />
                                </td>
                              </>
                            )}
                          </tr>
                        );
                      })}
                </tbody>
              );
            })}
          </table>
          {overview.length === 0 && <p className="empty">Select two crawls and press Compare.</p>}
        </div>
      )}

      {view === "issues" && (
        <div className="side-body">
          <div className="chips">
            {chips.map((c) => (
              <span key={c.t} className={`chip ${c.t.toLowerCase()}`}>
                {c.t}s: {c.current} {c.change !== 0 && <Delta change={c.change} polarity="down-good" />}
              </span>
            ))}
          </div>
          <table className="compare-table issues">
            <thead>
              <tr>
                <th>Issue</th>
                <th>Type</th>
                <th className="num">Current</th>
                <th className="num">Previous</th>
                <th className="num">Change</th>
                <th className="num">Added</th>
                <th className="num">New</th>
                <th className="num">Removed</th>
                <th className="num">Missing</th>
              </tr>
            </thead>
            <tbody>
              {issues.map((i) => {
                const pick = (s: CompareState) => () => onPick(i.tab, i.filter, s);
                return (
                  <tr key={`${i.tab}/${i.filter}`} title={i.description}>
                    <td className="filter-cell">
                      <button className="link-button" onClick={pick("Current")}>
                        {i.tab}: {i.filter}
                      </button>
                    </td>
                    <td>
                      <span className={`pill ${i.issue_type.toLowerCase()}`}>{i.issue_type}</span>{" "}
                      <span className={`prio ${i.priority.toLowerCase()}`}>{i.priority}</span>
                    </td>
                    <td className="num">
                      <Count n={i.current} onClick={pick("Current")} title="Current" />
                    </td>
                    <td className="num">
                      <Count n={i.previous} onClick={pick("Previous")} title="Previous" />
                    </td>
                    <td className="num">
                      <Delta change={i.change} polarity="down-good" />
                    </td>
                    <td className="num">
                      <Count n={i.added} onClick={pick("Added")} title="Added" />
                    </td>
                    <td className="num">
                      <Count n={i.new} onClick={pick("New")} title="New" />
                    </td>
                    <td className="num">
                      <Count n={i.removed} onClick={pick("Removed")} title="Removed" />
                    </td>
                    <td className="num">
                      <Count n={i.missing} onClick={pick("Missing")} title="Missing" />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {issues.length === 0 && <p className="empty">No issues in either crawl.</p>}
        </div>
      )}

      {view === "structure" && (
        <div className="side-body">
          <div className="structure-bar">
            <label>
              Level{" "}
              <select value={depth} onChange={(e) => setDepth(Number(e.target.value))}>
                {[1, 2, 3, 4, 5, 6, 7, 8].map((d) => (
                  <option key={d} value={d}>
                    {d}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {structure && (
            <table className="compare-table">
              <thead>
                <tr>
                  {structure.columns.map((c) => (
                    <th key={c} className={c === "Path" ? "" : "num"}>
                      {c}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {structure.rows.map((r) => {
                  const level = r[0].split("/").filter(Boolean).length;
                  const change = Number(r[3]);
                  return (
                    <tr key={r[0]}>
                      <td style={{ paddingLeft: 8 + level * 12 }} title={r[0]}>
                        {r[0]}
                      </td>
                      <td className="num">{Number(r[1]).toLocaleString()}</td>
                      <td className="num">{Number(r[2]).toLocaleString()}</td>
                      <td className="num">
                        <Delta change={change} polarity="neutral" />
                      </td>
                      <td className="num muted">{r[4] && `${r[4]}%`}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>
      )}
    </aside>
  );
}
