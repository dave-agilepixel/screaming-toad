import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type SitemapOptions } from "../api";

const DEFAULTS: SitemapOptions = {
  include_noindex: false,
  include_canonicalised: false,
  include_paginated: false,
  include_pdfs: false,
  // Screaming Frog includes lastmod (from the server response), priority and
  // changefreq by default.
  last_mod: "server_response",
  priority: true,
  change_freq: "from_last_modified",
  max_urls_per_file: 49999,
};

/** Sitemaps > XML Sitemap: choose what to include, then a folder to write to. */
export function SitemapDialog({ onClose, onDone }: { onClose: () => void; onDone: (message: string) => void }) {
  const [opts, setOpts] = useState<SitemapOptions>(DEFAULTS);
  const [lastModMode, setLastModMode] = useState<"none" | "server" | "custom">("server");
  const [customDate, setCustomDate] = useState(new Date().toISOString().slice(0, 10));
  const [error, setError] = useState<string | null>(null);
  const set = <K extends keyof SitemapOptions>(key: K, value: SitemapOptions[K]) => setOpts({ ...opts, [key]: value });

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

  const includes: [keyof SitemapOptions, string][] = [
    ["include_noindex", "Include noindex pages"],
    ["include_canonicalised", "Include canonicalised pages"],
    ["include_paginated", "Include paginated URLs (rel=prev)"],
    ["include_pdfs", "Include PDFs"],
  ];

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>XML Sitemap</h2>
        <div className="form">
          <fieldset className="wide toggles">
            {includes.map(([key, label]) => (
              <label key={key} className="check">
                <input type="checkbox" checked={Boolean(opts[key])} onChange={(e) => set(key, e.target.checked as never)} />
                {label}
              </label>
            ))}
          </fieldset>
          <label>
            Last modified
            <select value={lastModMode} onChange={(e) => setLastModMode(e.target.value as typeof lastModMode)}>
              <option value="none">Don't include</option>
              <option value="server">From server response</option>
              <option value="custom">Custom date</option>
            </select>
          </label>
          <label>
            Custom date
            <input type="date" disabled={lastModMode !== "custom"} value={customDate} onChange={(e) => setCustomDate(e.target.value)} />
          </label>
          <label>
            Change frequency
            <select value={opts.change_freq} onChange={(e) => set("change_freq", e.target.value as SitemapOptions["change_freq"])}>
              <option value="none">Don't include</option>
              <option value="from_last_modified">From Last-Modified header</option>
              <option value="by_depth">By crawl depth</option>
            </select>
          </label>
          <label className="check">
            <input type="checkbox" checked={opts.priority} onChange={(e) => set("priority", e.target.checked)} />
            Include priority (by crawl depth)
          </label>
          <p className="hint wide">Only internal, indexable 200 pages are included by default. Over 49,999 URLs, the sitemap is split into several files with a sitemap index.</p>
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
