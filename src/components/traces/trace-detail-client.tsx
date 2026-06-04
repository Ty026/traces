"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import {
  AlertTriangle,
  ArrowLeft,
  Bot,
  Boxes,
  Braces,
  ChevronDown,
  ChevronRight,
  Clock3,
  FunctionSquare,
  GitBranch,
  Workflow,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { DeleteTraceButton } from "@/components/traces/delete-trace-button";
import { cn } from "@/lib/utils";
import { formatDateTime, formatDuration } from "@/lib/traces/format";
import type {
  HighlightedCode,
  TraceCodeHighlights,
  TraceDetail,
  TraceSpan,
} from "@/lib/traces/types";

type SpanNode = TraceSpan & {
  children: SpanNode[];
  depth: number;
};

type SpanRow = {
  node: SpanNode;
  offsetMs: number;
  durationMs: number;
};

const SPAN_COLOR: Record<string, string> = {
  agent: "bg-teal-500",
  generation: "bg-violet-500",
  function: "bg-amber-500",
  handoff: "bg-pink-500",
  guardrail: "bg-orange-500",
  custom: "bg-sky-500",
};

function spanBarColor(type: string, hasError: boolean) {
  if (hasError) return "bg-destructive";
  return SPAN_COLOR[type] ?? "bg-zinc-400 dark:bg-zinc-500";
}

export function TraceDetailClient({
  detail,
  highlights,
}: {
  detail: TraceDetail;
  highlights: TraceCodeHighlights;
}) {
  const { roots, flat } = useMemo(() => buildSpanTree(detail.spans), [detail.spans]);
  const [selectedId, setSelectedId] = useState(flat[0]?.id ?? "");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const selectedSpan = flat.find((span) => span.id === selectedId) ?? flat[0] ?? null;
  const errorCount = detail.spans.filter((span) => span.error).length;

  const { rows, traceStart, traceEnd, totalDuration } = useMemo(
    () => computeWaterfall(roots, collapsed),
    [roots, collapsed],
  );

  const toggleCollapse = (id: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <main className="mx-auto flex max-w-[1600px] flex-col px-4 py-4 sm:px-6 lg:h-[calc(100svh-4rem)] lg:overflow-hidden">
      <div className="mb-4 flex shrink-0 items-start justify-between gap-4">
        <div>
          <Button asChild variant="ghost" size="sm" className="-ml-3 mb-2">
            <Link href="/traces">
              <ArrowLeft className="size-4" />
              Traces
            </Link>
          </Button>
          <div className="flex flex-wrap items-center gap-3">
            <h1 className="text-2xl font-semibold tracking-tight">
              {detail.trace.workflowName}
            </h1>
            {errorCount > 0 ? (
              <Badge variant="destructive" className="gap-1.5">
                <AlertTriangle className="size-3" />
                {errorCount} errors
              </Badge>
            ) : (
              <Badge variant="outline">clean</Badge>
            )}
          </div>
          <div className="mt-2 flex flex-wrap gap-3 text-sm text-muted-foreground">
            <code className="break-all font-mono">{detail.trace.id}</code>
            {detail.trace.groupId ? <span>group {detail.trace.groupId}</span> : null}
            <span>last seen {formatDateTime(detail.trace.lastSeenAt)}</span>
          </div>
        </div>
        <DeleteTraceButton
          traceId={detail.trace.id}
          workflowName={detail.trace.workflowName}
          redirectTo="/traces"
        />
      </div>

      <section className="grid shrink-0 gap-3 border-y py-3 sm:grid-cols-4">
        <Metric label="Spans" value={detail.spans.length.toString()} />
        <Metric label="Duration" value={formatDuration(totalDuration)} />
        <Metric
          label="Generations"
          value={detail.spans
            .filter((span) => span.spanType === "generation")
            .length.toString()}
        />
        <Metric label="Functions" value={detail.spans.filter((span) => span.spanType === "function").length.toString()} />
      </section>

      <div className="mt-4 grid min-h-0 gap-5 lg:flex-1 lg:grid-cols-[minmax(0,1fr)_minmax(360px,430px)]">
        <section className="flex min-h-0 flex-col">
          <div className="mb-3 flex shrink-0 items-center justify-between">
            <div>
              <h2 className="text-sm font-semibold">Timeline</h2>
              <p className="text-xs text-muted-foreground">
                Waterfall of spans relative to the trace start. Click a row to inspect.
              </p>
            </div>
            <GitBranch className="size-4 text-muted-foreground" />
          </div>
          <Waterfall
            rows={rows}
            totalDuration={totalDuration ?? 0}
            traceStart={traceStart}
            traceEnd={traceEnd}
            selectedId={selectedSpan?.id ?? ""}
            collapsed={collapsed}
            onSelect={setSelectedId}
            onToggle={toggleCollapse}
          />
        </section>

        <section className="min-h-0">
          {selectedSpan ? (
            <SpanInspector
              span={selectedSpan}
              trace={detail}
              highlights={highlights[selectedSpan.id]}
            />
          ) : (
            <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
              Select a span to inspect details.
            </div>
          )}
        </section>
      </div>
    </main>
  );
}

