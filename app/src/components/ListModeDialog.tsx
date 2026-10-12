import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { crawlsApi } from "../api";

/** Mode > List > Upload: Enter Manually / Paste, From a File, Download XML Sitemap. */
export function ListModeDialog({ onClose, onStart }: { onClose: () => void; onStart: (urls: string[]) => Promise<void> }) {
  const [text, setText] = useState("");
  const [sitemaps, setSitemaps] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  // Keep upload order and duplicates; skip blank lines.
  const urls = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean);
  const invalid = urls.filter((u) => !/^https?:\/\/\S+$/i.test(u));
  const unique = new Set(urls).size;

  const add = (more: string[], from: string) => {
    setText((t) => [t.trim(), ...more].filter(Boolean).join("\n"));
    setMessage(`Added ${more.length.toLocaleString()} URL${more.length === 1 ? "" : "s"} from ${from}.`);
  };

  const fromFile = async () => {
    const path = await open({ filters: [{ name: "URL lists", extensions: ["txt", "csv", "xml", "gz"] }] });
    if (typeof path !== "string") return;
    try {
      add(await crawlsApi.readUrlFile(path), path);
    } catch (e) {
      setMessage(String(e));
    }
  };

  const download = async () => {
    const list = sitemaps.split(/\s+/).filter(Boolean);
    if (!list.length) return;
    setBusy(true);
    try {
      add(await crawlsApi.fetchSitemapUrls(list), list.length === 1 ? list[0] : `${list.length} sitemaps`);
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  };

  const start = async () => {
    setBusy(true);
    try {
      await onStart(urls.filter((u) => !invalid.includes(u)));
      onClose();
    } catch (e) {
      setMessage(String(e));
      setBusy(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide-modal" onClick={(e) => e.stopPropagation()}>
        <h2>List Mode: URLs to crawl</h2>
        <div className="form">
          <label className="wide stack">
            Enter or paste URLs, one per line
            <textarea rows={14} value={text} onChange={(e) => setText(e.target.value)} placeholder="https://www.example.com/page-1" spellCheck={false} />
          </label>
          <div className="wide button-row">
            <button onClick={fromFile}>From a File…</button>
            <input
              className="grow"
              placeholder="XML sitemap URL(s), e.g. https://www.example.com/sitemap.xml"
              value={sitemaps}
              onChange={(e) => setSitemaps(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && download()}
              spellCheck={false}
            />
            <button onClick={download} disabled={busy || !sitemaps.trim()}>
              Download XML Sitemap
            </button>
            <button onClick={() => setText("")} disabled={!text}>
              Clear
            </button>
          </div>
          <p className="hint wide">
            {urls.length.toLocaleString()} URLs ({unique.toLocaleString()} unique)
            {invalid.length > 0 && <span className="error"> · {invalid.length} not valid http(s) URLs will be skipped</span>}. List mode crawls depth 0 and
            ignores robots.txt by default; Reports &gt; List Mode Export keeps this order and any duplicates.
          </p>
          {message && <p className="hint wide">{message}</p>}
        </div>
        <div className="modal-actions">
          <span className="spacer" />
          <button onClick={onClose}>Cancel</button>
          <button className="primary" onClick={start} disabled={busy || urls.length === invalid.length}>
            Start
          </button>
        </div>
      </div>
    </div>
  );
}
