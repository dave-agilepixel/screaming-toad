import { useEffect, useState } from "react";
import { api, type Table } from "../api";
import { SourceView } from "./SourceView";

type View = "details" | "inlinks" | "outlinks" | "duplicates" | "structured" | "headers" | "cookies" | "source";

const LABELS: Record<View, string> = {
  details: "URL Details",
  inlinks: "Inlinks",
  outlinks: "Outlinks",
  duplicates: "Duplicate Details",
  structured: "Structured Data Details",
  headers: "HTTP Headers",
  cookies: "Cookies",
  source: "View Source",
};

/** Lower window: URL Details, Inlinks and Outlinks for the selected URL. */
export function DetailPanel({ url, refreshKey }: { url: string | null; refreshKey: number }) {
  const [view, setView] = useState<View>("details");
  const [details, setDetails] = useState<[string, string][]>([]);
  const [links, setLinks] = useState<Table | null>(null);
  const [source, setSource] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    if (!url) return;
    let live = true;
    if (view === "details") {
      api.urlDetails(url).then((d) => live && setDetails(d)).catch(console.error);
    } else if (view === "source") {
      setSource(undefined);
      api.pageSource(url).then((s) => live && setSource(s)).catch(console.error);
    } else {
      const load =
        view === "inlinks"
          ? api.inlinks(url)
          : view === "outlinks"
            ? api.outlinks(url)
            : view === "duplicates"
              ? api.nearDuplicates(url)
              : view === "headers"
                ? api.httpHeaders(url)
                : view === "cookies"
                  ? api.cookies(url)
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
        {url && view === "source" && source !== undefined && <SourceView source={source} />}
        {url && view !== "details" && view !== "source" && links && (
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
