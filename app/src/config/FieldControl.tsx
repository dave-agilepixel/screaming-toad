import { useEffect, useState, type ReactNode } from "react";
import { api, type UrlTest } from "../api";
import { AuthenticationPanel } from "../components/AuthenticationPanel";
import { SegmentsEditor } from "../components/SegmentsEditor";
import { AiPromptsEditor, ApiKeyField, EmbeddingModelField, GoogleAccountField, GooglePropertyField } from "../components/ApiFields";
import { HEADER_NAMES, USER_AGENT_PRESETS, WINDOW_PRESETS, validHeaderName } from "./presets";
import type { RobotsTest } from "../api";
import { getPath, type CrawlConfig, type CustomExtractor, type CustomJavaScript, type CustomSearch, type Field } from "./schema";

interface Props {
  field: Field;
  cfg: CrawlConfig;
  defaults: CrawlConfig | null;
  set: (key: string, value: unknown) => void;
  /** Search text to highlight in labels. */
  highlight: string;
}

const lines = (v: string) =>
  v
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);

function Label({ text, highlight }: { text: string; highlight: string }) {
  if (!highlight) return <>{text}</>;
  const i = text.toLowerCase().indexOf(highlight.toLowerCase());
  if (i < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, i)}
      <mark>{text.slice(i, i + highlight.length)}</mark>
      {text.slice(i + highlight.length)}
    </>
  );
}

/** Marks a control whose value differs from the default. */
function Changed({ cfg, defaults, keys }: { cfg: CrawlConfig; defaults: CrawlConfig | null; keys: string[] }) {
  if (!defaults) return null;
  const changed = keys.some((k) => JSON.stringify(getPath(cfg, k)) !== JSON.stringify(getPath(defaults, k)));
  return changed ? <span className="changed-dot" title="Changed from the default" /> : null;
}

function Help({ text }: { text?: string }) {
  return text ? <p className="hint">{text}</p> : null;
}

