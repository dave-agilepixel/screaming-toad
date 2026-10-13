import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { crawlsApi, notifyApi, type AppSettings, type NotificationSettings, type ProxyConfig } from "../api";
import { ApiKeyField } from "./ApiFields";
import { applyAppearance, DEFAULT_ACCENT } from "../appearance";

const PRESETS: [string, number | null][] = [
  ["Never", null],
  ["7 days", 7],
  ["30 days", 30],
];

/** File > Settings: storage folder, crawl retention, proxy and trusted certificates. */
export function SettingsDialog({ onClose: close }: { onClose: () => void }) {
  const [s, setS] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [mail, setMail] = useState<NotificationSettings | null>(null);
  const [mailNote, setMailNote] = useState<string | null>(null);
  // What was applied when the dialog opened, to restore on Cancel.
  const [initial, setInitial] = useState<{ theme: AppSettings["theme"]; accent: string } | null>(null);

  useEffect(() => {
    crawlsApi
      .getSettings()
      .then((x) => {
        setS(x);
        setInitial({ theme: x.theme, accent: x.accent });
      })
      .catch((e) => setError(String(e)));
    notifyApi.get().then(setMail).catch((e) => setError(String(e)));
  }, []);

  // Closing without saving puts the appearance back.
  const onClose = () => {
    if (initial) applyAppearance(initial.theme, initial.accent);
    close();
  };
  if (!s) return null;
  const preset = PRESETS.find(([, d]) => d === s.retention_days)?.[0] ?? "Custom";

  const pickFolder = async () => {
    const dir = await open({ directory: true, title: "Folder for saved crawls" });
    if (typeof dir === "string") setS({ ...s, storage_dir: dir });
  };

  const proxy = s.network.proxy;
  const setProxy = (patch: Partial<ProxyConfig>) => setS({ ...s, network: { ...s.network, proxy: { ...proxy, ...patch } } });
  const certs = s.network.trusted_certificates;
  const setCerts = (list: string[]) => setS({ ...s, network: { ...s.network, trusted_certificates: list } });
  const addCert = async () => {
    const picked = await open({ multiple: true, title: "Trusted certificates", filters: [{ name: "Certificates", extensions: ["pem", "crt", "cer", "der"] }] });
    const paths = Array.isArray(picked) ? picked : typeof picked === "string" ? [picked] : [];
    setCerts([...certs, ...paths.filter((p) => !certs.includes(p))]);
  };

  const testMail = async () => {
    if (!mail) return;
    setMailNote("Sending…");
    try {
      await notifyApi.set(mail);
      await notifyApi.test([]);
      setMailNote(`Sent to ${mail.default_recipients.join(", ")}.`);
    } catch (e) {
      setMailNote(String(e));
    }
  };

  const apply = async () => {
    try {
      if (mail) await notifyApi.set(mail);
      await crawlsApi.setSettings(s);
      close();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>Settings</h2>
        <fieldset className="config-group">
          <legend>Storage</legend>
          <p className="hint">
            Every crawl is saved automatically as its own database file (<code>.toad</code>), so crawls can be reopened, resumed and
            compared. Use a local SSD; network and cloud-synced folders are not supported.
          </p>
          <div className="field">
            <label className="stack">
              Directory
              <span className="button-row">
                <input className="grow" value={s.storage_dir ?? ""} placeholder="Default (the app's data folder)" readOnly />
                <button onClick={pickFolder}>Browse…</button>
                <button onClick={() => setS({ ...s, storage_dir: null })} disabled={!s.storage_dir}>
                  Default
                </button>
              </span>
            </label>
            <p className="hint">Changing the folder doesn't move existing crawls; File &gt; Crawls lists the crawls in the current folder.</p>
          </div>
        </fieldset>
        <fieldset className="config-group">
          <legend>Crawl Retention</legend>
          <div className="field">
            <label className="row">
              Delete crawls older than
              <span className="input-unit">
                <select
                  value={preset}
                  onChange={(e) => {
                    const p = PRESETS.find(([n]) => n === e.target.value);
                    setS({ ...s, retention_days: p ? p[1] : (s.retention_days ?? 14) });
                  }}
                >
                  {PRESETS.map(([n]) => (
                    <option key={n}>{n}</option>
                  ))}
                  <option>Custom</option>
                </select>
                {preset === "Custom" && (
                  <>
                    <input type="number" min={1} value={s.retention_days ?? 14} onChange={(e) => setS({ ...s, retention_days: Math.max(1, Number(e.target.value)) })} />
                    <span className="unit">days</span>
                  </>
                )}
              </span>
            </label>
            <p className="hint">Checked at startup and after every crawl. Locked crawls, the open crawl and crawls in progress are never deleted.</p>
          </div>
        </fieldset>
        <fieldset className="config-group">
          <legend>Proxy</legend>
          <div className="field">
            <label className="row">
              <span>Use a proxy server</span>
              <input type="checkbox" checked={proxy.enabled} onChange={(e) => setProxy({ enabled: e.target.checked })} />
            </label>
            <div className="button-row">
              <select value={proxy.kind} disabled={!proxy.enabled} onChange={(e) => setProxy({ kind: e.target.value as ProxyConfig["kind"] })}>
                <option value="http">HTTP</option>
                <option value="https">HTTPS</option>
                <option value="socks5">SOCKS5</option>
              </select>
              <input className="grow" placeholder="Address" value={proxy.host} disabled={!proxy.enabled} onChange={(e) => setProxy({ host: e.target.value })} spellCheck={false} />
              <input type="number" min={1} max={65535} value={proxy.port} disabled={!proxy.enabled} onChange={(e) => setProxy({ port: Number(e.target.value) })} style={{ width: "6em" }} />
            </div>
            <div className="button-row">
              <input placeholder="Username (optional)" value={proxy.username} disabled={!proxy.enabled} onChange={(e) => setProxy({ username: e.target.value })} spellCheck={false} />
              <input type="password" placeholder="Password" value={proxy.password} disabled={!proxy.enabled} onChange={(e) => setProxy({ password: e.target.value })} />
            </div>
            <label className="stack">
              No proxy for
              <input
                value={proxy.no_proxy.join(", ")}
                placeholder="localhost, .internal.example.com"
                disabled={!proxy.enabled}
                onChange={(e) => setProxy({ no_proxy: e.target.value.split(",").map((x) => x.trim()).filter(Boolean) })}
                spellCheck={false}
              />
            </label>
            <p className="hint">
              Used for crawling, rendering and downloads. With the proxy off, the HTTP_PROXY / HTTPS_PROXY environment variables still apply. The
              proxy password is saved in Toad's settings file.
            </p>
          </div>
        </fieldset>
        <fieldset className="config-group">
          <legend>Trusted Certificates</legend>
          <div className="field">
            <p className="hint">
              Certificates (PEM or DER) to trust in addition to the system's, for example a staging site's private CA or an intercepting proxy. JavaScript
              rendering uses the system store only.
            </p>
            {certs.map((c) => (
              <div key={c} className="button-row">
                <input className="grow" value={c} readOnly title={c} />
                <button onClick={() => setCerts(certs.filter((x) => x !== c))}>Remove</button>
              </div>
            ))}
            <div className="button-row">
              <button onClick={addCert}>Add…</button>
            </div>
          </div>
        </fieldset>
        <fieldset className="config-group">
          <legend>Appearance</legend>
          <div className="field">
            <label className="row">
              Theme
              <select
                value={s.theme}
                onChange={(e) => {
                  const theme = e.target.value as AppSettings["theme"];
                  setS({ ...s, theme });
                  applyAppearance(theme, s.accent);
                }}
              >
                <option value="dark">Dark</option>
                <option value="light">Light</option>
                <option value="system">Match the system</option>
              </select>
            </label>
            <label className="row">
              Accent colour
              <span className="button-row">
                <input
                  type="color"
                  value={s.accent}
                  onChange={(e) => {
                    setS({ ...s, accent: e.target.value });
                    applyAppearance(s.theme, e.target.value);
                  }}
                />
                <button
                  onClick={() => {
                    setS({ ...s, accent: DEFAULT_ACCENT });
                    applyAppearance(s.theme, DEFAULT_ACCENT);
                  }}
                  disabled={s.accent.toLowerCase() === DEFAULT_ACCENT}
                >
                  Default
                </button>
              </span>
            </label>
          </div>
        </fieldset>
        <fieldset className="config-group">
          <legend>MCP Server</legend>
          <div className="field">
            <label className="row">
              <span>Let AI assistants read crawls (Model Context Protocol)</span>
              <input type="checkbox" checked={s.mcp_server} onChange={(e) => setS({ ...s, mcp_server: e.target.checked })} />
            </label>
            <label className="row">
              Port
              <input type="number" min={1024} max={65535} value={s.mcp_port} onChange={(e) => setS({ ...s, mcp_port: Number(e.target.value) })} style={{ width: "7em" }} />
            </label>
            <p className="hint">
              Read-only tools (list crawls, issues, any tab and filter, URL details, links, reports, compare) at http://127.0.0.1:{s.mcp_port}/mcp, for this
              computer only. For clients that use STDIO, run <code>toad mcp --dir "crawls folder"</code> instead.
            </p>
          </div>
        </fieldset>
        {mail && (
          <fieldset className="config-group">
            <legend>Notifications (Email)</legend>
            <p className="hint">Email a summary when a scheduled crawl (or a CLI crawl with --email-on-complete) finishes.</p>
            <div className="field">
              <div className="button-row">
                <input className="grow" placeholder="SMTP server" value={mail.smtp_host} onChange={(e) => setMail({ ...mail, smtp_host: e.target.value })} spellCheck={false} />
                <input type="number" min={1} max={65535} value={mail.smtp_port} onChange={(e) => setMail({ ...mail, smtp_port: Number(e.target.value) })} style={{ width: "6em" }} />
                <select value={mail.security} onChange={(e) => setMail({ ...mail, security: e.target.value as NotificationSettings["security"] })}>
                  <option value="start_tls">STARTTLS</option>
                  <option value="tls">TLS</option>
                  <option value="none">None</option>
                </select>
              </div>
              <div className="button-row">
                <input className="grow" placeholder="Username" value={mail.username} onChange={(e) => setMail({ ...mail, username: e.target.value })} spellCheck={false} />
                <input className="grow" placeholder="From (default: username)" value={mail.from} onChange={(e) => setMail({ ...mail, from: e.target.value })} spellCheck={false} />
              </div>
            </div>
            <ApiKeyField secret="smtp_password" label="Password" help="Saved in the OS credential store. For Gmail or Outlook, use an app password." />
            <div className="field">
              <label className="stack">
                Send to
                <input
                  value={mail.default_recipients.join(", ")}
                  placeholder="you@example.com, team@example.com"
                  onChange={(e) => setMail({ ...mail, default_recipients: e.target.value.split(",").map((x) => x.trim()).filter(Boolean) })}
                  spellCheck={false}
                />
              </label>
              <div className="button-row">
                <button onClick={testMail} disabled={!mail.smtp_host.trim() || mail.default_recipients.length === 0}>
                  Send Test Email
                </button>
                {mailNote && <span className="hint">{mailNote}</span>}
              </div>
            </div>
          </fieldset>
        )}
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <span className="spacer" />
          <button onClick={onClose}>Cancel</button>
          <button className="primary" onClick={apply}>
            OK
          </button>
        </div>
      </div>
    </div>
  );
}
