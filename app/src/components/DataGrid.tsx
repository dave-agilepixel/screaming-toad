import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, type Table } from "../api";

const ROW_HEIGHT = 24;
const BLOCK = 200;

interface Props {
  tab: string;
  filter: string;
  search: string;
  sort: { column: string; desc: boolean } | null;
  onSort: (column: string) => void;
  selected: string | null;
  onSelect: (address: string) => void;
  /** Bump to re-query (live crawl updates, finished analysis). */
  refreshKey: number;
}

/** Column widths by header, so URLs get room and numbers stay narrow. */
function widthFor(column: string): number {
  if (column === "Address" || column.endsWith("URL") || column.startsWith("Canonical") || column === "Final Destination") return 360;
  if (/^(Title|Meta Description|H1-|H2-|Meta Keywords)/.test(column) && !/Length|Width|Occurrences/.test(column)) return 280;
  if (/Status$|Indexability Status|Content Type|Readability|Redirect Type|Matched/.test(column)) return 150;
  if (column === "Hash") return 260;
  return 110;
}

/**
 * Virtualised grid: only visible rows are rendered, and rows are fetched
 * from the store in blocks of 200 as the user scrolls.
 */
export function DataGrid({ tab, filter, search, sort, onSort, selected, onSelect, refreshKey }: Props) {
  const scroller = useRef<HTMLDivElement>(null);
  const [height, setHeight] = useState(400);
  const [scrollTop, setScrollTop] = useState(0);
  const [columns, setColumns] = useState<string[]>([]);
  const [total, setTotal] = useState(0);
  const [blocks, setBlocks] = useState<Map<number, string[][]>>(new Map());
  const pending = useRef<Set<number>>(new Set());
  const generation = useRef(0);

  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setHeight(el.clientHeight));
    ro.observe(el);
    setHeight(el.clientHeight);
    return () => ro.disconnect();
  }, []);

  const fetchBlock = useCallback(
    async (block: number, gen: number) => {
      if (pending.current.has(block)) return;
      pending.current.add(block);
      try {
        const t: Table = await api.rows({
          tab,
          filter,
          search: search || undefined,
          sortColumn: sort?.column,
          sortDesc: sort?.desc,
          offset: block * BLOCK,
          limit: BLOCK,
        });
        if (gen !== generation.current) return;
        setColumns(t.columns);
        setTotal(t.total);
        setBlocks((prev) => new Map(prev).set(block, t.rows));
      } catch (e) {
        console.error(e);
      } finally {
        pending.current.delete(block);
      }
    },
    [tab, filter, search, sort],
  );

  // New query: drop cached rows. A refresh keeps rows on screen until replaced.
  useEffect(() => {
    generation.current += 1;
    pending.current.clear();
    setBlocks(new Map());
    scroller.current?.scrollTo({ top: 0 });
    setScrollTop(0);
    fetchBlock(0, generation.current);
  }, [fetchBlock]);

  useEffect(() => {
    if (refreshKey === 0) return;
    generation.current += 1;
    pending.current.clear();
    const first = Math.floor(scrollTop / ROW_HEIGHT / BLOCK);
    fetchBlock(first, generation.current).then(() => {
      setBlocks((prev) => {
        const keep = new Map<number, string[][]>();
        const b = prev.get(first);
        if (b) keep.set(first, b);
        return keep;
      });
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshKey]);

  const firstRow = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - 10);
  const lastRow = Math.min(total, Math.ceil((scrollTop + height) / ROW_HEIGHT) + 10);

  useEffect(() => {
    for (let b = Math.floor(firstRow / BLOCK); b <= Math.floor(Math.max(lastRow - 1, 0) / BLOCK); b++) {
      if (!blocks.has(b)) fetchBlock(b, generation.current);
    }
  }, [firstRow, lastRow, blocks, fetchBlock]);

  const widths = columns.map(widthFor);
  const template = `48px ${widths.map((w) => `${w}px`).join(" ")}`;
  const totalWidth = 48 + widths.reduce((a, b) => a + b, 0);
  const addressIdx = columns.indexOf("Address");

  const rows = [];
  for (let i = firstRow; i < lastRow; i++) {
    const row = blocks.get(Math.floor(i / BLOCK))?.[i % BLOCK];
    const address = row && addressIdx >= 0 ? row[addressIdx] : null;
    rows.push(
      <div
        key={i}
        className={`grid-row${address && address === selected ? " selected" : ""}${i % 2 ? " odd" : ""}`}
        style={{ top: i * ROW_HEIGHT, gridTemplateColumns: template, width: totalWidth }}
        onClick={() => address && onSelect(address)}
      >
        <span className="cell num muted">{i + 1}</span>
        {row
          ? row.map((v, c) => (
              <span key={c} className={`cell${/^-?\d+(\.\d+)?$/.test(v) ? " num" : ""}`} title={v}>
                {v}
              </span>
            ))
          : columns.map((_, c) => <span key={c} className="cell loading" />)}
      </div>,
    );
  }

  return (
    <div className="grid">
      <div className="grid-scroll" ref={scroller} onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}>
        <div className="grid-header" style={{ gridTemplateColumns: template, width: totalWidth }}>
          <span className="cell muted">#</span>
          {columns.map((c) => (
            <button key={c} className="cell head" onClick={() => onSort(c)} title={`Sort by ${c}`}>
              {c}
              {sort?.column === c ? (sort.desc ? " ▼" : " ▲") : ""}
            </button>
          ))}
        </div>
        <div className="grid-body" style={{ height: total * ROW_HEIGHT, width: totalWidth }}>
          {rows}
        </div>
        {total === 0 && <div className="empty">No URLs match this filter.</div>}
      </div>
      <div className="grid-footer">
        {total.toLocaleString()} URL{total === 1 ? "" : "s"}
      </div>
    </div>
  );
}
