import { asc, eq, sql } from "drizzle-orm";

import { db, schema } from "@/lib/db";
import type {
  TraceDetail,
  TraceListFilters,
  TraceSpan,
  TraceSummary,
} from "./types";

type TraceSummaryRow = {
  id: string;
  workflow_name: string;
  group_id: string | null;
  metadata: Record<string, unknown>;
  first_seen_at: Date;
  last_seen_at: Date;
  started_at: Date | null;
  ended_at: Date | null;
  duration_ms: number | null;
  span_count: number;
  error_count: number;
  generation_count: number;
  function_count: number;
};

export async function listTraces(
  filters: TraceListFilters = {},
): Promise<TraceSummary[]> {
  const q = filters.q?.trim();
  const spanType = filters.spanType?.trim();
  const whereParts = [sql`true`];

  if (q) {
    const like = `%${q}%`;
    whereParts.push(sql`(
      ${schema.traces.id} ilike ${like}
      or ${schema.traces.workflowName} ilike ${like}
      or ${schema.traces.groupId} ilike ${like}
    )`);
  }

  if (filters.status === "errors") {
    whereParts.push(sql`exists (
      select 1 from ${schema.spans} error_spans
      where error_spans.trace_id = ${schema.traces.id}
        and error_spans.error is not null
    )`);
  }

  if (spanType) {
    whereParts.push(sql`exists (
      select 1 from ${schema.spans} typed_spans
      where typed_spans.trace_id = ${schema.traces.id}
        and typed_spans.span_type = ${spanType}
    )`);
  }

  const rows = await db.execute<TraceSummaryRow>(sql`
    select
      ${schema.traces.id} as id,
      ${schema.traces.workflowName} as workflow_name,
      ${schema.traces.groupId} as group_id,
      ${schema.traces.metadata} as metadata,
      ${schema.traces.firstSeenAt} as first_seen_at,
      ${schema.traces.lastSeenAt} as last_seen_at,
      min(${schema.spans.startedAt}) as started_at,
      max(${schema.spans.endedAt}) as ended_at,
      case
        when min(${schema.spans.startedAt}) is null or max(${schema.spans.endedAt}) is null
          then null
        else extract(epoch from (max(${schema.spans.endedAt}) - min(${schema.spans.startedAt}))) * 1000
      end::int as duration_ms,
      count(${schema.spans.id})::int as span_count,
      count(${schema.spans.id}) filter (where ${schema.spans.error} is not null)::int as error_count,
      count(${schema.spans.id}) filter (where ${schema.spans.spanType} = 'generation')::int as generation_count,
      count(${schema.spans.id}) filter (where ${schema.spans.spanType} = 'function')::int as function_count
    from ${schema.traces}
    left join ${schema.spans} on ${schema.spans.traceId} = ${schema.traces.id}
    where ${sql.join(whereParts, sql` and `)}
    group by ${schema.traces.id}
    order by ${schema.traces.lastSeenAt} desc
    limit 100
  `);

  return rows.map((row) => ({
    id: row.id,
    workflowName: row.workflow_name,
    groupId: row.group_id,
    metadata: row.metadata ?? {},
    firstSeenAt: toIso(row.first_seen_at),
    lastSeenAt: toIso(row.last_seen_at),
    startedAt: toNullableIso(row.started_at),
    endedAt: toNullableIso(row.ended_at),
    durationMs: row.duration_ms,
    spanCount: Number(row.span_count),
    errorCount: Number(row.error_count),
    generationCount: Number(row.generation_count),
    functionCount: Number(row.function_count),
  }));
}

export async function getKnownSpanTypes(): Promise<string[]> {
  const rows = await db
    .selectDistinct({ spanType: schema.spans.spanType })
    .from(schema.spans)
    .orderBy(asc(schema.spans.spanType));

  return rows.map((row) => row.spanType);
}

export async function getTraceDetail(
  traceId: string,
): Promise<TraceDetail | null> {
  const traceRows = await db
    .select()
    .from(schema.traces)
    .where(eq(schema.traces.id, traceId))
    .limit(1);

  const trace = traceRows[0];
  if (!trace) {
    return null;
  }

  const spanRows = await db
    .select()
    .from(schema.spans)
    .where(eq(schema.spans.traceId, traceId))
    .orderBy(asc(schema.spans.startedAt), asc(schema.spans.createdAt));

  return {
    trace: {
      id: trace.id,
      workflowName: trace.workflowName,
      groupId: trace.groupId,
      metadata: trace.metadata,
      raw: trace.raw,
      firstSeenAt: toIso(trace.firstSeenAt),
      lastSeenAt: toIso(trace.lastSeenAt),
    },
    spans: spanRows.map<TraceSpan>((span) => ({
      id: span.id,
      traceId: span.traceId,
      parentId: span.parentId,
      spanType: span.spanType,
      name: span.name,
      model: span.model,
      startedAt: toNullableIso(span.startedAt),
      endedAt: toNullableIso(span.endedAt),
      durationMs: span.durationMs,
      error: span.error,
      spanData: span.spanData,
      raw: span.raw,
    })),
  };
}

function toIso(value: Date | string) {
  return value instanceof Date ? value.toISOString() : new Date(value).toISOString();
}

function toNullableIso(value: Date | string | null) {
  return value ? toIso(value) : null;
}