function Waterfall({
  rows,
  totalDuration,
  traceStart,
  traceEnd,
  selectedId,
  collapsed,
  onSelect,
  onToggle,
}: {
  rows: SpanRow[];
  totalDuration: number;
  traceStart: number;
  traceEnd: number;
  selectedId: string;
  collapsed: Set<string>;
  onSelect: (id: string) => void;
  onToggle: (id: string) => void;
}) {
  if (rows.length === 0) {
    return (
      <div className="rounded-md border px-3 py-10 text-center text-sm text-muted-foreground">
        No spans recorded for this trace.
      </div>
    );
  }

  const safeTotal = totalDuration > 0 ? totalDuration : 1;
  const ticks = buildTicks(safeTotal);

  return (
    <div className="flex min-h-0 flex-col overflow-hidden rounded-md border bg-card lg:flex-1">
      <div className="grid shrink-0 grid-cols-[minmax(130px,1fr)_minmax(72px,1fr)_60px] items-center gap-3 border-b bg-muted/40 px-3 py-2 text-xs font-medium uppercase tracking-wide text-muted-foreground sm:grid-cols-[minmax(220px,320px)_1fr_72px]">
        <div>Span</div>
        <div className="relative hidden h-4 sm:block">
          {ticks.map((tick, i) => {
            const isFirst = i === 0;
            const isLast = i === ticks.length - 1;
            return (
              <div
                key={tick.pct}
                className={cn(
                  "absolute tabular-nums",
                  !isFirst && !isLast && "-translate-x-1/2",
                  isLast && "-translate-x-full",
                )}
                style={{ left: `${tick.pct}%` }}
              >
                {tick.label}
              </div>
            );
          })}
        </div>
        <div className="text-right">Duration</div>
      </div>
      <ScrollArea className="h-[420px] lg:h-auto lg:min-h-0 lg:flex-1">
        <div>
          {rows.map((row) => (
            <WaterfallRow
              key={row.node.id}
              row={row}
              totalDuration={safeTotal}
              traceStart={traceStart}
              traceEnd={traceEnd}
              ticks={ticks}
              selected={row.node.id === selectedId}
              collapsed={collapsed.has(row.node.id)}
              hasChildren={row.node.children.length > 0}
              onSelect={onSelect}
              onToggle={onToggle}
            />
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}

function WaterfallRow({
  row,
  totalDuration,
  traceStart,
  traceEnd,
  ticks,
  selected,
  collapsed,
  hasChildren,
  onSelect,
  onToggle,
}: {
  row: SpanRow;
  totalDuration: number;
  traceStart: number;
  traceEnd: number;
  ticks: { pct: number; label: string }[];
  selected: boolean;
  collapsed: boolean;
  hasChildren: boolean;
  onSelect: (id: string) => void;
  onToggle: (id: string) => void;
}) {
  const { node, offsetMs, durationMs } = row;
  const startVal = dateValue(node.startedAt);
  const endVal = dateValue(node.endedAt);
  const hasTiming = Number.isFinite(startVal) && Number.isFinite(endVal);
  const offsetPct = hasTiming ? (offsetMs / totalDuration) * 100 : 0;
  const widthPct = hasTiming
    ? Math.max((durationMs / totalDuration) * 100, 0.4)
    : 100;
  const barColor = spanBarColor(node.spanType, !!node.error);
  const label = node.name ?? node.model ?? node.spanType;

  void traceStart;
  void traceEnd;

  return (
    <button
      type="button"
      onClick={() => onSelect(node.id)}
      className={cn(
        "group grid w-full grid-cols-[minmax(130px,1fr)_minmax(72px,1fr)_60px] items-center gap-3 border-b border-border/60 px-3 py-1.5 text-left transition-colors hover:bg-muted/60 sm:grid-cols-[minmax(220px,320px)_1fr_72px]",
        selected && "bg-accent/60 hover:bg-accent/60",
      )}
    >
      <div
        className="flex min-w-0 items-center gap-1.5"
        style={{ paddingLeft: `${node.depth * 14}px` }}
      >
        <span
          role={hasChildren ? "button" : undefined}
          tabIndex={hasChildren ? 0 : -1}
          onClick={(e) => {
            if (!hasChildren) return;
            e.stopPropagation();
            onToggle(node.id);
          }}
          onKeyDown={(e) => {
            if (!hasChildren) return;
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              e.stopPropagation();
              onToggle(node.id);
            }
          }}
          className={cn(
            "flex size-4 shrink-0 items-center justify-center rounded-sm text-muted-foreground",
            hasChildren && "hover:bg-muted hover:text-foreground",
          )}
        >
          {hasChildren ? (
            collapsed ? (
              <ChevronRight className="size-3.5" />
            ) : (
              <ChevronDown className="size-3.5" />
            )
          ) : null}
        </span>
        <SpanIcon type={node.spanType} />
        <span className="min-w-0 truncate text-sm font-medium">{label}</span>
        {node.error ? (
          <AlertTriangle className="size-3 shrink-0 text-destructive" />
        ) : null}
      </div>

      <div className="relative h-5">
        {ticks.map((tick) => (
          <div
            key={tick.pct}
            className="absolute top-0 h-full w-px bg-border/40"
            style={{ left: `${tick.pct}%` }}
          />
        ))}
        <div
          className={cn(
            "absolute top-1/2 h-2.5 -translate-y-1/2 rounded-sm transition-opacity",
            barColor,
            !hasTiming && "opacity-30",
            selected ? "opacity-100" : "opacity-85 group-hover:opacity-100",
          )}
          style={{
            left: `${offsetPct}%`,
            width: `${Math.min(widthPct, 100 - offsetPct)}%`,
          }}
          title={`${label} · ${formatDuration(durationMs)}`}
        />
      </div>

      <div className="text-right font-mono text-xs tabular-nums text-muted-foreground">
        {formatDuration(durationMs)}
      </div>
    </button>
  );
}

function buildTicks(totalDuration: number) {
  const steps = [0, 0.25, 0.5, 0.75, 1];
  return steps.map((step) => ({
    pct: step * 100,
    label: formatTickLabel(step * totalDuration),
  }));
}

function formatTickLabel(ms: number) {
  if (!Number.isFinite(ms)) return "";
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(2)} s`;
  return `${(ms / 60_000).toFixed(1)} min`;
}

function computeWaterfall(roots: SpanNode[], collapsed: Set<string>) {
  const allStarts: number[] = [];
  const allEnds: number[] = [];

  const visitAll = (node: SpanNode) => {
    const s = dateValue(node.startedAt);
    const e = dateValue(node.endedAt);
    if (Number.isFinite(s)) allStarts.push(s);
    if (Number.isFinite(e)) allEnds.push(e);
    node.children.forEach(visitAll);
  };
  roots.forEach(visitAll);

  const traceStart = allStarts.length ? Math.min(...allStarts) : 0;
  const traceEnd = allEnds.length ? Math.max(...allEnds) : 0;
  const totalDuration =
    allStarts.length && allEnds.length ? Math.max(0, traceEnd - traceStart) : null;

  const rows: SpanRow[] = [];
  const visit = (node: SpanNode) => {
    const s = dateValue(node.startedAt);
    const offsetMs = Number.isFinite(s) ? s - traceStart : 0;
    const durationMs = node.durationMs ?? 0;
    rows.push({ node, offsetMs, durationMs });
    if (collapsed.has(node.id)) return;
    node.children.forEach(visit);
  };
  roots.forEach(visit);

  return { rows, traceStart, traceEnd, totalDuration };
}

function SpanInspector({
  span,
  trace,
  highlights,
}: {
  span: TraceSpan;
  trace: TraceDetail;
  highlights?: {
    input?: HighlightedCode;
    output?: HighlightedCode;
  };
}) {
  const keyFields = getSpanKeyFields(span);
  const defaultTab = highlights?.input
    ? "input"
    : highlights?.output
      ? "output"
      : "span-data";

  return (
    <div className="flex min-h-[560px] min-w-0 flex-col overflow-hidden rounded-md border bg-card lg:h-full lg:min-h-0">
      <div className="shrink-0 border-b px-4 py-4">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant={span.error ? "destructive" : "secondary"}>
                {span.spanType}
              </Badge>
              {span.model ? <Badge variant="outline">{span.model}</Badge> : null}
            </div>
            <h2 className="mt-3 truncate text-lg font-semibold">
              {span.name ?? span.model ?? span.spanType}
            </h2>
            <code className="mt-1 block truncate font-mono text-xs text-muted-foreground">
              {span.id}
            </code>
          </div>
          <div className="grid grid-cols-2 gap-3 text-right text-sm">
            <div>
              <div className="text-xs text-muted-foreground">Started</div>
              <div className="tabular-nums">{formatDateTime(span.startedAt)}</div>
            </div>
            <div>
              <div className="text-xs text-muted-foreground">Duration</div>
              <div className="tabular-nums">{formatDuration(span.durationMs)}</div>
            </div>
          </div>
        </div>
      </div>

      {keyFields.length > 0 ? (
        <div className="shrink-0 border-b px-4 py-3">
          <div className="mb-2 text-xs font-medium uppercase text-muted-foreground">
            Key fields
          </div>
          <div className="grid gap-2">
            {keyFields.map((field) => (
              <div key={field.label} className="grid grid-cols-[88px_1fr] gap-3 text-sm">
                <div className="text-xs text-muted-foreground">{field.label}</div>
                <div className="min-w-0 whitespace-pre-wrap break-words font-mono text-xs leading-5">
                  {field.value}
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      <Tabs
        key={span.id}
        defaultValue={defaultTab}
        className="flex min-h-0 flex-1 flex-col px-4 py-3"
      >
        <TabsList className="shrink-0 justify-start overflow-x-auto">
          {highlights?.input ? <TabsTrigger value="input">Input</TabsTrigger> : null}
          {highlights?.output ? (
            <TabsTrigger value="output">Output</TabsTrigger>
          ) : null}
          <TabsTrigger value="span-data">Span data</TabsTrigger>
          <TabsTrigger value="raw">Raw item</TabsTrigger>
          <TabsTrigger value="trace">Trace</TabsTrigger>
          {span.error ? <TabsTrigger value="error">Error</TabsTrigger> : null}
        </TabsList>
        {highlights?.input ? (
          <TabsContent value="input" className="min-h-0 flex-1">
            <HighlightedCodeBlock value={highlights.input} />
          </TabsContent>
        ) : null}
        {highlights?.output ? (
          <TabsContent value="output" className="min-h-0 flex-1">
            <HighlightedCodeBlock value={highlights.output} />
          </TabsContent>
        ) : null}
        <TabsContent value="span-data" className="min-h-0 flex-1">
          <JsonBlock value={span.spanData} />
        </TabsContent>
        <TabsContent value="raw" className="min-h-0 flex-1">
          <JsonBlock value={span.raw} />
        </TabsContent>
        <TabsContent value="trace" className="min-h-0 flex-1">
          <JsonBlock value={trace.trace} />
        </TabsContent>
        {span.error ? (
          <TabsContent value="error" className="min-h-0 flex-1">
            <JsonBlock value={span.error} />
          </TabsContent>
        ) : null}
      </Tabs>
    </div>
  );
}

function getSpanKeyFields(span: TraceSpan) {
  const fields = [
    ["Name", span.name],
    ["Model", span.model],
    ["Usage", formatUsage(span.spanData.output)],
    ["Instructions", span.spanData.instructions],
    ["From", span.spanData.from_agent],
    ["To", span.spanData.to_agent],
    ["Tool", span.spanData.tool_name ?? span.spanData.name],
    ["Server", span.spanData.server],
  ] as const;

  const seen = new Set<string>();
  return fields.flatMap(([label, rawValue]) => {
    const value = previewJsonValue(rawValue);
    if (!value || seen.has(`${label}:${value}`)) return [];
    seen.add(`${label}:${value}`);
    return [{ label, value }];
  });
}

function previewJsonValue(value: unknown) {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return truncate(value, 360);
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  const compact = JSON.stringify(value);
  return compact ? truncate(compact, 360) : "";
}

function formatUsage(value: unknown) {
  const usage = findUsage(value);
  if (!usage) return "";

  const inputTokens = readNumber(usage.input_tokens ?? usage.prompt_tokens);
  const outputTokens = readNumber(usage.output_tokens ?? usage.completion_tokens);
  const totalTokens = readNumber(usage.total_tokens);

  const parts = [
    inputTokens !== null ? `${inputTokens} input` : null,
    outputTokens !== null ? `${outputTokens} output` : null,
    totalTokens !== null ? `${totalTokens} total` : null,
  ].filter(Boolean);

  return parts.join(" / ");
}

function findUsage(value: unknown): Record<string, unknown> | null {
  if (isRecord(value)) {
    if (isRecord(value.usage)) return value.usage;
    for (const child of Object.values(value)) {
      const usage = findUsage(child);
      if (usage) return usage;
    }
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      const usage = findUsage(item);
      if (usage) return usage;
    }
  }

  return null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function readNumber(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function truncate(value: string, max: number) {
  const normalized = value.trim();
  if (normalized.length <= max) return normalized;
  return `${normalized.slice(0, max - 1)}…`;
}

function HighlightedCodeBlock({ value }: { value: HighlightedCode }) {
  return (
    <div
      className="trace-code h-[520px] overflow-auto rounded-md border bg-background lg:h-full"
      data-language={value.language}
      dangerouslySetInnerHTML={{ __html: value.html }}
    />
  );
}

function JsonBlock({ value }: { value: unknown }) {
  return (
    <div className="h-[520px] overflow-auto rounded-md border bg-background lg:h-full">
      <pre className="whitespace-pre-wrap break-words p-4 font-mono text-xs leading-5 text-card-foreground">
        {JSON.stringify(value, null, 2)}
      </pre>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-xs uppercase text-muted-foreground">{label}</div>
      <div className="mt-1 text-2xl font-semibold tabular-nums">{value}</div>
    </div>
  );
}

function SpanIcon({ type }: { type: string }) {
  if (type === "agent") {
    return <Bot className="size-4 text-primary" />;
  }
  if (type === "generation") {
    return <Workflow className="size-4 text-primary" />;
  }
  if (type === "function") {
    return <FunctionSquare className="size-4 text-primary" />;
  }
  if (type === "handoff") {
    return <GitBranch className="size-4 text-primary" />;
  }
  if (type === "custom") {
    return <Braces className="size-4 text-primary" />;
  }
  if (type === "guardrail") {
    return <Clock3 className="size-4 text-primary" />;
  }
  return <Boxes className="size-4 text-muted-foreground" />;
}

function buildSpanTree(spans: TraceSpan[]) {
  const byId = new Map<string, SpanNode>();
  const roots: SpanNode[] = [];

  for (const span of spans) {
    byId.set(span.id, { ...span, children: [], depth: 0 });
  }

  for (const node of byId.values()) {
    const parent = node.parentId ? byId.get(node.parentId) : undefined;
    if (parent) {
      parent.children.push(node);
    } else {
      roots.push(node);
    }
  }

  const sortNodes = (nodes: SpanNode[]) => {
    nodes.sort(compareSpans);
    for (const node of nodes) {
      node.children.sort(compareSpans);
      for (const child of node.children) {
        child.depth = node.depth + 1;
      }
      sortNodes(node.children);
    }
  };
  sortNodes(roots);

  const flat: SpanNode[] = [];
  const visit = (node: SpanNode) => {
    flat.push(node);
    node.children.forEach(visit);
  };
  roots.forEach(visit);

  return { roots, flat };
}

function compareSpans(a: TraceSpan, b: TraceSpan) {
  return dateValue(a.startedAt) - dateValue(b.startedAt) || a.id.localeCompare(b.id);
}

function dateValue(value: string | null) {
  return value ? Date.parse(value) : Number.POSITIVE_INFINITY;
}
