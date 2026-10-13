import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { authApi, type AuthConfig, type Credential } from "../api";

const AUTH_FILTER = [{ name: "Screaming Toad authentication", extensions: ["toadauth"] }];

/**
 * Configuration > Authentication. Credentials are held for this session
 * only: they are never saved into crawl files or configuration profiles.
 * Export writes them to a password-encrypted file.
 */
export function AuthenticationPanel() {
  const [auth, setAuth] = useState<AuthConfig | null>(null);
  const [tab, setTab] = useState<"standards" | "forms">("standards");
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    authApi.get().then(setAuth).catch((e) => setError(String(e)));
  }, []);
  if (!auth) return error ? <p className="error">{error}</p> : null;

  const update = (next: AuthConfig) => {
    setAuth(next);
    authApi.set(next).catch((e) => setError(String(e)));
  };

  return (
    <div className="auth-panel">
      <p className="hint warn">
        Crawling while logged in follows every link the account can see, including ones that change data (log out, delete, publish). Add
        Exclude rules for logout and admin URLs, and use a test account where you can.
      </p>
      <div className="source-switch">
        <button className={tab === "standards" ? "active" : ""} onClick={() => setTab("standards")}>
          Standards Based
        </button>
        <button className={tab === "forms" ? "active" : ""} onClick={() => setTab("forms")}>
          Forms Based
        </button>
      </div>
      {tab === "standards" ? <StandardsTab auth={auth} update={update} /> : <FormsTab auth={auth} update={update} setError={setError} />}
      <ImportExport onImported={(a) => { setAuth(a); setNote("Imported."); }} setError={setError} setNote={setNote} />
      {note && <p className="hint">{note}</p>}
      {error && <p className="error">{error}</p>}
      <p className="hint">Held for this session only. Credentials are never saved in crawl files or configuration profiles; use Export to keep them.</p>
    </div>
  );
}

