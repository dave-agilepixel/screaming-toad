import { useEffect, useState } from "react";
import { CHANGE_ELEMENTS, CHANGE_ELEMENT_NAMES, compareApi, type CompareConfig, type UrlMapping } from "../api";

type Section = "change" | "mapping";
type MappingTab = "regex" | "test";

/** Config > Compare: "Crawl Comparison Configuration". */
export function CompareConfigDialog({ onClose }: { onClose: (saved: boolean) => void }) {
  const [cfg, setCfg] = useState<CompareConfig | null>(null);
  const [section, setSection] = useState<Section>("change");
  const [mappingTab, setMappingTab] = useState<MappingTab>("regex");
  const [testUrl, setTestUrl] = useState("");
  const [testResult, setTestResult] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    compareApi.getConfig().then(setCfg).catch(console.error);
  }, []);

  useEffect(() => {
    if (!cfg || !testUrl.trim()) {
      setTestResult("");
      return;
    }
    compareApi
      .testMapping(cfg.url_mapping, testUrl.trim())
      .then((r) => setTestResult(r))
      .catch((e) => setTestResult(String(e)));
  }, [cfg, testUrl]);

  if (!cfg) return null;
  const set = <K extends keyof CompareConfig>(key: K, value: CompareConfig[K]) => setCfg({ ...cfg, [key]: value });
  const allOn = CHANGE_ELEMENTS.every((e) => cfg.elements.includes(e));
  const updateRule = (i: number, patch: Partial<UrlMapping>) =>
    set("url_mapping", cfg.url_mapping.map((r, j) => (j === i ? { ...r, ...patch } : r)));

  const save = async () => {
    try {
      await compareApi.setConfig({ ...cfg, url_mapping: cfg.url_mapping.filter((r) => r.regex.trim() !== "") });
      onClose(true);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="modal-backdrop" onClick={() => onClose(false)}>
      <div className="modal wide-modal" onClick={(e) => e.stopPropagation()}>
        <h2>Crawl Comparison Configuration</h2>
        <div className="modal-layout">
          <nav className="modal-nav">
            <button className={section === "change" ? "active" : ""} onClick={() => setSection("change")}>
              Change Detection
            </button>
            <button className={section === "mapping" ? "active" : ""} onClick={() => setSection("mapping")}>
              URL Mapping
            </button>
          </nav>
          <div className="modal-content">
            {section === "change" && (
              <div className="form">
                <p className="hint wide">
                  Choose the elements compared for pages that are in both crawls. Each one becomes a filter in the Change Detection tab.
                </p>
                <label className="check wide">
                  <input
                    type="checkbox"
                    checked={allOn}
                    onChange={(e) => set("elements", e.target.checked ? [...CHANGE_ELEMENTS] : [])}
                  />
                  Select All
                </label>
                <div className="toggles wide">
                  {CHANGE_ELEMENTS.map((el) => (
                    <label key={el} className="check">
                      <input
                        type="checkbox"
                        checked={cfg.elements.includes(el)}
                        onChange={(e) =>
                          set("elements", e.target.checked ? CHANGE_ELEMENTS.filter((x) => x === el || cfg.elements.includes(x)) : cfg.elements.filter((x) => x !== el))
                        }
                      />
                      {CHANGE_ELEMENT_NAMES[el]}
                    </label>
                  ))}
                </div>
                <label>
                  Content: Similarity Change %
                  <input
                    type="number"
                    min={0}
                    max={100}
                    value={cfg.similarity_change_pct}
                    disabled={!cfg.elements.includes("Content")}
                    onChange={(e) => set("similarity_change_pct", Math.min(100, Math.max(0, Number(e.target.value))))}
                  />
                </label>
                <p className="hint">
                  Content compares a MinHash of each page's content area, so it needs Store HTML (or Near Duplicates) switched on in both crawls.
                </p>
                <label className="check wide">
                  <input type="checkbox" checked={cfg.ignore_non_indexable} onChange={(e) => set("ignore_non_indexable", e.target.checked)} />
                  Ignore Non-Indexable URLs for Change Detection
                </label>
              </div>
            )}

            {section === "mapping" && (
              <div>
                <div className="detail-tabs subtabs">
                  <button className={mappingTab === "regex" ? "active" : ""} onClick={() => setMappingTab("regex")}>
                    Regex Replace
                  </button>
                  <button className={mappingTab === "test" ? "active" : ""} onClick={() => setMappingTab("test")}>
                    Test
                  </button>
                </div>
                {mappingTab === "regex" && (
                  <div className="rules">
                    <p className="hint">
                      Enter one or more regular expressions to map the previous crawl URL into the current crawl URL. Rules run in order;
                      use $1 for capture groups. Examples: <code>staging\.example\.com</code> → <code>www.example.com</code>, or{" "}
                      <code>/$</code> → (empty) to drop trailing slashes.
                    </p>
                    <table className="mapping-table">
                      <thead>
                        <tr>
                          <th>Regex</th>
                          <th>Replace</th>
                          <th></th>
                        </tr>
                      </thead>
                      <tbody>
                        {cfg.url_mapping.map((r, i) => (
                          <tr key={i}>
                            <td>
                              <input value={r.regex} onChange={(e) => updateRule(i, { regex: e.target.value })} spellCheck={false} />
                            </td>
                            <td>
                              <input value={r.replace} onChange={(e) => updateRule(i, { replace: e.target.value })} spellCheck={false} />
                            </td>
                            <td>
                              <button onClick={() => set("url_mapping", cfg.url_mapping.filter((_, j) => j !== i))}>Delete</button>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                    <button onClick={() => set("url_mapping", [...cfg.url_mapping, { regex: "", replace: "" }])}>Add</button>
                  </div>
                )}
                {mappingTab === "test" && (
                  <div className="form">
                    <label className="wide">
                      Previous crawl URL
                      <input value={testUrl} onChange={(e) => setTestUrl(e.target.value)} placeholder="https://staging.example.com/page/" spellCheck={false} />
                    </label>
                    <label className="wide">
                      Mapped URL
                      <input value={testResult} readOnly />
                    </label>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <button
            onClick={() =>
              setCfg({ elements: [...CHANGE_ELEMENTS], similarity_change_pct: 10, ignore_non_indexable: false, url_mapping: [] })
            }
          >
            Reset to defaults
          </button>
          <span className="spacer" />
          <button onClick={() => onClose(false)}>Cancel</button>
          <button className="primary" onClick={save}>
            OK
          </button>
        </div>
      </div>
    </div>
  );
}
