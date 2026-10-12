import { useEffect, useState } from "react";
import { api, type CrawlConfig, type CustomExtractor, type CustomSearch } from "../api";

type Section = "spider" | "sitemaps" | "content" | "search" | "extraction";

const SECTIONS: [Section, string][] = [
  ["spider", "Spider"],
  ["sitemaps", "XML Sitemaps"],
  ["content", "Duplicates"],
  ["search", "Custom Search"],
  ["extraction", "Custom Extraction"],
];

const newSearch = (): CustomSearch => ({
  name: "",
  mode: "contains",
  kind: "text",
  query: "",
  case_sensitive: false,
  search_in: "html",
});

const newExtractor = (): CustomExtractor => ({
  name: "",
  kind: "css",
  expression: "",
  attribute: null,
  extract: "text",
});

/** Configuration dialog, grouped like Screaming Frog's Configuration menu. */
export function ConfigDialog({ onClose }: { onClose: () => void }) {
  const [cfg, setCfg] = useState<CrawlConfig | null>(null);
  const [section, setSection] = useState<Section>("spider");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.getConfig().then(setCfg).catch(console.error);
  }, []);

  if (!cfg) return null;
  const set = <K extends keyof CrawlConfig>(key: K, value: CrawlConfig[K]) => setCfg({ ...cfg, [key]: value });
  const num = (v: string) => (v.trim() === "" ? null : Number(v));
  const lines = (v: string) => v.split("\n").map((l) => l.trim()).filter(Boolean);

  const updateSearch = (i: number, patch: Partial<CustomSearch>) =>
    set("custom_search", cfg.custom_search.map((s, j) => (j === i ? { ...s, ...patch } : s)));
  const updateExtractor = (i: number, patch: Partial<CustomExtractor>) =>
    set("custom_extraction", cfg.custom_extraction.map((x, j) => (j === i ? { ...x, ...patch } : x)));

  const save = async () => {
    const named = [...cfg.custom_search, ...cfg.custom_extraction].filter((r) => (r as { name: string }).name.trim() === "");
    if (named.length > 0) {
      setError("Every custom search and extractor needs a name.");
      return;
    }
    await api.setConfig(cfg);
    onClose();
  };

  const toggles: [keyof CrawlConfig, string][] = [
    ["crawl_images", "Crawl images"],
    ["crawl_css", "Crawl CSS"],
    ["crawl_js", "Crawl JavaScript"],
    ["crawl_external_links", "Check external links"],
    ["crawl_all_subdomains", "Crawl all subdomains"],
    ["crawl_outside_start_folder", "Crawl outside start folder"],
    ["follow_internal_nofollow", "Follow internal nofollow"],
  ];

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide-modal" onClick={(e) => e.stopPropagation()}>
        <h2>Configuration</h2>
        <div className="modal-layout">
          <nav className="modal-nav">
            {SECTIONS.map(([id, label]) => (
              <button key={id} className={section === id ? "active" : ""} onClick={() => setSection(id)}>
                {label}
              </button>
            ))}
          </nav>

          <div className="modal-content">
            {section === "spider" && (
              <div className="form">
                <label>
                  Max URLs
                  <input type="number" value={cfg.max_urls} onChange={(e) => set("max_urls", Number(e.target.value))} />
                </label>
                <label>
                  Max crawl depth
                  <input type="number" value={cfg.max_depth ?? ""} placeholder="No limit" onChange={(e) => set("max_depth", num(e.target.value))} />
                </label>
                <label>
                  Threads
                  <input type="number" min={1} max={50} value={cfg.threads} onChange={(e) => set("threads", Number(e.target.value))} />
                </label>
                <label>
                  Max URLs per second
                  <input type="number" step="0.1" value={cfg.max_urls_per_sec ?? ""} placeholder="No limit" onChange={(e) => set("max_urls_per_sec", num(e.target.value))} />
                </label>
                <label>
                  robots.txt
                  <select value={cfg.robots_mode} onChange={(e) => set("robots_mode", e.target.value as CrawlConfig["robots_mode"])}>
                    <option value="respect">Respect robots.txt</option>
                    <option value="ignore">Ignore robots.txt</option>
                    <option value="ignore_but_report">Ignore robots.txt but report status</option>
                  </select>
                </label>
                <label className="wide">
                  User-agent
                  <input value={cfg.user_agent} onChange={(e) => set("user_agent", e.target.value)} />
                </label>
                <label className="wide">
                  Include (regex, one per line)
                  <textarea rows={3} value={cfg.include.join("\n")} onChange={(e) => set("include", lines(e.target.value))} />
                </label>
                <label className="wide">
                  Exclude (regex, one per line)
                  <textarea rows={3} value={cfg.exclude.join("\n")} onChange={(e) => set("exclude", lines(e.target.value))} />
                </label>
                <fieldset className="wide toggles">
                  {toggles.map(([key, label]) => (
                    <label key={String(key)} className="check">
                      <input type="checkbox" checked={Boolean(cfg[key])} onChange={(e) => set(key, e.target.checked as never)} />
                      {label}
                    </label>
                  ))}
                </fieldset>
              </div>
            )}

            {section === "sitemaps" && (
              <div className="form">
                <label className="check wide">
                  <input type="checkbox" checked={cfg.crawl_linked_sitemaps} onChange={(e) => set("crawl_linked_sitemaps", e.target.checked)} />
                  Crawl linked XML sitemaps
                </label>
                <label className="check wide">
                  <input
                    type="checkbox"
                    disabled={!cfg.crawl_linked_sitemaps}
                    checked={cfg.auto_discover_sitemaps_via_robots}
                    onChange={(e) => set("auto_discover_sitemaps_via_robots", e.target.checked)}
                  />
                  Auto discover XML sitemaps via robots.txt
                </label>
                <label className="wide">
                  Crawl these sitemaps (one URL per line)
                  <textarea
                    rows={5}
                    disabled={!cfg.crawl_linked_sitemaps}
                    value={cfg.sitemap_urls.join("\n")}
                    onChange={(e) => set("sitemap_urls", lines(e.target.value))}
                  />
                </label>
                <p className="hint wide">Sitemap URLs are crawled and compared with the site in the Sitemaps tab: orphans, URLs missing from sitemaps, and non-indexable URLs listed in them.</p>
              </div>
            )}

            {section === "content" && (
              <div className="form">
                <label className="check wide">
                  <input type="checkbox" checked={cfg.near_duplicates} onChange={(e) => set("near_duplicates", e.target.checked)} />
                  Enable near duplicates
                </label>
                <label>
                  Similarity threshold (%)
                  <input
                    type="number"
                    min={50}
                    max={100}
                    disabled={!cfg.near_duplicates}
                    value={Math.round(cfg.near_duplicate_threshold * 100)}
                    onChange={(e) => set("near_duplicate_threshold", Number(e.target.value) / 100)}
                  />
                </label>
                <p className="hint wide">Pages are compared on the text of their content area (body minus nav and footer). Exact duplicates are always detected.</p>
              </div>
            )}

            {section === "search" && (
              <div className="rules">
                {cfg.custom_search.map((s, i) => (
                  <div key={i} className="rule">
                    <input placeholder="Name" value={s.name} onChange={(e) => updateSearch(i, { name: e.target.value })} />
                    <select value={s.mode} onChange={(e) => updateSearch(i, { mode: e.target.value as CustomSearch["mode"] })}>
                      <option value="contains">Contains</option>
                      <option value="does_not_contain">Does Not Contain</option>
                    </select>
                    <select value={s.kind} onChange={(e) => updateSearch(i, { kind: e.target.value as CustomSearch["kind"] })}>
                      <option value="text">Text</option>
                      <option value="regex">Regex</option>
                    </select>
                    <input className="grow" placeholder="Search for…" value={s.query} onChange={(e) => updateSearch(i, { query: e.target.value })} />
                    <select value={s.search_in} onChange={(e) => updateSearch(i, { search_in: e.target.value as CustomSearch["search_in"] })}>
                      <option value="html">HTML</option>
                      <option value="html_head">HTML head</option>
                      <option value="html_body">HTML body</option>
                      <option value="page_text">Page text</option>
                      <option value="content_area">Content area</option>
                    </select>
                    <button onClick={() => set("custom_search", cfg.custom_search.filter((_, j) => j !== i))} title="Remove">
                      ✕
                    </button>
                  </div>
                ))}
                <button onClick={() => set("custom_search", [...cfg.custom_search, newSearch()])}>Add search</button>
                <p className="hint">Text searches ignore case. Regex searches are case-sensitive unless they start with (?i).</p>
              </div>
            )}

            {section === "extraction" && (
              <div className="rules">
                {cfg.custom_extraction.map((x, i) => (
                  <div key={i} className="rule">
                    <input placeholder="Name" value={x.name} onChange={(e) => updateExtractor(i, { name: e.target.value })} />
                    <select value={x.kind} onChange={(e) => updateExtractor(i, { kind: e.target.value as CustomExtractor["kind"] })}>
                      <option value="css">CSS selector</option>
                      <option value="regex">Regex</option>
                    </select>
                    <input
                      className="grow"
                      placeholder={x.kind === "css" ? "e.g. span.price" : 'e.g. "sku":"([^"]+)"'}
                      value={x.expression}
                      onChange={(e) => updateExtractor(i, { expression: e.target.value })}
                    />
                    {x.kind === "css" && (
                      <>
                        <input
                          placeholder="Attribute (optional)"
                          value={x.attribute ?? ""}
                          onChange={(e) => updateExtractor(i, { attribute: e.target.value || null })}
                        />
                        <select value={x.extract} disabled={!!x.attribute} onChange={(e) => updateExtractor(i, { extract: e.target.value as CustomExtractor["extract"] })}>
                          <option value="text">Extract text</option>
                          <option value="inner_html">Extract inner HTML</option>
                          <option value="element">Extract HTML element</option>
                        </select>
                      </>
                    )}
                    <button onClick={() => set("custom_extraction", cfg.custom_extraction.filter((_, j) => j !== i))} title="Remove">
                      ✕
                    </button>
                  </div>
                ))}
                <button onClick={() => set("custom_extraction", [...cfg.custom_extraction, newExtractor()])}>Add extractor</button>
                <p className="hint">Regex extractors return the first capture group, or the whole match if there is none.</p>
              </div>
            )}
          </div>
        </div>

        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <button onClick={() => api.resetConfig().then(setCfg)}>Reset to defaults</button>
          <span className="spacer" />
          <button onClick={onClose}>Cancel</button>
          <button className="primary" onClick={save}>
            OK
          </button>
        </div>
      </div>
    </div>
  );
}
