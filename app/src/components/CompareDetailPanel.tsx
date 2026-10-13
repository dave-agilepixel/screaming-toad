import { useEffect, useState } from "react";
import { compareApi, type LinkState, type SourceCompare, type Table } from "../api";
import { SourceCompareView, SourceView } from "./SourceView";

type View = "details" | "inlinks" | "outlinks" | "source" | "rendered";
type SourceChoice = "Auto" | "Current" | "Previous" | "Compare";
type LinkChoice = "Auto" | LinkState;

const LABELS: Record<View, string> = {
  details: "URL Details",
  inlinks: "Inlinks",
  outlinks: "Outlinks",
  source: "View Source",
  rendered: "Rendered Page",
};

const LINK_TEXT: Record<LinkState, string> = {
  Current: "Links from the current crawl",
  Previous: "Links from the previous crawl; URLs have been remapped",
  Added: "Links in the current crawl that don't exist in the previous crawl",
  Removed: "Links from the previous crawl that don't exist in the current crawl; URLs have been remapped",
};

interface Props {
  url: string | null;
  refreshKey: number;
  /** Whether the grid is showing previous-crawl rows (Previous, Removed, Missing). */
  previousSide: boolean;
}

/** Lower window in Compare mode: URL Details side by side, and link diffs. */
export function CompareDetailPanel({ url, refreshKey, previousSide }: Props) {
  const [view, setView] = useState<View>("details");
  const [linkChoice, setLinkChoice] = useState<LinkChoice>("Auto");
  const [onlyChanged, setOnlyChanged] = useState(false);
  const [table, setTable] = useState<Table | null>(null);
  const [src, setSrc] = useState<SourceCompare | null>(null);
  const [shots, setShots] = useState<[string | null, string | null] | null>(null);
  const [sourceChoice, setSourceChoice] = useState<SourceChoice>("Auto");
  const [showDifferences, setShowDifferences] = useState(true);
  const [onlyChanges, setOnlyChanges] = useState(false);
  const sourceMode = sourceChoice === "Auto" ? (previousSide ? "Previous" : "Current") : sourceChoice;

  // Auto follows the master view: previous-crawl rows show previous links.
  const linkState: LinkState = linkChoice === "Auto" ? (previousSide ? "Previous" : "Current") : linkChoice;

  useEffect(() => {
    if (!url) return;
    let live = true;
    if (view === "source") {
      setSrc(null);
      compareApi.source(url).then((s) => live && setSrc(s)).catch(console.error);
      return () => {
        live = false;
      };
    }
    if (view === "rendered") {
      setShots(null);
      compareApi.screenshots(url).then((s) => live && setShots(s)).catch(console.error);
      return () => {
        live = false;
      };
    }
    const load = view === "details" ? compareApi.urlDetails(url) : compareApi.links(url, view === "outlinks", linkState);
    load.then((t) => live && setTable(t)).catch((e) => {
      console.error(e);
      if (live) setTable(null);
    });
    return () => {
      live = false;
    };
  }, [url, view, linkState, refreshKey]);

  const rows =
    table && view === "details"
      ? table.rows.filter((r) => (r[1] !== "" || r[2] !== "") && (!onlyChanged || r[1] !== r[2]))
      : (table?.rows ?? []);

  return (
    <section className="detail">
      <div className="detail-tabs">
        {(Object.keys(LABELS) as View[]).map((v) => (
          <button key={v} className={view === v ? "active" : ""} onClick={() => setView(v)}>
            {LABELS[v]}
          </button>
        ))}
        {view === "source" || view === "rendered" ? (
          <>
            <select value={sourceChoice} onChange={(e) => setSourceChoice(e.target.value as SourceChoice)} title="Compare: side-by-side comparison of current and previous URL">
              {(["Auto", "Current", "Previous", "Compare"] as SourceChoice[]).map((s) => (
                <option key={s} value={s}>
                  {s}
                </option>
              ))}
            </select>
            {view === "source" && sourceMode === "Compare" && (
              <>
                <label className="check inline">
                  <input type="checkbox" checked={showDifferences} onChange={(e) => setShowDifferences(e.target.checked)} /> Show Differences
                </label>
                <label className="check inline">
                  <input type="checkbox" checked={onlyChanges} onChange={(e) => setOnlyChanges(e.target.checked)} /> Only changed lines
                </label>
                {src && <span className="muted">{src.changed} line{src.changed === 1 ? "" : "s"} changed</span>}
              </>
            )}
          </>
        ) : view !== "details" ? (
          <select value={linkChoice} onChange={(e) => setLinkChoice(e.target.value as LinkChoice)} title={LINK_TEXT[linkState]}>
            {(["Auto", "Current", "Previous", "Added", "Removed"] as LinkChoice[]).map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </select>
        ) : (
          <label className="check inline">
            <input type="checkbox" checked={onlyChanged} onChange={(e) => setOnlyChanged(e.target.checked)} /> Only differences
          </label>
        )}
        <span className="detail-url" title={url ?? ""}>
          {url ? (view === "details" || view === "source" || view === "rendered" ? url : `${LINK_TEXT[linkState]} · ${url}`) : "Select a URL"}
        </span>
      </div>
      <div className="detail-body">
        {!url && <p className="empty">Select a URL in the table above.</p>}
        {url && view === "source" && src && sourceMode === "Compare" && (
          <SourceCompareView current={src.current} previous={src.previous} lines={src.lines} showDifferences={showDifferences} onlyChanges={onlyChanges} />
        )}
        {url && view === "source" && src && sourceMode !== "Compare" && <SourceView source={sourceMode === "Current" ? src.current : src.previous} />}
        {url && view === "rendered" && shots && (
          <div className={sourceMode === "Compare" ? "screenshot-pair" : ""}>
            {(sourceMode === "Compare" ? (["Current", "Previous"] as const) : [sourceMode as "Current" | "Previous"]).map((side) => {
              const shot = side === "Current" ? shots[0] : shots[1];
              return (
                <div key={side} className="screenshot">
                  {sourceMode === "Compare" && <h4>{side}</h4>}
                  {shot ? <img src={shot} alt={`${side} crawl: ${url}`} /> : <p className="empty">No screenshot in the {side.toLowerCase()} crawl.</p>}
                </div>
              );
            })}
          </div>
        )}
        {url && table && view === "details" && (
          <table className="kv compare-kv">
            <thead>
              <tr>
                <th>Name</th>
                <th>Current Value</th>
                <th>Previous Value</th>
              </tr>
            </thead>
            <tbody>
              {rows.map(([k, cur, prev]) => (
                <tr key={k} className={cur !== prev ? "changed" : ""}>
                  <th>{k.replace(/_/g, " ")}</th>
                  <td>{cur}</td>
                  <td>{prev}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {url && table && (view === "inlinks" || view === "outlinks") && (
          <table className="links">
            <thead>
              <tr>
                {table.columns.map((c) => (
                  <th key={c}>{c}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={i}>
                  {r.map((v, j) => (
                    <td key={j} title={v}>
                      {v}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {url && table && view !== "source" && view !== "rendered" && rows.length === 0 && <p className="empty">Nothing to show.</p>}
      </div>
    </section>
  );
}
