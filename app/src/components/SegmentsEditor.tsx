import { useEffect, useState } from "react";
import { api, type TabDef } from "../api";
import type { Segment, SegmentOp, SegmentRule } from "../config/schema";

const OPS: [SegmentOp, string][] = [
  ["contains", "Contains"],
  ["does_not_contain", "Does Not Contain"],
  ["equals", "Equals"],
  ["does_not_equal", "Does Not Equal"],
  ["starts_with", "Starts With"],
  ["ends_with", "Ends With"],
  ["regex", "Matches Regex"],
  ["not_regex", "Does Not Match Regex"],
  ["greater_than", "Greater Than"],
  ["less_than", "Less Than"],
];

const COLORS = ["#2f9e44", "#1971c2", "#e8590c", "#9c36b5", "#c92a2a", "#0c8599", "#5c940d", "#495057"];

const newRule = (): SegmentRule => ({ field: "Address", op: "contains", value: "" });

/**
 * Configuration > Segments. A URL belongs to the first segment it matches,
 * so order matters. Segments apply to the open crawl straight away.
 */
export function SegmentsEditor({ segments, onChange }: { segments: Segment[]; onChange: (v: Segment[]) => void }) {
  const [fields, setFields] = useState<string[]>(["Issue", "Address"]);
  const [tabs, setTabs] = useState<TabDef[]>([]);
  useEffect(() => {
    api.segmentFields().then(setFields).catch(console.error);
    api.tabs().then(setTabs).catch(console.error);
  }, []);

  const issues = tabs.flatMap((t) => t.filters.filter((f) => f.name !== "All").map((f) => `${t.name}: ${f.name}`));
  const update = (i: number, patch: Partial<Segment>) => onChange(segments.map((s, j) => (j === i ? { ...s, ...patch } : s)));
  const updateRule = (i: number, k: number, patch: Partial<SegmentRule>) =>
    update(i, { rules: segments[i].rules.map((r, j) => (j === k ? { ...r, ...patch } : r)) });
  const move = (i: number, by: number) => {
    const next = [...segments];
    const [s] = next.splice(i, 1);
    next.splice(i + by, 0, s);
    onChange(next);
  };

  return (
    <div className="rules">
      <p className="hint">
        Group URLs (blog, products, templates…) to see issues and counts per group. Rules can test any column, or whether a URL is in a filter (Issue). A URL
        belongs to the <b>first</b> segment it matches, so put specific segments above general ones. Segments apply to the open crawl straight away: the
        Segment column, the segment selector above the grid, the Segments tab on the right, Reports &gt; Segments Overview and sitemaps by segment.
      </p>
      {segments.map((s, i) => (
        <div key={i} className="segment-card" style={{ borderLeftColor: s.color }}>
          <div className="button-row">
            <input placeholder="Segment name" value={s.name} onChange={(e) => update(i, { name: e.target.value })} />
            <input type="color" value={s.color} onChange={(e) => update(i, { color: e.target.value })} title="Colour" />
            <select value={s.match_all ? "all" : "any"} onChange={(e) => update(i, { match_all: e.target.value === "all" })}>
              <option value="all">Match all rules (AND)</option>
              <option value="any">Match any rule (OR)</option>
            </select>
            <span className="spacer" />
            <button onClick={() => move(i, -1)} disabled={i === 0} title="Higher precedence">
              ↑
            </button>
            <button onClick={() => move(i, 1)} disabled={i === segments.length - 1} title="Lower precedence">
              ↓
            </button>
            <button onClick={() => onChange(segments.filter((_, j) => j !== i))} title="Delete segment">
              ✕
            </button>
          </div>
          {s.rules.map((r, k) => (
            <div key={k} className="button-row">
              <select value={r.field} onChange={(e) => updateRule(i, k, { field: e.target.value, value: e.target.value === "Issue" ? (issues[0] ?? "") : r.value })}>
                {fields.map((f) => (
                  <option key={f}>{f}</option>
                ))}
              </select>
              {r.field === "Issue" ? (
                <>
                  <select value={r.op === "does_not_equal" ? "does_not_equal" : "equals"} onChange={(e) => updateRule(i, k, { op: e.target.value as SegmentOp })}>
                    <option value="equals">Is In</option>
                    <option value="does_not_equal">Is Not In</option>
                  </select>
                  <select className="grow" value={r.value} onChange={(e) => updateRule(i, k, { value: e.target.value })}>
                    {!issues.includes(r.value) && <option value={r.value}>{r.value || "Choose a filter"}</option>}
                    {issues.map((v) => (
                      <option key={v}>{v}</option>
                    ))}
                  </select>
                </>
              ) : (
                <>
                  <select value={r.op} onChange={(e) => updateRule(i, k, { op: e.target.value as SegmentOp })}>
                    {OPS.map(([v, label]) => (
                      <option key={v} value={v}>
                        {label}
                      </option>
                    ))}
                  </select>
                  <input className="grow" value={r.value} onChange={(e) => updateRule(i, k, { value: e.target.value })} spellCheck={false} />
                </>
              )}
              <button onClick={() => update(i, { rules: s.rules.filter((_, j) => j !== k) })} title="Delete rule">
                −
              </button>
            </div>
          ))}
          <div className="button-row">
            <button onClick={() => update(i, { rules: [...s.rules, newRule()] })}>Add Rule</button>
          </div>
        </div>
      ))}
      <div className="button-row">
        <button
          onClick={() =>
            onChange([...segments, { name: `Segment ${segments.length + 1}`, color: COLORS[segments.length % COLORS.length], match_all: true, rules: [newRule()] }])
          }
        >
          Add Segment
        </button>
      </div>
    </div>
  );
}
