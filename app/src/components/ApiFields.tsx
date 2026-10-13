import { useEffect, useState } from "react";
import { apiAccess } from "../api";
import type { AiPrompt, CrawlConfig } from "../config/schema";

/**
 * A key or secret for an API. Stored in the OS credential store (Windows
 * Credential Manager, macOS Keychain), never in the configuration or crawl
 * files; only whether it is set is shown.
 */
export function ApiKeyField({ secret, label, help, placeholder }: { secret: string; label: string; help?: string; placeholder?: string }) {
  const [isSet, setIsSet] = useState<boolean | null>(null);
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);
  const refresh = () =>
    apiAccess
      .secretStatus()
      .then((s) => setIsSet(!!s[secret]))
      .catch((e) => setError(String(e)));
  useEffect(() => {
    refresh();
  }, [secret]);
  const save = async (v: string) => {
    setError(null);
    try {
      await apiAccess.setSecret(secret, v);
      setValue("");
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <div className="field">
      <label className="stack">
        <span>
          {label} {isSet !== null && <span className={isSet ? "pill good-pill" : "pill muted-pill"}>{isSet ? "Saved" : "Not set"}</span>}
        </span>
        <span className="button-row">
          <input
            className="grow"
            type="password"
            value={value}
            placeholder={isSet ? "•••••••• (enter a new value to replace it)" : (placeholder ?? "")}
            onChange={(e) => setValue(e.target.value)}
            autoComplete="off"
          />
          <button onClick={() => save(value)} disabled={!value.trim()}>
            Save
          </button>
          <button onClick={() => save("")} disabled={!isSet}>
            Remove
          </button>
        </span>
      </label>
      {help && <p className="hint">{help}</p>}
      {error && <p className="error">{error}</p>}
    </div>
  );
}

/** Google: the user's own OAuth client, and Connect / Disconnect. */
export function GoogleAccountField() {
  const [status, setStatus] = useState<Record<string, boolean>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const refresh = () => apiAccess.secretStatus().then(setStatus).catch((e) => setError(String(e)));
  useEffect(() => {
    refresh();
  }, []);
  const connected = !!status["google_refresh_token"];
  const run = async (f: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      refresh();
    }
  };
  return (
    <div>
      <p className="hint">
        Search Console and GA4 sign in with your own Google OAuth client: in Google Cloud Console, enable the Search Console API, Google Analytics Data API and
        Google Analytics Admin API, then create an OAuth client of type <b>Desktop app</b> and enter its ID and secret here. Access is read-only.
      </p>
      <ApiKeyField secret="google_client_id" label="OAuth Client ID" placeholder="1234-abc.apps.googleusercontent.com" />
      <ApiKeyField secret="google_client_secret" label="OAuth Client Secret" />
      <div className="field">
        <div className="button-row">
          <span className={connected ? "pill good-pill" : "pill muted-pill"}>{connected ? "Connected" : "Not connected"}</span>
          <button onClick={() => run(apiAccess.googleConnect)} disabled={busy || !status["google_client_id"] || !status["google_client_secret"]}>
            {busy ? "Waiting for the browser…" : connected ? "Reconnect" : "Connect"}
          </button>
          <button onClick={() => run(apiAccess.googleDisconnect)} disabled={busy || !connected}>
            Disconnect
          </button>
        </div>
        <p className="hint">Connect opens Google's sign-in page in your browser; come back here when it says you're signed in.</p>
        {error && <p className="error">{error}</p>}
      </div>
    </div>
  );
}

