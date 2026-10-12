import { useEffect, useMemo, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { api, type ProfileInfo } from "../api";
import { FieldControl } from "../config/FieldControl";
import {
  NODES,
  changedFields,
  formatValue,
  getPath,
  nodePath,
  setPath,
  type CrawlConfig,
  type Field,
  type Node,
} from "../config/schema";

const PROFILE_FILTER = [{ name: "Screaming Toad configuration", extensions: ["toadconfig"] }];
const CHANGES = "__changes__";

function fieldText(f: Field): string {
  if (f.kind === "note") return f.text;
  if (f.kind === "custom-search") return "Custom Search";
  if (f.kind === "custom-extraction") return "Custom Extraction";
  return `${f.label} ${"help" in f && f.help ? f.help : ""}`;
}

/** Configuration dialog: a tree in Screaming Frog's menu order, with search,
 *  a "changed from default" view, validation and profiles. */
export function ConfigDialog({ onClose }: { onClose: () => void }) {
  const [cfg, setCfg] = useState<CrawlConfig | null>(null);
  const [defaults, setDefaults] = useState<CrawlConfig | null>(null);
  const [nodeId, setNodeId] = useState("crawl");
  const [query, setQuery] = useState("");
  const [problems, setProblems] = useState<string[]>([]);
  const [recent, setRecent] = useState<ProfileInfo[]>([]);
  const [hasDefault, setHasDefault] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  // Bumped when the whole config is replaced (load, reset), to refresh inputs
  // that keep their own text while typing.
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    api.getConfig().then(setCfg).catch(console.error);
    api.builtinConfig().then(setDefaults).catch(console.error);
    api.recentProfiles().then(setRecent).catch(console.error);
    api.hasDefaultProfile().then(setHasDefault).catch(console.error);
  }, []);

  // Validate as the user edits (debounced).
  useEffect(() => {
    if (!cfg) return;
    const t = setTimeout(() => api.validateConfig(cfg).then(setProblems).catch(console.error), 250);
    return () => clearTimeout(t);
  }, [cfg]);

  // Ctrl+Shift+C opens the "changed from default" view, as in Screaming Frog.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "c") {
        e.preventDefault();
        setNodeId(CHANGES);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return null;
    const hit = new Set<string>();
    for (const n of NODES) {
      const text = [n.label, ...n.groups.flatMap((g) => [g.title ?? "", ...g.fields.map(fieldText)])].join(" ").toLowerCase();
      if (text.includes(q)) {
        hit.add(n.id);
        if (n.parent) hit.add(n.parent);
      }
    }
    return hit;
  }, [query]);

  if (!cfg) return null;

  const set = (key: string, value: unknown) => setCfg((c) => (c ? setPath(c, key, value) : c));
  const replace = (next: CrawlConfig) => {
    setCfg(next);
    setRevision((r) => r + 1);
  };
  const changes = defaults ? changedFields(cfg, defaults) : [];
  const node = NODES.find((n) => n.id === nodeId);
  const visibleNodes = NODES.filter((n) => !matches || matches.has(n.id));

  // With a search, show every matching control from every page.
  const q = query.trim().toLowerCase();
  const searchHits: { node: Node; title?: string; field: Field }[] = q
    ? NODES.flatMap((n) =>
        n.groups.flatMap((g) =>
          g.fields
            .filter((f) => f.kind !== "note" && (fieldText(f).toLowerCase().includes(q) || (g.title ?? "").toLowerCase().includes(q) || n.label.toLowerCase().includes(q)))
            .map((field) => ({ node: n, title: g.title, field })),
        ),
      )
    : [];

  const apply = async () => {
    try {
      await api.setConfig(cfg);
      onClose();
    } catch (e) {
      setProblems(String(e).split("\n"));
    }
  };

  const loadProfile = async (path?: string) => {
    const picked = path ?? (await open({ filters: PROFILE_FILTER }));
    if (typeof picked !== "string") return;
    try {
      replace(await api.loadProfile(picked));
      setRecent(await api.recentProfiles());
      setNotice(`Loaded ${picked}`);
    } catch (e) {
      setNotice(String(e));
    }
  };

  const saveProfile = async () => {
    const path = await save({ defaultPath: "crawl.toadconfig", filters: PROFILE_FILTER });
    if (!path) return;
    try {
      await api.saveProfile(path, cfg);
      setRecent(await api.recentProfiles());
      setNotice(`Saved ${path}`);
    } catch (e) {
      setNotice(String(e));
    }
  };

  const saveDefault = async () => {
    try {
      await api.saveDefaultProfile(cfg);
      setHasDefault(true);
      setNotice("Saved as the default configuration. The app starts with it from now on.");
    } catch (e) {
      setNotice(String(e));
    }
  };

  const clearDefault = async () => {
    replace(await api.clearDefaultProfile());
    setHasDefault(false);
    setNotice("Cleared the saved default. Built-in defaults restored.");
  };

  const renderFields = (n: Node) => (
    <>
      {n.planned && <p className="planned">{n.planned}</p>}
      {n.groups.map((g, gi) => (
        <fieldset key={gi} className="config-group">
          {g.title && <legend>{g.title}</legend>}
          {g.fields.map((f, fi) => (
            <FieldControl key={fi} field={f} cfg={cfg} defaults={defaults} set={set} highlight={q} />
          ))}
        </fieldset>
      ))}
      {!n.planned && n.groups.length === 0 && (
        <ul className="child-links">
          {NODES.filter((c) => c.parent === n.id).map((c) => (
            <li key={c.id}>
              <button className="link-button" onClick={() => setNodeId(c.id)}>
                {c.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </>
  );

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal config-modal" onClick={(e) => e.stopPropagation()}>
        <div className="config-head">
          <h2>Configuration</h2>
          <input className="config-search" placeholder="Search…" value={query} onChange={(e) => setQuery(e.target.value)} autoFocus spellCheck={false} />
        </div>
        <div className="modal-layout">
          <nav className="modal-nav config-tree">
            {visibleNodes.map((n) => (
              <button
                key={n.id}
                className={`${nodeId === n.id && !q ? "active" : ""}${n.parent ? " child" : ""}${n.planned ? " planned-node" : ""}`}
                onClick={() => {
                  setQuery("");
                  setNodeId(n.groups.length === 0 && !n.planned ? (NODES.find((c) => c.parent === n.id)?.id ?? n.id) : n.id);
                }}
              >
                {n.label}
              </button>
            ))}
            <button className={`changes-node${nodeId === CHANGES && !q ? " active" : ""}`} onClick={() => (setQuery(""), setNodeId(CHANGES))} title="Ctrl+Shift+C">
              Changed from default {changes.length > 0 && <span className="badge neutral">{changes.length}</span>}
            </button>
          </nav>

          <div className="modal-content config-content" key={revision}>
            {q ? (
              <>
                <h3 className="config-title">Search results</h3>
                {searchHits.length === 0 && <p className="empty">No settings match “{query}”.</p>}
                {searchHits.map((h, i) => (
                  <div key={i} className="search-hit">
                    <button className="crumb link-button" onClick={() => (setQuery(""), setNodeId(h.node.id))}>
                      {nodePath(h.node)}
                      {h.title ? ` > ${h.title}` : ""}
                    </button>
                    <FieldControl field={h.field} cfg={cfg} defaults={defaults} set={set} highlight={q} />
                  </div>
                ))}
              </>
            ) : nodeId === CHANGES ? (
              <>
                <h3 className="config-title">Changed from default</h3>
                {changes.length === 0 ? (
                  <p className="empty">Every setting is at its default.</p>
                ) : (
                  <table className="changes-table">
                    <thead>
                      <tr>
                        <th>Setting</th>
                        <th>Default</th>
                        <th>Current</th>
                        <th></th>
                      </tr>
                    </thead>
                    <tbody>
                      {changes.map((c) => (
                        <tr key={c.key}>
                          <td>
                            <button className="link-button" onClick={() => setNodeId(c.node.id)}>
                              {nodePath(c.node)} &gt; {c.group ? `${c.group} > ` : ""}
                              {c.label}
                            </button>
                          </td>
                          <td className="muted">{formatValue(getPath(defaults, c.key))}</td>
                          <td>{formatValue(getPath(cfg, c.key))}</td>
                          <td>
                            <button onClick={() => (set(c.key, getPath(defaults, c.key)), setRevision((r) => r + 1))}>Reset</button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
                <p className="hint">Compared with the built-in defaults{hasDefault ? " (a saved default is also in use)" : ""}.</p>
              </>
            ) : (
              node && (
                <>
                  <h3 className="config-title">{nodePath(node)}</h3>
                  {renderFields(node)}
                </>
              )
            )}
          </div>
        </div>

        <div className={`config-status ${problems.length ? "invalid" : "valid"}`}>
          {problems.length === 0 ? (
            "✓ Valid configuration"
          ) : (
            <>
              ✗ {problems.length} problem{problems.length === 1 ? "" : "s"}:
              <ul>
                {problems.map((p) => (
                  <li key={p}>{p}</li>
                ))}
              </ul>
            </>
          )}
        </div>
        {notice && (
          <p className="hint notice" onClick={() => setNotice(null)}>
            {notice}
          </p>
        )}

        <div className="modal-actions">
          <select
            className="profiles"
            value=""
            onChange={async (e) => {
              const v = e.target.value;
              e.target.value = "";
              if (v === "load") await loadProfile();
              else if (v === "save") await saveProfile();
              else if (v === "default") await saveDefault();
              else if (v === "clear") await clearDefault();
              else if (v.startsWith("recent:")) await loadProfile(v.slice(7));
            }}
            title="Configuration profiles"
          >
            <option value="">Profiles…</option>
            <option value="load">Load…</option>
            <option value="save">Save As…</option>
            {recent.length > 0 && (
              <optgroup label="Load Recent">
                {recent.map((r) => (
                  <option key={r.path} value={`recent:${r.path}`} disabled={!r.exists} title={r.path}>
                    {r.name}
                    {r.exists ? "" : " (missing)"}
                  </option>
                ))}
              </optgroup>
            )}
            <option value="default">Save Current Configuration as Default</option>
            <option value="clear" disabled={!hasDefault}>
              Clear Default Configuration
            </option>
          </select>
          <button onClick={() => defaults && replace(defaults)} disabled={!defaults} title="Back to the built-in defaults (not saved until OK)">
            Reset to defaults
          </button>
          <span className="spacer" />
          <span className="muted change-count">{changes.length} changed</span>
          <button onClick={onClose}>Cancel</button>
          <button className="primary" onClick={apply} disabled={problems.length > 0}>
            OK
          </button>
        </div>
      </div>
    </div>
  );
}
