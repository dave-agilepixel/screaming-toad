import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type SitemapOptions } from "../api";

const FREQS = ["always", "hourly", "daily", "weekly", "monthly", "yearly", "never"];
const DEPTHS = ["Start URL", "1 click", "2 clicks", "3 clicks", "4 clicks", "5+ clicks"];

const DEFAULTS: SitemapOptions = {
  include_noindex: false,
  include_canonicalised: false,
  include_paginated: false,
  include_pdfs: false,
  include_3xx: false,
  include_4xx: false,
  include_5xx: false,
  // Screaming Frog includes lastmod (from the server response), priority and
  // changefreq by default.
  last_mod: "server_response",
  priority: true,
  priority_by_depth: [1.0, 0.9, 0.8, 0.7, 0.6, 0.5],
  change_freq: "from_last_modified",
  change_freq_by_depth: ["daily", "daily", "weekly", "monthly", "monthly", "monthly"],
  max_urls_per_file: 49999,
  by_segment: false,
  include_images: false,
  images: { max_inlinks: 10, cdn_regex: "", include_non_200: false, include_noindex: false, max_urls_per_file: 49999 },
  include_hreflang: false,
};

type Section = "pages" | "lastmod" | "priority" | "changefreq" | "images" | "hreflang";

/** Sitemaps > XML Sitemap: choose what to include, then a folder to write to. */
export function SitemapDialog({ onClose, onDone }: { onClose: () => void; onDone: (message: string) => void }) {
  const [opts, setOpts] = useState<SitemapOptions>(DEFAULTS);
  const [section, setSection] = useState<Section>("pages");
  const [lastModMode, setLastModMode] = useState<"none" | "server" | "custom">("server");
  const [customDate, setCustomDate] = useState(new Date().toISOString().slice(0, 10));
  const [error, setError] = useState<string | null>(null);
  const set = <K extends keyof SitemapOptions>(key: K, value: SitemapOptions[K]) => setOpts({ ...opts, [key]: value });
  const setImg = <K extends keyof SitemapOptions["images"]>(key: K, value: SitemapOptions["images"][K]) => setOpts({ ...opts, images: { ...opts.images, [key]: value } });

  const generate = async () => {
    const dir = await open({ directory: true, title: "Choose a folder for the sitemap" });
    if (typeof dir !== "string") return;
    const options: SitemapOptions = {
      ...opts,
      last_mod: lastModMode === "none" ? "none" : lastModMode === "server" ? "server_response" : { custom: customDate },
    };
    try {
      const files = await api.generateSitemap(dir, options);
      onDone(`Wrote ${files.length} sitemap file${files.length === 1 ? "" : "s"} to ${dir}`);
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  const check = (key: keyof SitemapOptions, label: string) => (
    <label key={key} className="check wide">
      <input type="checkbox" checked={Boolean(opts[key])} onChange={(e) => set(key, e.target.checked as never)} /> {label}
    </label>
  );

  const sections: [Section, string][] = [
    ["pages", "Pages"],
    ["lastmod", "Last Modified"],
    ["priority", "Priority"],
    ["changefreq", "Change Frequency"],
    ["images", "Images"],
    ["hreflang", "Hreflang"],
  ];

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide-modal" onClick={(e) => e.stopPropagation()}>
        <h2>XML Sitemap</h2>
        <div className="modal-layout">
          <nav className="modal-nav">
            {sections.map(([id, label]) => (
              <button key={id} className={section === id ? "active" : ""} onClick={() => setSection(id)}>
                {label}
              </button>
            ))}
          </nav>
          <div className="modal-content sched-form">
            {section === "pages" && (
              <>
                {check("include_noindex", "Include noindex pages")}
                {check("include_canonicalised", "Include canonicalised pages")}
                {check("include_paginated", "Include paginated URLs (rel=prev)")}
                {check("include_pdfs", "Include PDFs")}
                {check("by_segment", "One sitemap per segment (with an index)")}
                <fieldset className="config-group">
                  <legend>Response Codes</legend>
                  <label className="check wide">
                    <input type="checkbox" checked disabled /> 2xx (always)
                  </label>
                  {check("include_3xx", "3xx redirects")}
                  {check("include_4xx", "4xx client errors")}
                  {check("include_5xx", "5xx server errors")}
                </fieldset>
                <p className="hint">Only internal, indexable 2xx pages are included by default. Over 49,999 URLs, the sitemap is split into several files with a sitemap index.</p>
              </>
            )}
            {section === "lastmod" && (
              <>
                <label className="stack">
                  Last modified
                  <select value={lastModMode} onChange={(e) => setLastModMode(e.target.value as typeof lastModMode)}>
                    <option value="none">Don't include</option>
                    <option value="server">From server response (Last-Modified header)</option>
                    <option value="custom">Custom date</option>
                  </select>
                </label>
                <label className="stack">
                  Custom date
                  <input type="date" disabled={lastModMode !== "custom"} value={customDate} onChange={(e) => setCustomDate(e.target.value)} />
                </label>
              </>
            )}
            {section === "priority" && (
              <>
                {check("priority", "Include priority")}
                <table className="mapping-table">
                  <tbody>
                    {DEPTHS.map((d, i) => (
                      <tr key={d}>
                        <td>{d}</td>
                        <td>
                          <input
                            type="number"
                            min={0}
                            max={1}
                            step={0.1}
                            disabled={!opts.priority}
                            value={opts.priority_by_depth[i]}
                            onChange={(e) => set("priority_by_depth", opts.priority_by_depth.map((p, j) => (j === i ? Number(e.target.value) : p)))}
                            style={{ width: "6em" }}
                          />
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </>
            )}
            {section === "changefreq" && (
              <>
                <label className="stack">
                  Change frequency
                  <select value={opts.change_freq} onChange={(e) => set("change_freq", e.target.value as SitemapOptions["change_freq"])}>
                    <option value="none">Don't include</option>
                    <option value="from_last_modified">From Last-Modified header (daily if changed in the last day, else monthly)</option>
                    <option value="by_depth">By crawl depth</option>
                  </select>
                </label>
                <table className="mapping-table">
                  <tbody>
                    {DEPTHS.map((d, i) => (
                      <tr key={d}>
                        <td>{d}</td>
                        <td>
                          <select
                            disabled={opts.change_freq !== "by_depth"}
                            value={opts.change_freq_by_depth[i]}
                            onChange={(e) => set("change_freq_by_depth", opts.change_freq_by_depth.map((f, j) => (j === i ? e.target.value : f)))}
                          >
                            {FREQS.map((f) => (
                              <option key={f}>{f}</option>
                            ))}
                          </select>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </>
            )}
            {section === "images" && (
              <>
                {check("include_images", "Include images (image:image entries under each page)")}
                <label className="check wide">
                  <input type="checkbox" disabled={!opts.include_images} checked={opts.images.include_noindex} onChange={(e) => setImg("include_noindex", e.target.checked)} /> Include
                  noindex images
                </label>
                <label className="check wide">
                  <input type="checkbox" disabled={!opts.include_images} checked={opts.images.include_non_200} onChange={(e) => setImg("include_non_200", e.target.checked)} />{" "}
                  Include images that didn't return 200 (or weren't crawled)
                </label>
                <label className="check wide">
                  <input
                    type="checkbox"
                    disabled={!opts.include_images}
                    checked={opts.images.max_inlinks !== null}
                    onChange={(e) => setImg("max_inlinks", e.target.checked ? 10 : null)}
                  />{" "}
                  Only relevant images, linked from at most
                  <input
                    type="number"
                    min={1}
                    disabled={!opts.include_images || opts.images.max_inlinks === null}
                    value={opts.images.max_inlinks ?? 10}
                    onChange={(e) => setImg("max_inlinks", Math.max(1, Number(e.target.value)))}
                    style={{ width: "5em", margin: "0 6px" }}
                  />
                  pages (leaves out logos and icons)
                </label>
                <label className="stack">
                  Include CDN images matching (regex)
                  <input
                    disabled={!opts.include_images}
                    placeholder="e.g. cdn\.example\.com"
                    value={opts.images.cdn_regex}
                    onChange={(e) => setImg("cdn_regex", e.target.value)}
                    spellCheck={false}
                  />
                </label>
              </>
            )}
            {section === "hreflang" && (
              <>
                {check("include_hreflang", "Include hreflang (xhtml:link alternates for each page)")}
                <p className="hint">Each page lists the alternates found in its hreflang annotations.</p>
              </>
            )}
          </div>
        </div>
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <span className="spacer" />
          <button onClick={onClose}>Cancel</button>
          <button className="primary" onClick={generate}>
            Choose folder & generate
          </button>
        </div>
      </div>
    </div>
  );
}