/** Renders one schema field. */
export function FieldControl({ field: f, cfg, defaults, set, highlight }: Props): ReactNode {
  switch (f.kind) {
    case "note":
      return <p className="hint note">{f.text}</p>;

    case "bool": {
      const disabled = f.disabledIf?.(cfg) ?? false;
      return (
        <div className="field">
          <label className={`check${disabled ? " disabled" : ""}`} title={f.pending}>
            <input type="checkbox" checked={Boolean(getPath(cfg, f.key))} disabled={disabled} onChange={(e) => set(f.key, e.target.checked)} />
            <Label text={f.label} highlight={highlight} />
            <Changed cfg={cfg} defaults={defaults} keys={[f.key]} />
          </label>
          <Help text={f.help} />
        </div>
      );
    }

    case "pair": {
      const crawl = f.crawl ? Boolean(getPath(cfg, f.crawl)) : false;
      const store = f.store ? Boolean(getPath(cfg, f.store)) : false;
      return (
        <div className="field">
          <div className="pair-row">
            <span className="pair-label">
              <Label text={f.label} highlight={highlight} />
              <Changed cfg={cfg} defaults={defaults} keys={[...(f.crawl ? [f.crawl] : []), ...(f.store ? [f.store] : [])]} />
            </span>
            <label className="check" title={crawl ? "Crawl implies Store" : "Record the link and report the URL, without requesting it"}>
              <input type="checkbox" checked={store || crawl} disabled={!f.store || crawl} onChange={(e) => f.store && set(f.store, e.target.checked)} />
              Store
            </label>
            {f.storeOnly ? (
              <span />
            ) : (
              <label className="check">
                <input type="checkbox" checked={crawl} onChange={(e) => f.crawl && set(f.crawl, e.target.checked)} />
                Crawl
              </label>
            )}
          </div>
          <Help text={f.help} />
        </div>
      );
    }

    case "number": {
      const disabled = f.disabledIf?.(cfg) ?? false;
      const value = getPath(cfg, f.key) as number;
      return (
        <div className="field">
          <label className="row">
            <span>
              <Label text={f.label} highlight={highlight} />
              <Changed cfg={cfg} defaults={defaults} keys={[f.key]} />
            </span>
            <span className="input-unit">
              <input
                type="number"
                min={f.min}
                max={f.max}
                step={f.step ?? 1}
                value={value}
                disabled={disabled}
                onChange={(e) => e.target.value !== "" && set(f.key, Number(e.target.value))}
              />
              {f.unit && <span className="unit">{f.unit}</span>}
            </span>
          </label>
          <Help text={f.help} />
        </div>
      );
    }

    case "limit": {
      const value = getPath(cfg, f.key) as number | null;
      const on = value !== null && value !== undefined;
      return (
        <div className="field">
          <label className="row">
            <span className="check">
              <input type="checkbox" checked={on} onChange={(e) => set(f.key, e.target.checked ? f.whenOn : null)} />
              <Label text={f.label} highlight={highlight} />
              <Changed cfg={cfg} defaults={defaults} keys={[f.key]} />
            </span>
            <span className="input-unit">
              <input
                type="number"
                min={f.min}
                step={f.step ?? 1}
                disabled={!on}
                value={on ? value : f.whenOn}
                onChange={(e) => e.target.value !== "" && set(f.key, Number(e.target.value))}
              />
              {f.unit && <span className="unit">{f.unit}</span>}
            </span>
          </label>
          <Help text={f.help} />
        </div>
      );
    }

    case "percent": {
      const disabled = f.disabledIf?.(cfg) ?? false;
      const value = getPath(cfg, f.key) as number;
      return (
        <div className="field">
          <label className="row">
            <span>
              <Label text={f.label} highlight={highlight} />
              <Changed cfg={cfg} defaults={defaults} keys={[f.key]} />
            </span>
            <span className="input-unit">
              <input
                type="number"
                min={f.min ?? 0}
                max={100}
                disabled={disabled}
                value={Math.round(value * 100)}
                onChange={(e) => e.target.value !== "" && set(f.key, Math.min(100, Math.max(0, Number(e.target.value))) / 100)}
              />
              <span className="unit">%</span>
            </span>
          </label>
          <Help text={f.help} />
        </div>
      );
    }

    case "text":
      return (
        <div className="field">
          <label className="stack">
            <span>
              <Label text={f.label} highlight={highlight} />
              <Changed cfg={cfg} defaults={defaults} keys={[f.key]} />
            </span>
            <input value={String(getPath(cfg, f.key) ?? "")} placeholder={f.placeholder} onChange={(e) => set(f.key, e.target.value)} spellCheck={false} />
          </label>
          <Help text={f.help} />
        </div>
      );

    case "select":
      return (
        <div className="field">
          <label className="row">
            <span>
              <Label text={f.label} highlight={highlight} />
              <Changed cfg={cfg} defaults={defaults} keys={[f.key]} />
            </span>
            <select value={String(getPath(cfg, f.key))} onChange={(e) => set(f.key, e.target.value)}>
              {f.options.map(([v, l]) => (
                <option key={v} value={v}>
                  {l}
                </option>
              ))}
            </select>
          </label>
          <Help text={f.help} />
        </div>
      );

    case "lines": {
      const disabled = f.disabledIf?.(cfg) ?? false;
      return <LinesField key={f.key} field={f} cfg={cfg} defaults={defaults} set={set} highlight={highlight} disabled={disabled} />;
    }

    case "headers":
      return <HeadersField cfg={cfg} defaults={defaults} set={set} help={f.help} />;

    case "path-limits": {
      const rows = (getPath(cfg, f.key) as { pattern: string; max: number }[]) ?? [];
      const update = (i: number, patch: Partial<{ pattern: string; max: number }>) => set(f.key, rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
      return (
        <div className="field">
          <span>
            <Label text={f.label} highlight={highlight} />
            <Changed cfg={cfg} defaults={defaults} keys={[f.key]} />
          </span>
          {rows.length === 0 ? (
            <p className="hint">No URL path limits configured.</p>
          ) : (
            <table className="mapping-table">
              <thead>
                <tr>
                  <th>URL path regex</th>
                  <th>Max URLs</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {rows.map((r, i) => (
                  <tr key={i}>
                    <td>
                      <input value={r.pattern} onChange={(e) => update(i, { pattern: e.target.value })} spellCheck={false} />
                    </td>
                    <td>
                      <input type="number" min={0} value={r.max} onChange={(e) => update(i, { max: Math.max(0, Number(e.target.value)) })} />
                    </td>
                    <td>
                      <button onClick={() => set(f.key, rows.filter((_, j) => j !== i))}>Delete</button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <div className="button-row">
            <button onClick={() => set(f.key, [...rows, { pattern: "", max: 1000 }])}>Add</button>
            <button onClick={() => set(f.key, [])} disabled={rows.length === 0}>
              Delete All
            </button>
          </div>
          <Help text={f.help} />
        </div>
      );
    }

    case "rewrite-rules": {
      const rows = (getPath(cfg, f.key) as { regex: string; replace: string }[]) ?? [];
      const update = (i: number, patch: Partial<{ regex: string; replace: string }>) => set(f.key, rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
      const move = (i: number, by: number) => {
        const j = i + by;
        if (j < 0 || j >= rows.length) return;
        const next = [...rows];
        [next[i], next[j]] = [next[j], next[i]];
        set(f.key, next);
      };
      return (
        <div className="field">
          <span>
            <Label text={f.label} highlight={highlight} />
            <Changed cfg={cfg} defaults={defaults} keys={[f.key]} />
          </span>
          {rows.length > 0 && (
            <table className="mapping-table">
              <thead>
                <tr>
                  <th>Regex</th>
                  <th>Replace</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {rows.map((r, i) => (
                  <tr key={i}>
                    <td>
                      <input value={r.regex} onChange={(e) => update(i, { regex: e.target.value })} spellCheck={false} />
                    </td>
                    <td>
                      <input value={r.replace} onChange={(e) => update(i, { replace: e.target.value })} spellCheck={false} />
                    </td>
                    <td className="rule-buttons">
                      <button onClick={() => move(i, -1)} disabled={i === 0} title="Move up">
                        ↑
                      </button>
                      <button onClick={() => move(i, 1)} disabled={i === rows.length - 1} title="Move down">
                        ↓
                      </button>
                      <button onClick={() => set(f.key, rows.filter((_, j) => j !== i))}>Delete</button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <div className="button-row">
            <button onClick={() => set(f.key, [...rows, { regex: "", replace: "" }])}>Add</button>
          </div>
          <Help text={f.help} />
        </div>
      );
    }

    case "custom-robots":
      return <CustomRobotsField cfg={cfg} set={set} fieldKey={f.key} label={f.label} help={f.help} highlight={highlight} />;

    case "ua-preset": {
      const current = USER_AGENT_PRESETS.find((p) => p.userAgent === cfg.user_agent && p.robots === cfg.robots_user_agent);
      const isDefault = defaults && cfg.user_agent === defaults.user_agent && cfg.robots_user_agent === defaults.robots_user_agent;
      return (
        <div className="field">
          <label className="row">
            <Label text={f.label} highlight={highlight} />
            <select
              value={isDefault ? "__default__" : (current?.name ?? "__custom__")}
              onChange={(e) => {
                const v = e.target.value;
                if (v === "__default__" && defaults) {
                  set("user_agent", defaults.user_agent);
                  set("robots_user_agent", defaults.robots_user_agent);
                  return;
                }
                const p = USER_AGENT_PRESETS.find((x) => x.name === v);
                if (p) {
                  set("user_agent", p.userAgent);
                  set("robots_user_agent", p.robots);
                }
              }}
            >
              <option value="__default__">Screaming Toad SEO Spider</option>
              {USER_AGENT_PRESETS.map((p) => (
                <option key={p.name} value={p.name}>
                  {p.name}
                </option>
              ))}
              {!isDefault && !current && <option value="__custom__">Custom</option>}
            </select>
          </label>
          <p className="hint">A preset fills both fields below. Use Chrome for sites that block bots (403, 406, 429); Googlebot for cookie walls.</p>
        </div>
      );
    }

    case "window-preset": {
      const r = cfg.rendering;
      const current = WINDOW_PRESETS.find(
        (p) => p.width === r.width && p.height === r.height && p.scaling_factor === r.scaling_factor && p.mobile === r.mobile && p.touch === r.touch,
      );
      return (
        <div className="field">
          <label className="row">
            <span>
              <Label text={f.label} highlight={highlight} />
              <Changed cfg={cfg} defaults={defaults} keys={["rendering.width", "rendering.height", "rendering.scaling_factor", "rendering.mobile", "rendering.touch"]} />
            </span>
            <select
              disabled={f.disabledIf?.(cfg)}
              value={current?.name ?? "__custom__"}
              onChange={(e) => {
                const p = WINDOW_PRESETS.find((x) => x.name === e.target.value);
                if (!p) return;
                set("rendering.window_preset", p.name);
                set("rendering.width", p.width);
                set("rendering.height", p.height);
                set("rendering.scaling_factor", p.scaling_factor);
                set("rendering.mobile", p.mobile);
                set("rendering.touch", p.touch);
              }}
            >
              {WINDOW_PRESETS.map((p) => (
                <option key={p.name} value={p.name}>
                  {p.name} ({p.width}×{p.height})
                </option>
              ))}
              {!current && <option value="__custom__">Custom</option>}
            </select>
          </label>
        </div>
      );
    }

    case "url-test":
      return <UrlTestBox cfg={cfg} show={f.show} label={f.label} />;

    case "spelling-language":
      return <SpellingLanguage cfg={cfg} set={set} fieldKey={f.key} label={f.label} help={f.help} />;

    case "global-dictionary":
      return <GlobalDictionary help={f.help} />;

    case "link-positions":
      return <LinkPositionsField cfg={cfg} set={set} />;

    case "custom-search":
      return <SearchEditor rules={cfg.custom_search} onChange={(v) => set("custom_search", v)} />;

    case "custom-extraction":
      return <ExtractionEditor rules={cfg.custom_extraction} onChange={(v) => set("custom_extraction", v)} />;

    case "authentication":
      return <AuthenticationPanel />;

    case "api-key":
      return <ApiKeyField secret={f.secret} label={f.label} help={f.help} placeholder={f.placeholder} />;

    case "google-account":
      return <GoogleAccountField />;

    case "google-property":
      return <GooglePropertyField which={f.which} label={f.label} value={String(getPath(cfg, f.key) ?? "")} onChange={(v) => set(f.key, v)} />;

    case "ai-prompts":
      return <AiPromptsEditor cfg={cfg} set={set} />;

    case "embedding-model":
      return <EmbeddingModelField cfg={cfg} set={set} label={f.label} />;

    case "segments":
      return <SegmentsEditor segments={cfg.segments} onChange={(v) => set("segments", v)} />;

    case "custom-javascript":
      return <JavaScriptEditor rules={cfg.custom_javascript} jsMode={cfg.rendering.mode === "javascript"} onChange={(v) => set("custom_javascript", v)} />;
  }
}

function SpellingLanguage({ cfg, set, fieldKey, label, help }: { cfg: CrawlConfig; set: (k: string, v: unknown) => void; fieldKey: string; label: string; help?: string }) {
  const [langs, setLangs] = useState<[string, string][]>([]);
  useEffect(() => {
    api.spellingLanguages().then(setLangs).catch(console.error);
  }, []);
  return (
    <div className="field">
      <label className="row">
        {label}
        <select value={String(getPath(cfg, fieldKey))} onChange={(e) => set(fieldKey, e.target.value)}>
          {langs.map(([code, name]) => (
            <option key={code} value={code}>
              {name} ({code})
            </option>
          ))}
        </select>
      </label>
      <Help text={help} />
    </div>
  );
}

function GlobalDictionary({ help }: { help?: string }) {
  const [text, setText] = useState<string | null>(null);
  const [saved, setSaved] = useState(true);
  useEffect(() => {
    api.getDictionaryWords().then((w) => setText(w.join("\n"))).catch(console.error);
  }, []);
  if (text === null) return null;
  const save = () => api.setDictionaryWords(text.split(/\r?\n/)).then(() => setSaved(true));
  return (
    <div className="field">
      <textarea rows={5} value={text} onChange={(e) => (setText(e.target.value), setSaved(false))} onBlur={save} spellCheck={false} />
      <p className="hint">
        {saved ? "Saved." : "Unsaved: click outside the box to save."} {help}
      </p>
    </div>
  );
}

/** robots.txt > Custom Robots: subdomain list, editor (Download / Clear) and URL tester. */
function CustomRobotsField({
  cfg,
  set,
  fieldKey,
  label,
  help,
  highlight,
}: {
  cfg: CrawlConfig;
  set: (key: string, value: unknown) => void;
  fieldKey: string;
  label: string;
  help?: string;
  highlight: string;
}) {
  const rows = (getPath(cfg, fieldKey) as { host: string; enabled: boolean; txt: string }[]) ?? [];
  const [selected, setSelected] = useState(0);
  const [testUrl, setTestUrl] = useState("");
  const [result, setResult] = useState<RobotsTest | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const row = rows[selected];
  const update = (patch: Partial<{ host: string; enabled: boolean; txt: string }>) =>
    set(fieldKey, rows.map((r, j) => (j === selected ? { ...r, ...patch } : r)));

  useEffect(() => {
    if (!row || !testUrl.trim()) {
      setResult(null);
      return;
    }
    const t = setTimeout(() => {
      api
        .testRobots(row.txt, cfg.robots_user_agent, testUrl.trim())
        .then((r) => {
          setResult(r);
          setMessage(null);
        })
        .catch((e) => {
          setResult(null);
          setMessage(String(e));
        });
    }, 200);
    return () => clearTimeout(t);
  }, [row, testUrl, cfg.robots_user_agent]);

  const download = async () => {
    if (!row?.host.trim()) return;
    try {
      update({ txt: await api.downloadRobots(row.host) });
      setMessage(`Downloaded robots.txt for ${row.host}.`);
    } catch (e) {
      setMessage(String(e));
    }
  };

  const lines = (row?.txt ?? "").split(/\r?\n/);
  const matchedNo = result?.line ? Number(/^Line (\d+):/.exec(result.line)?.[1] ?? 0) : 0;

  return (
    <div className="field">
      <span>
        <Label text={label} highlight={highlight} />
      </span>
      <div className="robots-layout">
        <div className="robots-list">
          {rows.map((r, i) => (
            <div key={i} className={`robots-row${i === selected ? " selected" : ""}`} onClick={() => setSelected(i)}>
              <input type="checkbox" checked={r.enabled} title="Override Enabled" onChange={(e) => set(fieldKey, rows.map((x, j) => (j === i ? { ...x, enabled: e.target.checked } : x)))} />
              <span>{r.host || "(new subdomain)"}</span>
            </div>
          ))}
          <div className="button-row">
            <button
              onClick={() => {
                set(fieldKey, [...rows, { host: "", enabled: true, txt: "User-agent: *\nDisallow:\n" }]);
                setSelected(rows.length);
              }}
            >
              Add
            </button>
            <button
              disabled={!row}
              onClick={() => {
                set(fieldKey, rows.filter((_, j) => j !== selected));
                setSelected(Math.max(0, selected - 1));
              }}
            >
              Delete
            </button>
          </div>
        </div>
        {row ? (
          <div className="robots-editor">
            <label className="stack">
              Subdomain
              <input value={row.host} onChange={(e) => update({ host: e.target.value })} placeholder="www.example.com" spellCheck={false} />
            </label>
            <div className="button-row">
              <button onClick={download} disabled={!row.host.trim()}>
                Download robots.txt
              </button>
              <button onClick={() => update({ txt: "" })}>Clear</button>
              <label className="check">
                <input type="checkbox" checked={row.enabled} onChange={(e) => update({ enabled: e.target.checked })} /> Override Enabled
              </label>
            </div>
            <div className="robots-text">
              <pre className="robots-gutter" aria-hidden>
                {lines.map((_, i) => (
                  <div key={i} className={i + 1 === matchedNo ? (result?.allowed ? "hit-allow" : "hit-block") : ""}>
                    {i + 1}
                  </div>
                ))}
              </pre>
              <textarea rows={10} value={row.txt} onChange={(e) => update({ txt: e.target.value })} spellCheck={false} wrap="off" />
            </div>
            <label className="stack">
              Test URL (uses the Robots User-Agent: {cfg.robots_user_agent || "* only"})
              <input value={testUrl} onChange={(e) => setTestUrl(e.target.value)} placeholder="https://www.example.com/private/page or /private/page" spellCheck={false} />
            </label>
            {result && (
              <p className={`verdict ${result.allowed ? "good" : "bad"}`}>
                {result.allowed ? "Allowed" : "Blocked"}
                {result.line ? ` by ${result.line}` : " (no rule matches)"}
                {result.group ? ` in group "${result.group}"` : ""}
              </p>
            )}
          </div>
        ) : (
          <p className="hint">No custom robots.txt. Add a subdomain to override its live file.</p>
        )}
      </div>
      {message && <p className="hint">{message}</p>}
      <Help text={help} />
    </div>
  );
}

/** Live Test pane: rewrites and classifies a URL under the dialog's (unsaved) configuration. */
function UrlTestBox({ cfg, show, label }: { cfg: CrawlConfig; show: "rewrite" | "cdn" | "include" | "exclude"; label: string }) {
  const [url, setUrl] = useState("");
  const [start, setStart] = useState("");
  const [result, setResult] = useState<UrlTest | null>(null);

  useEffect(() => {
    if (!url.trim()) {
      setResult(null);
      return;
    }
    const t = setTimeout(() => {
      api
        .testUrl(cfg, show === "cdn" ? start.trim() || null : null, url.trim())
        .then(setResult)
        .catch((e) => setResult({ rewritten: "", encoded: "", internal: null, include_match: null, included: false, exclude_match: null, error: String(e) }));
    }, 200);
    return () => clearTimeout(t);
  }, [cfg, url, start, show]);

  return (
    <fieldset className="url-test">
      <legend>{label}</legend>
      {show === "cdn" && (
        <label className="stack">
          Start URL
          <input value={start} onChange={(e) => setStart(e.target.value)} placeholder="https://www.example.com/" spellCheck={false} />
        </label>
      )}
      <label className="stack">
        {show === "rewrite" ? "URL before rewriting" : "Test URL"}
        <input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://www.example.com/page?id=1" spellCheck={false} />
      </label>
      {result?.error && <p className="error">{result.error}</p>}
      {result && !result.error && (
        <div className="url-test-result">
          {show === "rewrite" && (
            <label className="stack">
              URL after rewriting
              <input value={result.rewritten} readOnly className={result.rewritten !== url.trim() ? "changed-value" : ""} />
            </label>
          )}
          {show === "cdn" &&
            (result.internal === null ? (
              <p className="hint">Enter a start URL to see whether the test URL is Internal or External.</p>
            ) : (
              <p className={`verdict ${result.internal ? "good" : ""}`}>{result.internal ? "Internal" : "External"}</p>
            ))}
          {show === "include" && (
            <p className={`verdict ${result.included ? "good" : "bad"}`}>
              {result.include_match ? `Included: matches ${result.include_match}` : result.included ? "Included (the Include list is empty)" : "Not included: no pattern matches"}
            </p>
          )}
          {show === "exclude" && (
            <p className={`verdict ${result.exclude_match ? "bad" : "good"}`}>{result.exclude_match ? `Excluded: matches ${result.exclude_match}` : "Not excluded"}</p>
          )}
          {(show === "include" || show === "exclude") && <p className="hint">Matched against the encoded, rewritten URL: {result.rewritten}</p>}
        </div>
      )}
    </fieldset>
  );
}

/** A string list edited one item per line. Keeps the raw text while typing,
 *  so blank lines don't vanish under the cursor. */
function LinesField({
  field: f,
  cfg,
  defaults,
  set,
  highlight,
  disabled,
}: {
  field: Extract<Field, { kind: "lines" }>;
  cfg: CrawlConfig;
  defaults: CrawlConfig | null;
  set: (key: string, value: unknown) => void;
  highlight: string;
  disabled: boolean;
}) {
  const value = (getPath(cfg, f.key) as string[]) ?? [];
  return (
    <div className="field">
      <label className="stack">
        <span>
          <Label text={f.label} highlight={highlight} />
          <Changed cfg={cfg} defaults={defaults} keys={[f.key]} />
        </span>
        <textarea
          rows={f.rows ?? 4}
          placeholder={f.placeholder}
          disabled={disabled}
          defaultValue={value.join("\n")}
          onBlur={(e) => set(f.key, lines(e.target.value))}
          onChange={(e) => set(f.key, lines(e.target.value))}
          spellCheck={false}
        />
      </label>
      <Help text={f.help} />
    </div>
  );
}

function HeadersField({
  cfg,
  defaults,
  set,
  help,
}: {
  cfg: CrawlConfig;
  defaults: CrawlConfig | null;
  set: (key: string, value: unknown) => void;
  help?: string;
}) {
  const rows = cfg.custom_headers;
  const update = (i: number, part: 0 | 1, v: string) =>
    set(
      "custom_headers",
      rows.map((r, j) => (j === i ? ((part === 0 ? [v, r[1]] : [r[0], v]) as [string, string]) : r)),
    );
  return (
    <div className="field">
      <table className="mapping-table headers-table">
        <thead>
          <tr>
            <th>Header Name</th>
            <th>Header Value</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr className="muted">
            <td>User-Agent</td>
            <td title="Set in Configuration > User-Agent">{cfg.user_agent}</td>
            <td></td>
          </tr>
          {rows.map(([name, value], i) => (
            <tr key={i}>
              <td className="header-name">
                <input list="toad-header-names" value={name} onChange={(e) => update(i, 0, e.target.value)} spellCheck={false} />
                <span className={validHeaderName(name) ? "valid-mark" : "invalid-mark"} title={validHeaderName(name) ? "Valid header name" : "Not a valid header name"}>
                  {validHeaderName(name) ? "✓" : "✗"}
                </span>
              </td>
              <td>
                <input value={value} onChange={(e) => update(i, 1, e.target.value)} spellCheck={false} />
              </td>
              <td>
                <button onClick={() => set("custom_headers", rows.filter((_, j) => j !== i))}>Delete</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <datalist id="toad-header-names">
        {HEADER_NAMES.map((h) => (
          <option key={h} value={h} />
        ))}
      </datalist>
      <div className="button-row">
        <button onClick={() => set("custom_headers", [...rows, ["", ""]])}>Add</button>
        <button disabled={!defaults} onClick={() => defaults && set("custom_headers", defaults.custom_headers)}>
          Reset to Default
        </button>
      </div>
      <Help text={help} />
    </div>
  );
}

const MAX_RULES = 100;

const newSearch = (n: number): CustomSearch => ({
  name: `Search ${n}`,
  mode: "contains",
  kind: "text",
  query: "",
  case_sensitive: false,
  search_in: "html",
  xpath: "",
});

const newExtractor = (n: number): CustomExtractor => ({
  name: `Extractor ${n}`,
  kind: "xpath",
  expression: "",
  attribute: null,
  extract: "inner_html",
});

function move<T>(list: T[], i: number, by: number): T[] {
  const j = i + by;
  if (j < 0 || j >= list.length) return list;
  const next = [...list];
  [next[i], next[j]] = [next[j], next[i]];
  return next;
}

/** ✓ / ✗ for an expression, checked by the backend as the user types. */
function Validity({ kind, expression }: { kind: string; expression: string }) {
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!expression.trim()) {
      setError(null);
      return;
    }
    const t = setTimeout(() => api.checkExpression(kind, expression).then(setError).catch(() => setError(null)), 250);
    return () => clearTimeout(t);
  }, [kind, expression]);
  if (!expression.trim()) return <span className="valid-mark muted">·</span>;
  return error ? (
    <span className="invalid-mark" title={error}>
      ✗
    </span>
  ) : (
    <span className="valid-mark" title="Valid">
      ✓
    </span>
  );
}

function RowButtons<T>({ rules, i, onChange, copy }: { rules: T[]; i: number; onChange: (v: T[]) => void; copy: (r: T) => T }) {
  return (
    <span className="rule-buttons">
      <button onClick={() => onChange(move(rules, i, -1))} disabled={i === 0} title="Move up">
        ↑
      </button>
      <button onClick={() => onChange(move(rules, i, 1))} disabled={i === rules.length - 1} title="Move down">
        ↓
      </button>
      <button onClick={() => onChange([...rules.slice(0, i + 1), copy(rules[i]), ...rules.slice(i + 1)])} disabled={rules.length >= MAX_RULES} title="Duplicate">
        ⧉
      </button>
      <button onClick={() => onChange(rules.filter((_, j) => j !== i))} title="Delete">
        ✕
      </button>
    </span>
  );
}

function SearchEditor({ rules, onChange }: { rules: CustomSearch[]; onChange: (v: CustomSearch[]) => void }) {
  const [expanded, setExpanded] = useState<number | null>(null);
  const [bulk, setBulk] = useState(false);
  const update = (i: number, patch: Partial<CustomSearch>) => onChange(rules.map((s, j) => (j === i ? { ...s, ...patch } : s)));
  return (
    <div className="rules">
      <p className="hint">Search the source code or content of internal HTML pages. Each search is a filter in the Custom Search tab.</p>
      {rules.map((s, i) => (
        <div key={i} className="rule">
          <input placeholder="Name" value={s.name} onChange={(e) => update(i, { name: e.target.value })} />
          <select value={s.mode} onChange={(e) => update(i, { mode: e.target.value as CustomSearch["mode"] })}>
            <option value="contains">Contains</option>
            <option value="does_not_contain">Does Not Contain</option>
          </select>
          <select value={s.kind} onChange={(e) => update(i, { kind: e.target.value as CustomSearch["kind"] })}>
            <option value="text">Text</option>
            <option value="regex">Regex</option>
          </select>
          {expanded === i ? (
            <textarea className="grow" rows={5} placeholder="Search for… (multi-line, e.g. a whole tracking snippet)" value={s.query} onChange={(e) => update(i, { query: e.target.value })} spellCheck={false} />
          ) : (
            <input className="grow" placeholder="Search for…" value={s.query} onChange={(e) => update(i, { query: e.target.value })} spellCheck={false} />
          )}
          <button onClick={() => setExpanded(expanded === i ? null : i)} title="Multi-line query">
            {expanded === i ? "⤡" : "⤢"}
          </button>
          {s.kind === "regex" && <Validity kind="regex" expression={s.query} />}
          <select value={s.search_in} onChange={(e) => update(i, { search_in: e.target.value as CustomSearch["search_in"] })}>
            <option value="html">HTML</option>
            <option value="page_text">Page Text</option>
            <option value="page_text_no_anchors">Page Text No Anchors</option>
            <option value="html_head">HTML Head</option>
            <option value="html_body">HTML Body</option>
            <option value="xpath">XPath</option>
            <option value="content_area">Content Area</option>
          </select>
          {s.search_in === "xpath" && (
            <>
              <input placeholder="XPath, e.g. //h3" value={s.xpath} onChange={(e) => update(i, { xpath: e.target.value })} spellCheck={false} />
              <Validity kind="xpath" expression={s.xpath} />
            </>
          )}
          <label className="check" title="Text searches only; for regex start with (?i) to ignore case">
            <input type="checkbox" checked={s.case_sensitive} disabled={s.kind === "regex"} onChange={(e) => update(i, { case_sensitive: e.target.checked })} />
            Case Sensitive
          </label>
          <RowButtons rules={rules} i={i} onChange={onChange} copy={(r) => ({ ...r, name: `${r.name} copy` })} />
        </div>
      ))}
      <div className="button-row">
        <button onClick={() => onChange([...rules, newSearch(rules.length + 1)])} disabled={rules.length >= MAX_RULES}>
          Add
        </button>
        <button onClick={() => setBulk(true)} disabled={rules.length >= MAX_RULES}>
          Bulk Add…
        </button>
        <button onClick={() => onChange([])} disabled={rules.length === 0}>
          Delete All
        </button>
        <span className="muted">
          {rules.length} / {MAX_RULES}
        </span>
      </div>
      <p className="hint">
        Text matching ignores case unless Case Sensitive is ticked; regex is case-sensitive unless it starts with (?i). Contains counts matches; Does Not Contain flags pages with none.
      </p>
      {bulk && (
        <BulkAddSearches
          start={rules.length}
          room={MAX_RULES - rules.length}
          onCancel={() => setBulk(false)}
          onAdd={(added) => {
            onChange([...rules, ...added]);
            setBulk(false);
          }}
        />
      )}
    </div>
  );
}

/** Custom Search > Bulk Add: one search per line, sharing the settings below. */
function BulkAddSearches({ start, room, onAdd, onCancel }: { start: number; room: number; onAdd: (s: CustomSearch[]) => void; onCancel: () => void }) {
  const [text, setText] = useState("");
  const [useName, setUseName] = useState(false);
  const [name, setName] = useState("Keyword");
  const [template, setTemplate] = useState<CustomSearch>(newSearch(0));
  const queries = text
    .split(/\r?\n/)
    .map((q) => q.trim())
    .filter(Boolean)
    .slice(0, room);
  return (
    <div className="bulk-add">
      <h4>Bulk Add</h4>
      <label className="check">
        <input type="checkbox" checked={useName} onChange={(e) => setUseName(e.target.checked)} /> Specify search name
        <input value={name} disabled={!useName} onChange={(e) => setName(e.target.value)} />
      </label>
      <textarea rows={8} placeholder={"One query per line, e.g.\ntoad\nfrog\nnewt"} value={text} onChange={(e) => setText(e.target.value)} spellCheck={false} />
      <div className="button-row">
        <select value={template.mode} onChange={(e) => setTemplate({ ...template, mode: e.target.value as CustomSearch["mode"] })}>
          <option value="contains">Contains</option>
          <option value="does_not_contain">Does Not Contain</option>
        </select>
        <select value={template.kind} onChange={(e) => setTemplate({ ...template, kind: e.target.value as CustomSearch["kind"] })}>
          <option value="text">Text</option>
          <option value="regex">Regex</option>
        </select>
        <select value={template.search_in} onChange={(e) => setTemplate({ ...template, search_in: e.target.value as CustomSearch["search_in"] })}>
          <option value="html">HTML</option>
          <option value="page_text">Page Text</option>
          <option value="page_text_no_anchors">Page Text No Anchors</option>
          <option value="html_head">HTML Head</option>
          <option value="html_body">HTML Body</option>
          <option value="content_area">Content Area</option>
        </select>
        <label className="check">
          <input type="checkbox" checked={template.case_sensitive} onChange={(e) => setTemplate({ ...template, case_sensitive: e.target.checked })} /> Case Sensitive
        </label>
      </div>
      <div className="button-row">
        <span className="muted">
          {queries.length} search{queries.length === 1 ? "" : "es"} (room for {room})
        </span>
        <span className="spacer" />
        <button onClick={onCancel}>Cancel</button>
        <button
          className="primary"
          disabled={queries.length === 0}
          onClick={() => onAdd(queries.map((q, k) => ({ ...template, query: q, name: useName ? `${name} ${start + k + 1}` : q })))}
        >
          Add {queries.length}
        </button>
      </div>
    </div>
  );
}

function ExtractionEditor({ rules, onChange }: { rules: CustomExtractor[]; onChange: (v: CustomExtractor[]) => void }) {
  const update = (i: number, patch: Partial<CustomExtractor>) => onChange(rules.map((x, j) => (j === i ? { ...x, ...patch } : x)));
  return (
    <div className="rules">
      <p className="hint">Extract data from internal HTML pages using XPath, CSS Path or regex. Each extractor becomes columns in the Custom Extraction tab.</p>
      {rules.map((x, i) => (
        <div key={i} className="rule">
          <input placeholder="Name" value={x.name} onChange={(e) => update(i, { name: e.target.value })} />
          <select value={x.kind} onChange={(e) => update(i, { kind: e.target.value as CustomExtractor["kind"] })}>
            <option value="xpath">XPath</option>
            <option value="css">CSSPath</option>
            <option value="regex">Regex</option>
          </select>
          <input
            className="grow"
            placeholder={x.kind === "xpath" ? "e.g. //p[@class='price'] or count(//a)" : x.kind === "css" ? "e.g. span.price" : 'e.g. "sku":"([^"]+)"'}
            value={x.expression}
            onChange={(e) => update(i, { expression: e.target.value })}
            spellCheck={false}
          />
          <Validity kind={x.kind} expression={x.expression} />
          {x.kind !== "regex" && (
            <>
              <input placeholder="Attribute (optional)" value={x.attribute ?? ""} onChange={(e) => update(i, { attribute: e.target.value || null })} />
              <select value={x.extract} disabled={!!x.attribute} onChange={(e) => update(i, { extract: e.target.value as CustomExtractor["extract"] })}>
                <option value="element">Extract HTML Element</option>
                <option value="inner_html">Extract Inner HTML</option>
                <option value="text">Extract Text</option>
                {x.kind === "xpath" && <option value="function_value">Function Value</option>}
              </select>
            </>
          )}
          <RowButtons rules={rules} i={i} onChange={onChange} copy={(r) => ({ ...r, name: `${r.name} copy` })} />
        </div>
      ))}
      <div className="button-row">
        <button onClick={() => onChange([...rules, newExtractor(rules.length + 1)])} disabled={rules.length >= MAX_RULES}>
          Add
        </button>
        <button onClick={() => onChange([])} disabled={rules.length === 0}>
          Delete All
        </button>
        <span className="muted">
          {rules.length} / {MAX_RULES}
        </span>
      </div>
      <p className="hint">
        XPath 1.0 (functions such as count() and string-length() with Function Value; combine expressions with |). Regex returns the first capture group, or the whole
        match. Up to 1,000 values per page in total.
      </p>
    </div>
  );
}

const SAMPLE_JS = `// Return data with seoSpider.data(...). Other helpers:
// seoSpider.error(msg), seoSpider.saveUrls([...]),
// seoSpider.saveText(text, filePath, append), await seoSpider.loadScript(src)
return seoSpider.data(document.querySelectorAll('a').length);`;

const newJavaScript = (n: number): CustomJavaScript => ({ name: `Snippet ${n}`, kind: "extraction", code: SAMPLE_JS, timeout_secs: 10, content_types: "text/html" });

/** Custom > Custom JavaScript: snippets run in the browser on each rendered page. */
function JavaScriptEditor({ rules, jsMode, onChange }: { rules: CustomJavaScript[]; jsMode: boolean; onChange: (v: CustomJavaScript[]) => void }) {
  const update = (i: number, patch: Partial<CustomJavaScript>) => onChange(rules.map((x, j) => (j === i ? { ...x, ...patch } : x)));
  return (
    <div className="rules">
      <p className="hint">
        JavaScript run in the browser on each rendered page. Extraction snippets become columns in the Custom JavaScript tab; Action snippets (scroll, click, wait) run
        first and return nothing.
      </p>
      {!jsMode && <p className="hint warn">Needs Configuration &gt; Spider &gt; Rendering set to JavaScript.</p>}
      {rules.map((x, i) => (
        <div key={i} className="rule rule-js">
          <div className="rule-js-head">
            <input placeholder="Name" value={x.name} onChange={(e) => update(i, { name: e.target.value })} />
            <select value={x.kind} onChange={(e) => update(i, { kind: e.target.value as CustomJavaScript["kind"] })}>
              <option value="extraction">Extraction</option>
              <option value="action">Action</option>
            </select>
            <label className="inline">
              Timeout
              <input type="number" min={0.1} step={1} value={x.timeout_secs} onChange={(e) => update(i, { timeout_secs: Number(e.target.value) })} />s
            </label>
            <input placeholder="Content types" value={x.content_types} onChange={(e) => update(i, { content_types: e.target.value })} title="Comma-separated content types" />
            <RowButtons rules={rules} i={i} onChange={onChange} copy={(r) => ({ ...r, name: `${r.name} copy` })} />
          </div>
          <textarea className="code" rows={6} value={x.code} spellCheck={false} onChange={(e) => update(i, { code: e.target.value })} />
        </div>
      ))}
      <div className="button-row">
        <button onClick={() => onChange([...rules, newJavaScript(rules.length + 1)])} disabled={rules.length >= MAX_RULES}>
          Add
        </button>
        <button onClick={() => onChange([])} disabled={rules.length === 0}>
          Delete All
        </button>
        <span className="muted">
          {rules.length} / {MAX_RULES}
        </span>
      </div>
    </div>
  );
}

/** Custom > Link Positions: ordered name → link-path substring table, with a Test box. */
function LinkPositionsField({ cfg, set }: { cfg: CrawlConfig; set: (key: string, value: unknown) => void }) {
  const rows = cfg.link_positions;
  const [path, setPath] = useState("/html/body/header/nav/a");
  const update = (i: number, patch: Partial<{ name: string; contains: string }>) => set("link_positions", rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const matched = rows.find((p) => p.contains && path.includes(p.contains));
  return (
    <div className="field">
      <table className="mapping-table">
        <thead>
          <tr>
            <th>Link Position</th>
            <th>Link Path contains</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              <td>
                <input value={r.name} disabled={!cfg.link_positions_enabled} onChange={(e) => update(i, { name: e.target.value })} />
              </td>
              <td>
                <input value={r.contains} disabled={!cfg.link_positions_enabled} onChange={(e) => update(i, { contains: e.target.value })} spellCheck={false} />
              </td>
              <td className="rule-buttons">
                <button onClick={() => set("link_positions", move(rows, i, -1))} disabled={i === 0}>
                  ↑
                </button>
                <button onClick={() => set("link_positions", move(rows, i, 1))} disabled={i === rows.length - 1}>
                  ↓
                </button>
                <button onClick={() => set("link_positions", rows.filter((_, j) => j !== i))}>Delete</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="button-row">
        <button onClick={() => set("link_positions", [...rows, { name: "", contains: "" }])} disabled={!cfg.link_positions_enabled}>
          Add
        </button>
        <button onClick={() => set("link_positions", [])} disabled={rows.length === 0}>
          Delete All
        </button>
      </div>
      <p className="hint">Rows are tested top-down against each link's path and the first match wins; keep Content (/) last because it matches everything.</p>
      <fieldset className="url-test">
        <legend>Test</legend>
        <label className="stack">
          Link Path
          <input value={path} onChange={(e) => setPath(e.target.value)} spellCheck={false} />
        </label>
        <p className="verdict good">Matched Link Position: {matched ? matched.name || "(unnamed)" : "none"}</p>
      </fieldset>
    </div>
  );
}
