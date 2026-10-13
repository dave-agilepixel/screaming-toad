import { useEffect, useMemo, useRef, useState } from "react";
import { forceCenter, forceCollide, forceLink, forceManyBody, forceSimulation, type SimulationLinkDatum, type SimulationNodeDatum } from "d3-force";
import { hierarchy, stratify, tree, type HierarchyNode, type HierarchyPointNode } from "d3-hierarchy";
import { select } from "d3-selection";
import { zoom, zoomIdentity, type ZoomTransform } from "d3-zoom";
import { vizApi, type VizGraph, type VizNode } from "../api";

export type VizKind = "crawl-tree" | "directory-tree" | "crawl-force" | "directory-force" | "clusters" | "anchor-cloud" | "body-cloud";

const KINDS: [VizKind, string][] = [
  ["crawl-tree", "Crawl Tree Graph"],
  ["directory-tree", "Directory Tree Graph"],
  ["crawl-force", "Force-Directed Crawl Diagram"],
  ["directory-force", "Force-Directed Directory Tree Diagram"],
  ["clusters", "Content Cluster Diagram"],
  ["anchor-cloud", "Inlink Anchor Text Word Cloud"],
  ["body-cloud", "Body Text Word Cloud"],
];

type ColourBy = "indexability" | "depth" | "response" | "segment" | "cluster";
type SizeBy = "fixed" | "inlinks" | "link_score";

const ACCENT = "#f8b133";
const CLUSTER_COLOURS = ["#f8b133", "#6aa8ee", "#3ddc97", "#ff6b6b", "#c084fc", "#2dd4bf", "#fb923c", "#f472b6", "#a3e635", "#38bdf8", "#facc15", "#94a3b8"];
const DEPTH_COLOURS = ["#ffffff", "#f8b133", "#f59e0b", "#d97706", "#b45309", "#92400e", "#78350f"];

function colourOf(n: VizNode, by: ColourBy, segments: Record<string, string>): string {
  if (n.virtual_node) return "#4b4b4b";
  switch (by) {
    case "indexability":
      return n.indexable ? ACCENT : "#ff6b6b";
    case "depth":
      return DEPTH_COLOURS[Math.min(n.depth, DEPTH_COLOURS.length - 1)];
    case "response":
      return n.status === 0 ? "#8f8f8f" : n.status < 300 ? "#3ddc97" : n.status < 400 ? "#6aa8ee" : "#ff6b6b";
    case "segment":
      return (n.segment && segments[n.segment]) || "#8f8f8f";
    case "cluster":
      return n.cluster === null ? "#8f8f8f" : CLUSTER_COLOURS[n.cluster % CLUSTER_COLOURS.length];
  }
}

function radiusOf(n: VizNode, by: SizeBy, max: { inlinks: number }): number {
  if (by === "inlinks") return 3 + 9 * Math.sqrt(n.unique_inlinks / Math.max(1, max.inlinks));
  if (by === "link_score") return 3 + 9 * ((n.link_score ?? 0) / 100);
  return 4;
}

interface Options {
  colourBy: ColourBy;
  sizeBy: SizeBy;
  maxNodes: number;
  labels: boolean;
}