/** Search Console / GA4 property: a list from the account, or typed in. */
export function GooglePropertyField({
  which,
  label,
  value,
  onChange,
}: {
  which: "gsc" | "ga4";
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  const [options, setOptions] = useState<[string, string][] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = async () => {
    setError(null);
    try {
      const p = await apiAccess.googleProperties();
      setOptions(which === "gsc" ? p.gsc.map((s) => [s, s]) : p.ga4);
      if (p.errors.length) setError(p.errors.join("\n"));
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <div className="field">
      <label className="stack">
        {label}
        <span className="button-row">
          {options && options.length > 0 ? (
            <select className="grow" value={value} onChange={(e) => onChange(e.target.value)}>
              {!options.some(([v]) => v === value) && <option value={value}>{value || "Choose…"}</option>}
              {options.map(([v, name]) => (
                <option key={v} value={v}>
                  {name === v ? v : `${name} (${v})`}
                </option>
              ))}
            </select>
          ) : (
            <input
              className="grow"
              value={value}
              onChange={(e) => onChange(e.target.value)}
              placeholder={which === "gsc" ? "sc-domain:example.com or https://www.example.com/" : "Property ID, e.g. 123456789"}
              spellCheck={false}
            />
          )}
          <button onClick={load}>Load from Account</button>
        </span>
      </label>
      {error && <p className="error">{error}</p>}
    </div>
  );
}

const PROVIDERS: [AiPrompt["provider"], string][] = [
  ["open_ai", "OpenAI"],
  ["anthropic", "Anthropic"],
  ["gemini", "Gemini"],
  ["ollama", "Ollama"],
  ["open_ai_compatible", "OpenAI-compatible"],
];

const CONTENT: [AiPrompt["content"], string][] = [
  ["page_text", "Page Text"],
  ["content_area", "Content Area"],
  ["html", "HTML"],
  ["url_only", "URL Only"],
];

const DEFAULT_MODEL: Record<AiPrompt["provider"], string> = {
  open_ai: "gpt-4o-mini",
  anthropic: "claude-haiku-5-5",
  gemini: "gemini-2.5-flash",
  ollama: "llama3.2",
  open_ai_compatible: "",
};

/** API Access > AI > Prompt Configuration: one column per prompt. */
export function AiPromptsEditor({ cfg, set }: { cfg: CrawlConfig; set: (key: string, value: unknown) => void }) {
  const prompts = cfg.apis.ai.prompts;
  const [testText, setTestText] = useState("Toads are amphibians that live in damp gardens.");
  const [results, setResults] = useState<Record<number, string>>({});
  const update = (i: number, patch: Partial<AiPrompt>) => set("apis.ai.prompts", prompts.map((p, j) => (j === i ? { ...p, ...patch } : p)));
  const test = async (i: number) => {
    setResults((r) => ({ ...r, [i]: "…" }));
    try {
      const out = await apiAccess.testAiPrompt(prompts[i], testText);
      setResults((r) => ({ ...r, [i]: out }));
    } catch (e) {
      setResults((r) => ({ ...r, [i]: `Error: ${e}` }));
    }
  };
  return (
    <div className="rules">
      <p className="hint">
        Each prompt runs on every crawled HTML page (after the crawl, or with Request API Data) and its answer becomes a column in the AI tab. Use {"{url}"} in the
        prompt for the page address. Page text is sent to the provider you choose.
      </p>
      {prompts.map((p, i) => (
        <div key={i} className="segment-card">
          <div className="button-row">
            <input type="checkbox" checked={p.enabled} onChange={(e) => update(i, { enabled: e.target.checked })} title="Enabled" />
            <input placeholder="Column name" value={p.name} onChange={(e) => update(i, { name: e.target.value })} />
            <select value={p.provider} onChange={(e) => update(i, { provider: e.target.value as AiPrompt["provider"], model: DEFAULT_MODEL[e.target.value as AiPrompt["provider"]] })}>
              {PROVIDERS.map(([v, n]) => (
                <option key={v} value={v}>
                  {n}
                </option>
              ))}
            </select>
            <input placeholder="Model" value={p.model} onChange={(e) => update(i, { model: e.target.value })} spellCheck={false} />
            <select value={p.content} onChange={(e) => update(i, { content: e.target.value as AiPrompt["content"] })} title="What is sent with the prompt">
              {CONTENT.map(([v, n]) => (
                <option key={v} value={v}>
                  {n}
                </option>
              ))}
            </select>
            <span className="spacer" />
            <button onClick={() => test(i)}>Test</button>
            <button onClick={() => set("apis.ai.prompts", prompts.filter((_, j) => j !== i))} title="Delete">
              ✕
            </button>
          </div>
          <textarea className="code" rows={3} value={p.prompt} placeholder="e.g. Summarise this page in one sentence." onChange={(e) => update(i, { prompt: e.target.value })} />
          {results[i] !== undefined && <p className="hint">{results[i]}</p>}
        </div>
      ))}
      <div className="button-row">
        <button
          onClick={() =>
            set("apis.ai.prompts", [
              ...prompts,
              { enabled: true, name: `Prompt ${prompts.length + 1}`, provider: "open_ai", model: DEFAULT_MODEL.open_ai, prompt: "", content: "page_text" },
            ])
          }
        >
          Add Prompt
        </button>
        <label className="inline grow">
          Test text <input className="grow" value={testText} onChange={(e) => setTestText(e.target.value)} />
        </label>
      </div>
    </div>
  );
}