function StandardsTab({ auth, update }: { auth: AuthConfig; update: (a: AuthConfig) => void }) {
  const set = (i: number, patch: Partial<Credential>) => update({ ...auth, standards: auth.standards.map((c, j) => (j === i ? { ...c, ...patch } : c)) });
  return (
    <div className="field">
      <label className="row">
        <span>Show a login prompt when a site asks for a username and password</span>
        <input type="checkbox" checked={auth.prompt} onChange={(e) => update({ ...auth, prompt: e.target.checked })} />
      </label>
      <p className="hint">Basic and Digest authentication (HTTP 401). Credentials below are used without asking.</p>
      <table className="mapping-table">
        <thead>
          <tr>
            <th>Host or URL prefix</th>
            <th>Username</th>
            <th>Password</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {auth.standards.map((c, i) => (
            <tr key={i}>
              <td>
                <input value={c.url} placeholder="staging.example.com" onChange={(e) => set(i, { url: e.target.value })} spellCheck={false} />
              </td>
              <td>
                <input value={c.username} onChange={(e) => set(i, { username: e.target.value })} spellCheck={false} />
              </td>
              <td>
                <input type="password" value={c.password} onChange={(e) => set(i, { password: e.target.value })} />
              </td>
              <td>
                <button onClick={() => update({ ...auth, standards: auth.standards.filter((_, j) => j !== i) })} title="Remove">
                  ✕
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="button-row">
        <button onClick={() => update({ ...auth, standards: [...auth.standards, { url: "", username: "", password: "" }] })}>Add</button>
      </div>
    </div>
  );
}

function FormsTab({ auth, update, setError }: { auth: AuthConfig; update: (a: AuthConfig) => void; setError: (e: string | null) => void }) {
  const [url, setUrl] = useState("");
  const [name, setName] = useState("");
  const [open_, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  const start = async () => {
    setError(null);
    setBusy(true);
    try {
      await authApi.startFormLogin(url);
      setOpen(true);
      if (!name) setName(hostOf(url));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const done = async () => {
    setBusy(true);
    try {
      const login = await authApi.finishFormLogin(name || hostOf(url), url);
      update({ ...auth, forms: [...auth.forms.filter((f) => f.name !== login.name), login] });
      setOpen(false);
      setUrl("");
      setName("");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const cancel = async () => {
    await authApi.cancelFormLogin().catch(console.error);
    setOpen(false);
  };

  return (
    <div className="field">
      <p className="hint">
        Log in through a browser window (Chrome or Edge). Its cookies are then sent with every matching request, and set in the browser when
        rendering JavaScript.
      </p>
      <table className="mapping-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Login page</th>
            <th>Cookies</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {auth.forms.map((f, i) => (
            <tr key={i}>
              <td>{f.name}</td>
              <td title={f.login_url}>{f.login_url}</td>
              <td title={f.cookies.map((c) => `${c.name} (${c.domain})`).join("\n")}>{f.cookies.length}</td>
              <td>
                <button onClick={() => update({ ...auth, forms: auth.forms.filter((_, j) => j !== i) })} title="Remove">
                  ✕
                </button>
              </td>
            </tr>
          ))}
          {auth.forms.length === 0 && (
            <tr>
              <td colSpan={4} className="muted">
                No logins yet.
              </td>
            </tr>
          )}
        </tbody>
      </table>
      {!open_ ? (
        <div className="button-row">
          <input className="grow" placeholder="https://www.example.com/login" value={url} onChange={(e) => setUrl(e.target.value)} spellCheck={false} />
          <button onClick={start} disabled={!url.trim() || busy}>
            Open Browser…
          </button>
        </div>
      ) : (
        <div className="login-waiting">
          <p>Log in in the browser window, then click Done.</p>
          <div className="button-row">
            <input placeholder="Name" value={name} onChange={(e) => setName(e.target.value)} />
            <button className="primary" onClick={done} disabled={busy}>
              Done
            </button>
            <button onClick={cancel} disabled={busy}>
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function ImportExport({
  onImported,
  setError,
  setNote,
}: {
  onImported: (a: AuthConfig) => void;
  setError: (e: string | null) => void;
  setNote: (n: string | null) => void;
}) {
  const [password, setPassword] = useState("");
  const run = async (kind: "import" | "export") => {
    setError(null);
    setNote(null);
    if (!password) {
      setError("Enter the file's password first.");
      return;
    }
    try {
      if (kind === "export") {
        const path = await save({ defaultPath: "credentials.toadauth", filters: AUTH_FILTER });
        if (!path) return;
        await authApi.exportTo(path, password);
        setNote(`Exported to ${path}. Use it with the CLI's --auth-config.`);
      } else {
        const path = await open({ filters: AUTH_FILTER });
        if (typeof path !== "string") return;
        onImported(await authApi.importFrom(path, password));
      }
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <fieldset className="config-group">
      <legend>Import / Export</legend>
      <div className="button-row">
        <input type="password" placeholder="File password" value={password} onChange={(e) => setPassword(e.target.value)} />
        <button onClick={() => run("import")}>Import…</button>
        <button onClick={() => run("export")}>Export…</button>
      </div>
      <p className="hint">Encrypted with AES-256-GCM. The same file works with the CLI (--auth-config, password in TOAD_AUTH_PASSWORD).</p>
    </fieldset>
  );
}

function hostOf(u: string): string {
  try {
    return new URL(u.includes("://") ? u : `https://${u}`).host;
  } catch {
    return u;
  }
}

/** The in-crawl login prompt for a Basic/Digest challenge. */
export function AuthPromptDialog({ req, onDone }: { req: { id: number; url: string; realm: string; scheme: string }; onDone: () => void }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const answer = (creds: [string, string] | null) => {
    authApi.answer(req.id, req.url, creds).catch(console.error).finally(onDone);
  };
  return (
    <div className="modal-backdrop">
      <form
        className="modal"
        onSubmit={(e) => {
          e.preventDefault();
          answer([username, password]);
        }}
      >
        <h2>Authentication Required</h2>
        <p>
          {hostOf(req.url)} asks for a username and password ({req.scheme}
          {req.realm ? `, realm “${req.realm}”` : ""}).
        </p>
        <p className="hint" title={req.url}>
          {req.url}
        </p>
        <div className="field">
          <label className="stack">
            Username
            <input autoFocus value={username} onChange={(e) => setUsername(e.target.value)} spellCheck={false} />
          </label>
          <label className="stack">
            Password
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
          </label>
        </div>
        <div className="modal-actions">
          <span className="spacer" />
          <button type="button" onClick={() => answer(null)}>
            Skip
          </button>
          <button className="primary" type="submit">
            Log In
          </button>
        </div>
      </form>
    </div>
  );
}
