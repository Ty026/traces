export type JsonValue =
  | null
  | string
  | number
  | boolean
  | JsonValue[]
  | { [key: string]: JsonValue };

export type TraceSummary = {
  id: string;
  workflowName: string;
  groupId: string | null;
  metadata: Record<string, unknown>;
  firstSeenAt: string;
  lastSeenAt: string;
  startedAt: string | null;
  endedAt: string | null;
  durationMs: number | null;
  spanCount: number;
  errorCount: number;
  generationCount: number;
  functionCount: number;
};

export type TraceSpan = {
  id: string;
  traceId: string;
  parentId: string | null;
  spanType: string;
  name: string | null;
  model: string | null;
  startedAt: string | null;
  endedAt: string | null;
  durationMs: number | null;
  error: Record<string, unknown> | null;
  spanData: Record<string, unknown>;
  raw: Record<string, unknown>;
};

export type TraceDetail = {
  trace: {
    id: string;
    workflowName: string;
    groupId: string | null;
    metadata: Record<string, unknown>;
    raw: Record<string, unknown>;
    firstSeenAt: string;
    lastSeenAt: string;
  };
  spans: TraceSpan[];
};

export type HighlightedCode = {
  code: string;
  html: string;
  language: "json" | "text";
};

export type TraceCodeHighlights = Record<
  string,
  {
    input?: HighlightedCode;
    output?: HighlightedCode;
  }
>;

export type TraceListFilters = {
  q?: string;
  status?: "all" | "errors";
  spanType?: string;
};
