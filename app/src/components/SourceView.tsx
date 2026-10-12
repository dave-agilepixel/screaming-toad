import type { DiffLine } from "../api";

const NO_HTML = "No HTML stored for this URL. Switch on Configuration > Spider > Extraction > Store HTML and crawl again.";

/** View Source: the stored HTML with line numbers. */
export function SourceView({ source }: { source: string | null }) {
  if (source === null) return <p className="empty">{NO_HTML}</p>;
  const lines = source.split(/\r?\n/);
  return (
    <table className="source">
      <tbody>
        {lines.map((l, i) => (
          <tr key={i}>
            <td className="ln">{i + 1}</td>
            <td className="code">{l}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

type Side = { n: number | null; text: string; tag: DiffLine["tag"] } | null;

/**
 * Pair a line diff into side-by-side rows: equal lines on both sides; a run
 * of deletions next to the run of insertions that replaced it.
 */
function sideBySide(lines: DiffLine[]): [Side, Side][] {
  const rows: [Side, Side][] = [];
  let i = 0;
  while (i < lines.length) {
    const l = lines[i];
    if (l.tag === "equal") {
      rows.push([{ n: l.old, text: l.text, tag: "equal" }, { n: l.new, text: l.text, tag: "equal" }]);
      i++;
      continue;
    }
    const dels: DiffLine[] = [];
    const ins: DiffLine[] = [];
    while (i < lines.length && lines[i].tag !== "equal") {
      (lines[i].tag === "delete" ? dels : ins).push(lines[i]);
      i++;
    }
    for (let k = 0; k < Math.max(dels.length, ins.length); k++) {
      const d = dels[k];
      const a = ins[k];
      rows.push([d ? { n: d.old, text: d.text, tag: "delete" } : null, a ? { n: a.new, text: a.text, tag: "insert" } : null]);
    }
  }
  return rows;
}

/** View Source > Compare: current on the left, previous on the right. */
export function SourceCompareView({
  current,
  previous,
  lines,
  showDifferences,
  onlyChanges,
}: {
  current: string | null;
  previous: string | null;
  lines: DiffLine[];
  showDifferences: boolean;
  onlyChanges: boolean;
}) {
  if (current === null && previous === null) return <p className="empty">{NO_HTML}</p>;
  let rows = sideBySide(lines);
  if (onlyChanges) rows = rows.filter(([p, c]) => p?.tag !== "equal" || c?.tag !== "equal");
  const cls = (s: Side) => (showDifferences && s && s.tag !== "equal" ? ` diff-${s.tag}` : "");
  return (
    <table className="source source-compare">
      <thead>
        <tr>
          <th colSpan={2}>Current{current === null ? " (no HTML stored)" : ""}</th>
          <th colSpan={2}>Previous{previous === null ? " (no HTML stored)" : ""}</th>
        </tr>
      </thead>
      <tbody>
        {rows.map(([prev, cur], i) => (
          <tr key={i}>
            <td className="ln">{cur?.n ?? ""}</td>
            <td className={`code${cls(cur)}${cur ? "" : " blank"}`}>{cur?.text ?? ""}</td>
            <td className="ln">{prev?.n ?? ""}</td>
            <td className={`code${cls(prev)}${prev ? "" : " blank"}`}>{prev?.text ?? ""}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
