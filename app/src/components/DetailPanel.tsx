import { useEffect, useState } from "react";
import { api, type Table } from "../api";
import { SourceView } from "./SourceView";

type View =
  | "details"
  | "inlinks"
  | "outlinks"
  | "duplicates"
  | "structured"
  | "spelling"
  | "headers"
  | "cookies"
  | "source"
  | "rendered"
  | "console"
  | "blocked"
  | "accessibility";

const LABELS: Record<View, string> = {
  details: "URL Details",
  inlinks: "Inlinks",
  outlinks: "Outlinks",
  duplicates: "Duplicate Details",
  structured: "Structured Data Details",
  spelling: "Spelling & Grammar",
  headers: "HTTP Headers",
  cookies: "Cookies",
  source: "View Source",
  rendered: "Rendered Page",
  console: "Chrome Console Log",
  blocked: "Blocked Resources",
  accessibility: "Accessibility Details",
};

/** Lower window: URL Details, Inlinks and Outlinks for the selected URL. */
export function DetailPanel({ url, refreshKey }: { url: string | null; refreshKey: number }) {
  const [view, setView] = useState<View>("details");
  const [details, setDetails] = useState<[string, string][]>([]);
  const [links, setLinks] = useState<Table | null>(null);
  const [source, setSource] = useState<string | null | undefined>(undefined);
  const [renderedSource, setRenderedSource] = useState<string | null>(null);
  const [sourceSide, setSourceSide] = useState<"original" | "rendered">("original");
  const [shot, setShot] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    if (!url) return;
    let live = true;
    if (view === "details") {
      api.urlDetails(url).then((d) => live && setDetails(d)).catch(console.error);
    } else if (view === "source") {
      setSource(undefined);
      setRenderedSource(null);
      api.pageSource(url).then((s) => live && setSource(s)).catch(console.error);
      api.renderedSource(url).then((s) => live && setRenderedSource(s)).catch(console.error);
    } else if (view === "rendered") {
      setShot(undefined);
      api.screenshot(url).then((s) => live && setShot(s)).catch(console.error);
    } else {
      const load =
        view === "inlinks"
          ? api.inlinks(url)
          : view === "outlinks"
            ? api.outlinks(url)
            : view === "duplicates"
              ? api.nearDuplicates(url)
              : view === "spelling"
                ? api.languageDetails(url)
                : view === "headers"
                ? api.httpHeaders(url)
                : view === "cookies"
                  ? api.cookies(url)
                  : view === "console"
                    ? api.consoleLog(url)
                    : view === "blocked"
                      ? api.blockedResources(url)
                      : view === "accessibility"
                        ? api.accessibilityDetails(url)
                        : api.structuredDetails(url);
      load.then((t) => live && setLinks(t)).catch(console.error);
    }
    return () => {
      live = false;
    };
  }, [url, view, refreshKey]);

  return (
    <section className="detail">
      <div className="detail-tabs">
        {(Object.keys(LABELS) as View[]).map((v) => (
          <button key={v} className={view === v ? "active" : ""} onClick={() => setView(v)}>
            {LABELS[v]}
          </button>
        ))}
        <span className="detail-url" title={url ?? ""}>
          {url ?? "Select a URL"}
        </span>
      </div>
      <div className="detail-body">
        {!url && <p className="empty">Select a URL in the table above.</p>}
        {url && view === "details" && (
          <table className="kv">
            <tbody>
              {details
                .filter(([k, v]) => v !== "" && k !== "headers_json" && k !== "minhash")
                .map(([k, v]) => (
                  <tr key={k}>
                    <th>{k.replace(/_/g, " ")}</th>
                    <td>{v}</td>
                  </tr>
                ))}
            </tbody>
          </table>
        )}
        {url && view === "source" && source !== undefined && (
          <>
            {renderedSource !== null && (
              <div className="source-switch">
                <button className={sourceSide === "original" ? "active" : ""} onClick={() => setSourceSide("original")}>
                  Original HTML
                </button>
                <button className={sourceSide === "rendered" ? "active" : ""} onClick={() => setSourceSide("rendered")}>
                  Rendered HTML
                </button>
              </div>
            )}
            <SourceView source={sourceSide === "rendered" && renderedSource !== null ? renderedSource : source} />
          </>
        )}
        {url && view === "rendered" && shot !== undefined && (
          shot === null ? (
            <p className="empty">No screenshot for this URL. Crawl with Configuration &gt; Spider &gt; Rendering set to JavaScript, with screenshots on.</p>
          ) : (
            <div className="screenshot">
              <img src={shot} alt={`Rendered page: ${url}`} />
            </div>
          )
        )}
        {url && view !== "details" && view !== "source" && view !== "rendered" && links && (
          <table className="links">
            <thead>
              <tr>
                {links.columns.map((c) => (
                  <th key={c}>{c}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {links.rows.map((r, i) => (
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
      </div>
    </section>
  );
}
