import { useEffect, useState, type ReactNode } from "react";
import { api, type UrlTest } from "../api";
import { getPath, type CrawlConfig, type CustomExtractor, type CustomSearch, type Field } from "./schema";

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

    case "url-test":
      return <UrlTestBox cfg={cfg} show={f.show} label={f.label} />;

    case "custom-search":
      return <SearchEditor rules={cfg.custom_search} onChange={(v) => set("custom_search", v)} />;

    case "custom-extraction":
      return <ExtractionEditor rules={cfg.custom_extraction} onChange={(v) => set("custom_extraction", v)} />;
  }
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
              <td>
                <input value={name} onChange={(e) => update(i, 0, e.target.value)} spellCheck={false} />
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

const newSearch = (n: number): CustomSearch => ({
  name: `Search ${n}`,
  mode: "contains",
  kind: "text",
  query: "",
  case_sensitive: false,
  search_in: "html",
});

const newExtractor = (n: number): CustomExtractor => ({
  name: `Extractor ${n}`,
  kind: "css",
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

function SearchEditor({ rules, onChange }: { rules: CustomSearch[]; onChange: (v: CustomSearch[]) => void }) {
  const update = (i: number, patch: Partial<CustomSearch>) => onChange(rules.map((s, j) => (j === i ? { ...s, ...patch } : s)));
  return (
    <div className="rules">
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
          <input className="grow" placeholder="Search for…" value={s.query} onChange={(e) => update(i, { query: e.target.value })} spellCheck={false} />
          <select value={s.search_in} onChange={(e) => update(i, { search_in: e.target.value as CustomSearch["search_in"] })}>
            <option value="html">HTML</option>
            <option value="html_head">HTML Head</option>
            <option value="html_body">HTML Body</option>
            <option value="page_text">Page Text</option>
            <option value="content_area">Content Area</option>
          </select>
          <label className="check" title="Text searches only; for regex use (?i) to ignore case">
            <input type="checkbox" checked={s.case_sensitive} disabled={s.kind === "regex"} onChange={(e) => update(i, { case_sensitive: e.target.checked })} />
            Case Sensitive
          </label>
          <span className="rule-buttons">
            <button onClick={() => onChange(move(rules, i, -1))} disabled={i === 0} title="Move up">
              ↑
            </button>
            <button onClick={() => onChange(move(rules, i, 1))} disabled={i === rules.length - 1} title="Move down">
              ↓
            </button>
            <button onClick={() => onChange([...rules.slice(0, i + 1), { ...s, name: `${s.name} copy` }, ...rules.slice(i + 1)])} title="Duplicate">
              ⧉
            </button>
            <button onClick={() => onChange(rules.filter((_, j) => j !== i))} title="Delete">
              ✕
            </button>
          </span>
        </div>
      ))}
      <button onClick={() => onChange([...rules, newSearch(rules.length + 1)])}>Add</button>
      <p className="hint">Each search becomes a filter in the Custom Search tab with a count of matches per page.</p>
    </div>
  );
}

function ExtractionEditor({ rules, onChange }: { rules: CustomExtractor[]; onChange: (v: CustomExtractor[]) => void }) {
  const update = (i: number, patch: Partial<CustomExtractor>) => onChange(rules.map((x, j) => (j === i ? { ...x, ...patch } : x)));
  return (
    <div className="rules">
      {rules.map((x, i) => (
        <div key={i} className="rule">
          <input placeholder="Name" value={x.name} onChange={(e) => update(i, { name: e.target.value })} />
          <select value={x.kind} onChange={(e) => update(i, { kind: e.target.value as CustomExtractor["kind"] })}>
            <option value="css">CSSPath</option>
            <option value="regex">Regex</option>
          </select>
          <input
            className="grow"
            placeholder={x.kind === "css" ? "e.g. span.price" : 'e.g. "sku":"([^"]+)"'}
            value={x.expression}
            onChange={(e) => update(i, { expression: e.target.value })}
            spellCheck={false}
          />
          {x.kind === "css" && (
            <>
              <input placeholder="Attribute (optional)" value={x.attribute ?? ""} onChange={(e) => update(i, { attribute: e.target.value || null })} />
              <select value={x.extract} disabled={!!x.attribute} onChange={(e) => update(i, { extract: e.target.value as CustomExtractor["extract"] })}>
                <option value="element">Extract HTML Element</option>
                <option value="inner_html">Extract Inner HTML</option>
                <option value="text">Extract Text</option>
              </select>
            </>
          )}
          <span className="rule-buttons">
            <button onClick={() => onChange(move(rules, i, -1))} disabled={i === 0} title="Move up">
              ↑
            </button>
            <button onClick={() => onChange(move(rules, i, 1))} disabled={i === rules.length - 1} title="Move down">
              ↓
            </button>
            <button onClick={() => onChange([...rules.slice(0, i + 1), { ...x, name: `${x.name} copy` }, ...rules.slice(i + 1)])} title="Duplicate">
              ⧉
            </button>
            <button onClick={() => onChange(rules.filter((_, j) => j !== i))} title="Delete">
              ✕
            </button>
          </span>
        </div>
      ))}
      <button onClick={() => onChange([...rules, newExtractor(rules.length + 1)])}>Add</button>
      <p className="hint">Regex extractors return the first capture group, or the whole match if there is none. XPath is planned.</p>
    </div>
  );
}