/** Visualisations: a full-window view over the open crawl. */
export function Visualisations({
  initial,
  selected,
  segmentColours,
  onPick,
  onClose,
}: {
  initial: VizKind;
  selected: string | null;
  segmentColours: Record<string, string>;
  onPick: (url: string) => void;
  onClose: () => void;
}) {
  const [kind, setKind] = useState<VizKind>(initial);
  const [opts, setOpts] = useState<Options>({ colourBy: initial === "clusters" ? "cluster" : "indexability", sizeBy: "inlinks", maxNodes: 2000, labels: true });
  const [graph, setGraph] = useState<VizGraph | null>(null);
  const [words, setWords] = useState<[string, number][] | null>(null);
  const [cloudUrl, setCloudUrl] = useState(selected ?? "");
  const [error, setError] = useState<string | null>(null);
  const [hover, setHover] = useState<VizNode | null>(null);
  const isCloud = kind === "anchor-cloud" || kind === "body-cloud";

  useEffect(() => {
    setError(null);
    setGraph(null);
    setWords(null);
    if (isCloud) {
      if (!cloudUrl.trim()) return;
      vizApi.words(cloudUrl.trim(), kind === "body-cloud").then(setWords).catch((e) => setError(String(e)));
      return;
    }
    const load =
      kind === "crawl-tree" || kind === "crawl-force"
        ? vizApi.crawlTree(opts.maxNodes)
        : kind === "clusters"
          ? vizApi.contentClusters(opts.maxNodes)
          : vizApi.directoryTree(opts.maxNodes);
    load.then(setGraph).catch((e) => setError(String(e)));
  }, [kind, opts.maxNodes, cloudUrl, isCloud]);

  useEffect(() => {
    if (kind === "clusters") setOpts((o) => ({ ...o, colourBy: "cluster" }));
    else if (opts.colourBy === "cluster") setOpts((o) => ({ ...o, colourBy: "indexability" }));
  }, [kind]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="viz-overlay">
      <div className="viz-head">
        <h2>Visualisations</h2>
        <select value={kind} onChange={(e) => setKind(e.target.value as VizKind)}>
          {KINDS.map(([k, l]) => (
            <option key={k} value={k}>
              {l}
            </option>
          ))}
        </select>
        {isCloud ? (
          <input className="grow" value={cloudUrl} placeholder="URL (select one in the grid, or paste it)" onChange={(e) => setCloudUrl(e.target.value)} spellCheck={false} />
        ) : (
          <>
            <label className="inline">
              Colour
              <select value={opts.colourBy} onChange={(e) => setOpts({ ...opts, colourBy: e.target.value as ColourBy })}>
                <option value="indexability">Indexability</option>
                <option value="depth">Crawl Depth</option>
                <option value="response">Response Code</option>
                <option value="segment">Segment</option>
                {kind === "clusters" && <option value="cluster">Cluster</option>}
              </select>
            </label>
            <label className="inline">
              Size
              <select value={opts.sizeBy} onChange={(e) => setOpts({ ...opts, sizeBy: e.target.value as SizeBy })}>
                <option value="fixed">Fixed</option>
                <option value="inlinks">Unique Inlinks</option>
                <option value="link_score">Link Score</option>
              </select>
            </label>
            <label className="inline">
              Max nodes
              <select value={opts.maxNodes} onChange={(e) => setOpts({ ...opts, maxNodes: Number(e.target.value) })}>
                {[500, 1000, 2000, 5000, 10000].map((n) => (
                  <option key={n} value={n}>
                    {n.toLocaleString()}
                  </option>
                ))}
              </select>
            </label>
            <label className="check inline">
              <input type="checkbox" checked={opts.labels} onChange={(e) => setOpts({ ...opts, labels: e.target.checked })} /> Labels
            </label>
          </>
        )}
        <span className="spacer" />
        <button onClick={onClose}>Close</button>
      </div>
      <div className="viz-body">
        {error && <p className="error viz-msg">{error}</p>}
        {!error && !isCloud && !graph && <p className="muted viz-msg">Laying out…</p>}
        {graph && (kind === "crawl-tree" || kind === "directory-tree") && (
          <TreeGraph graph={graph} opts={opts} segments={segmentColours} onPick={onPick} onHover={setHover} />
        )}
        {graph && (kind === "crawl-force" || kind === "directory-force" || kind === "clusters") && (
          <ForceGraph graph={graph} opts={opts} segments={segmentColours} onPick={onPick} onHover={setHover} clusters={kind === "clusters"} />
        )}
        {isCloud && words && <WordCloud words={words} />}
        {isCloud && !cloudUrl.trim() && <p className="muted viz-msg">Select a URL in the grid, or paste one above.</p>}
        {graph?.truncated && <p className="viz-note">Showing the first {opts.maxNodes.toLocaleString()} URLs by crawl depth; raise Max nodes for more.</p>}
        {kind === "clusters" && graph && <ClusterLegend labels={graph.cluster_labels} />}
        {hover && (
          <div className="viz-tip">
            <b>{hover.label}</b>
            <div className="muted">{hover.url}</div>
            {!hover.virtual_node && (
              <div>
                {hover.status || "No response"} · {hover.indexable ? "Indexable" : "Non-Indexable"} · depth {hover.depth} · {hover.unique_inlinks} unique inlinks
                {hover.link_score !== null && ` · Link Score ${hover.link_score}`}
                {hover.segment && ` · ${hover.segment}`}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function ClusterLegend({ labels }: { labels: string[] }) {
  return (
    <div className="viz-legend">
      {labels.map((l, i) => (
        <div key={i}>
          <span className="swatch" style={{ background: CLUSTER_COLOURS[i % CLUSTER_COLOURS.length] }} /> {l}
        </div>
      ))}
    </div>
  );
}

/** Crawl / Directory Tree Graph: horizontal, collapsible, zoomable. */
function TreeGraph({
  graph,
  opts,
  segments,
  onPick,
  onHover,
}: {
  graph: VizGraph;
  opts: Options;
  segments: Record<string, string>;
  onPick: (url: string) => void;
  onHover: (n: VizNode | null) => void;
}) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [collapsed, setCollapsed] = useState<Set<number>>(() => {
    // Big trees start folded below depth 2.
    if (graph.nodes.length <= 300) return new Set();
    return new Set(graph.nodes.filter((n) => n.depth >= 2).map((n) => n.id));
  });
  const [transform, setTransform] = useState<ZoomTransform>(zoomIdentity.translate(60, 0));
  const maxInlinks = useMemo(() => Math.max(1, ...graph.nodes.map((n) => n.unique_inlinks)), [graph]);

  const root = useMemo(() => {
    try {
      const strat = stratify<VizNode>()
        .id((d) => String(d.id))
        .parentId((d) => (d.parent === null ? null : String(d.parent)))(graph.nodes);
      // Hide the children of collapsed nodes.
      const visible = hierarchy(strat, (d) => (collapsed.has(d.data.id) ? null : d.children)) as unknown as HierarchyNode<HierarchyNode<VizNode>>;
      return tree<HierarchyNode<VizNode>>().nodeSize([16, 200]).separation((a, b) => (a.parent === b.parent ? 1 : 1.3))(visible) as HierarchyPointNode<HierarchyNode<VizNode>>;
    } catch {
      return null;
    }
  }, [graph, collapsed]);

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    const height = svg.clientHeight;
    const width = svg.clientWidth;
    const z = zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.05, 4])
      .on("zoom", (e) => setTransform(e.transform));
    // Fit the whole tree on first show.
    let fit = zoomIdentity.translate(60, height / 2);
    if (root) {
      const all = root.descendants();
      const xs = all.map((n) => n.x);
      const ys = all.map((n) => n.y);
      const [minX, maxX] = [Math.min(...xs), Math.max(...xs)];
      const maxY = Math.max(...ys);
      const k = Math.min(1, (height - 60) / Math.max(1, maxX - minX), (width - 260) / Math.max(1, maxY));
      fit = zoomIdentity.translate(60, height / 2 - (k * (minX + maxX)) / 2).scale(k);
    }
    select(svg).call(z).call(z.transform, fit);
    return () => {
      select(svg).on(".zoom", null);
    };
  }, [graph]);

  if (!root) return <p className="error viz-msg">This crawl's tree couldn't be laid out.</p>;
  const nodes = root.descendants();
  const links = root.links();
  return (
    <svg ref={svgRef} className="viz-canvas">
      <g transform={transform.toString()}>
        {links.map((l, i) => (
          <path
            key={i}
            className="viz-link"
            d={`M${l.source.y},${l.source.x}C${(l.source.y + l.target.y) / 2},${l.source.x} ${(l.source.y + l.target.y) / 2},${l.target.x} ${l.target.y},${l.target.x}`}
          />
        ))}
        {nodes.map((n) => {
          const d = n.data.data;
          const hasKids = !!n.data.children?.length;
          const folded = collapsed.has(d.id);
          return (
            <g
              key={d.id}
              transform={`translate(${n.y},${n.x})`}
              className="viz-node"
              onMouseEnter={() => onHover(d)}
              onMouseLeave={() => onHover(null)}
              onClick={(e) => {
                if (e.detail > 1) return;
                if (hasKids) {
                  const next = new Set(collapsed);
                  if (folded) next.delete(d.id);
                  else next.add(d.id);
                  setCollapsed(next);
                }
              }}
              onDoubleClick={() => !d.virtual_node && onPick(d.url)}
            >
              <circle r={radiusOf(d, opts.sizeBy, { inlinks: maxInlinks })} fill={folded ? "#000" : colourOf(d, opts.colourBy, segments)} stroke={colourOf(d, opts.colourBy, segments)} strokeWidth={folded ? 2 : 0} />
              {opts.labels && (
                <text x={10} dy="0.32em">
                  {d.label}
                  {folded && hasKids ? ` (+${n.data.descendants().length - 1})` : ""}
                </text>
              )}
            </g>
          );
        })}
      </g>
      <text x={12} y={20} className="viz-hint">
        Click a node to fold or unfold it · double-click to select the URL · scroll to zoom · drag to pan
      </text>
    </svg>
  );
}

interface SimNode extends SimulationNodeDatum {
  d: VizNode;
  r: number;
  c: string;
}

/** Force-directed crawl / directory diagram and the content cluster diagram, on canvas. */
function ForceGraph({
  graph,
  opts,
  segments,
  onPick,
  onHover,
  clusters,
}: {
  graph: VizGraph;
  opts: Options;
  segments: Record<string, string>;
  onPick: (url: string) => void;
  onHover: (n: VizNode | null) => void;
  clusters: boolean;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const nodesRef = useRef<SimNode[]>([]);
  const transformRef = useRef<ZoomTransform>(zoomIdentity);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    const ctx = canvas.getContext("2d")!;
    const maxInlinks = Math.max(1, ...graph.nodes.map((n) => n.unique_inlinks));
    const nodes: SimNode[] = graph.nodes.map((d) => ({ d, r: radiusOf(d, opts.sizeBy, { inlinks: maxInlinks }), c: colourOf(d, opts.colourBy, segments) }));
    const byId = new Map(nodes.map((n) => [n.d.id, n]));
    const linkData: SimulationLinkDatum<SimNode>[] = clusters
      ? graph.edges.filter(([a, b]) => byId.has(a) && byId.has(b)).map(([a, b, wgt]) => ({ source: byId.get(a)!, target: byId.get(b)!, index: wgt }))
      : nodes.filter((n) => n.d.parent !== null && byId.has(n.d.parent)).map((n) => ({ source: byId.get(n.d.parent!)!, target: n }));
    nodesRef.current = nodes;

    const draw = () => {
      const t = transformRef.current;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);
      ctx.save();
      ctx.translate(t.x, t.y);
      ctx.scale(t.k, t.k);
      ctx.strokeStyle = "rgba(255,255,255,0.12)";
      ctx.lineWidth = 1 / t.k;
      ctx.beginPath();
      for (const l of linkData) {
        const s = l.source as SimNode;
        const g = l.target as SimNode;
        ctx.moveTo(s.x!, s.y!);
        ctx.lineTo(g.x!, g.y!);
      }
      ctx.stroke();
      for (const n of nodes) {
        ctx.beginPath();
        ctx.arc(n.x!, n.y!, n.r, 0, Math.PI * 2);
        ctx.fillStyle = n.c;
        ctx.fill();
      }
      if (opts.labels && t.k > 0.8) {
        ctx.fillStyle = "rgba(255,255,255,0.85)";
        ctx.font = `${11 / t.k}px "DM Sans", sans-serif`;
        for (const n of nodes) {
          if (n.r >= 6 || t.k > 2) ctx.fillText(n.d.label, n.x! + n.r + 2, n.y! + 3 / t.k);
        }
      }
      ctx.restore();
    };

    const sim = forceSimulation(nodes)
      .force("link", forceLink(linkData).distance(clusters ? 40 : 30).strength(clusters ? 0.3 : 0.8))
      .force("charge", forceManyBody().strength(nodes.length > 2000 ? -12 : -40))
      .force("center", forceCenter(w / 2, h / 2))
      .force("collide", forceCollide<SimNode>((n) => n.r + 1))
      .on("tick", draw);

    const z = zoom<HTMLCanvasElement, unknown>()
      .scaleExtent([0.05, 8])
      .on("zoom", (e) => {
        transformRef.current = e.transform;
        draw();
      });
    select(canvas).call(z);

    const nodeAt = (ev: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      const [x, y] = transformRef.current.invert([ev.clientX - rect.left, ev.clientY - rect.top]);
      let best: SimNode | null = null;
      let bestD = Infinity;
      for (const n of nodes) {
        const dd = (n.x! - x) ** 2 + (n.y! - y) ** 2;
        if (dd < bestD && dd <= (n.r + 4) ** 2) {
          best = n;
          bestD = dd;
        }
      }
      return best;
    };
    const move = (ev: MouseEvent) => onHover(nodeAt(ev)?.d ?? null);
    const dbl = (ev: MouseEvent) => {
      const n = nodeAt(ev);
      if (n && !n.d.virtual_node) onPick(n.d.url);
    };
    canvas.addEventListener("mousemove", move);
    canvas.addEventListener("dblclick", dbl);
    return () => {
      sim.stop();
      select(canvas).on(".zoom", null);
      canvas.removeEventListener("mousemove", move);
      canvas.removeEventListener("dblclick", dbl);
    };
  }, [graph, opts.colourBy, opts.sizeBy, opts.labels, clusters]);

  return (
    <>
      <canvas ref={canvasRef} className="viz-canvas" />
      <div className="viz-hint-html">Hover for details · double-click to select the URL · scroll to zoom · drag to pan</div>
    </>
  );
}

/** Word clouds: words sized by frequency. */
function WordCloud({ words }: { words: [string, number][] }) {
  if (words.length === 0) return <p className="muted viz-msg">No words found.</p>;
  const max = words[0][1];
  const min = words[words.length - 1][1];
  // Shuffle deterministically so big words aren't all first.
  const shuffled = [...words].sort((a, b) => ((a[0].charCodeAt(0) * 31 + a[0].length) % 7) - ((b[0].charCodeAt(0) * 31 + b[0].length) % 7));
  return (
    <div className="viz-cloud">
      {shuffled.map(([w, n]) => {
        const t = max === min ? 1 : (Math.log(n) - Math.log(min)) / (Math.log(max) - Math.log(min));
        return (
          <span key={w} title={`${w}: ${n}`} style={{ fontSize: `${13 + t * 42}px`, opacity: 0.55 + t * 0.45, color: t > 0.6 ? ACCENT : t > 0.3 ? "#f5d08a" : "#ffffff" }}>
            {w}
          </span>
        );
      })}
    </div>
  );
}
