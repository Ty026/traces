import { sql } from "drizzle-orm";
import { z } from "zod";

import { db, schema } from "@/lib/db";

const recordSchema = z.record(z.string(), z.unknown());

const ingestEnvelopeSchema = z.object({
  data: z.array(recordSchema).max(1000),
});

type RawTraceItem = z.infer<typeof recordSchema>;

type NormalizedTrace =
  | {
      kind: "trace";
      id: string;
      workflowName: string;
      groupId: string | null;
      metadata: Record<string, unknown>;
      raw: RawTraceItem;
    }
  | {
      kind: "span";
      id: string;
      traceId: string;
      parentId: string | null;
      spanType: string;
      name: string | null;
      model: string | null;
      startedAt: Date | null;
      endedAt: Date | null;
      durationMs: number | null;
      error: Record<string, unknown> | null;
      spanData: Record<string, unknown>;
      raw: RawTraceItem;
    };

export type IngestResult = {
  accepted: number;
  rejected: number;
};

export function parseIngestEnvelope(body: unknown) {
  return ingestEnvelopeSchema.safeParse(body);
}

export function getIngestAuthError(request: Request): Response | null {
  const expectedToken = process.env.TRACE_INGEST_TOKEN;

  if (!expectedToken) {
    return Response.json(
      { error: "TRACE_INGEST_TOKEN is not configured" },
      { status: 500 },
    );
  }

  const authorization = request.headers.get("authorization") ?? "";
  const token = authorization.startsWith("Bearer ")
    ? authorization.slice("Bearer ".length).trim()
    : "";

  if (token !== expectedToken) {
    return Response.json({ error: "Unauthorized" }, { status: 401 });
  }

  return null;
}

export async function ingestTraceItems(
  rawItems: RawTraceItem[],
): Promise<IngestResult> {
  let accepted = 0;
  let rejected = 0;
  const now = new Date();

  await db.transaction(async (tx) => {
    for (const rawItem of rawItems) {
      const item = normalizeTraceItem(rawItem);
      if (!item) {
        rejected += 1;
        continue;
      }

      if (item.kind === "trace") {
        await tx
          .insert(schema.traces)
          .values({
            id: item.id,
            workflowName: item.workflowName,
            groupId: item.groupId,
            metadata: item.metadata,
            raw: item.raw,
            firstSeenAt: now,
            lastSeenAt: now,
          })
          .onConflictDoUpdate({
            target: schema.traces.id,
            set: {
              workflowName: item.workflowName,
              groupId: item.groupId,
              metadata: item.metadata,
              raw: item.raw,
              lastSeenAt: now,
            },
          });
      } else {
        await tx
          .insert(schema.traces)
          .values({
            id: item.traceId,
            workflowName: "Unknown workflow",
            groupId: null,
            metadata: {},
            raw: { placeholder: true, trace_id: item.traceId },
            firstSeenAt: now,
            lastSeenAt: now,
          })
          .onConflictDoUpdate({
            target: schema.traces.id,
            set: {
              lastSeenAt: now,
            },
          });

        await tx
          .insert(schema.spans)
          .values({
            id: item.id,
            traceId: item.traceId,
            parentId: item.parentId,
            spanType: item.spanType,
            name: item.name,
            model: item.model,
            startedAt: item.startedAt,
            endedAt: item.endedAt,
            durationMs: item.durationMs,
            error: item.error,
            spanData: item.spanData,
            raw: item.raw,
            createdAt: now,
          })
          .onConflictDoUpdate({
            target: schema.spans.id,
            set: {
              traceId: item.traceId,
              parentId: item.parentId,
              spanType: item.spanType,
              name: item.name,
              model: item.model,
              startedAt: item.startedAt,
              endedAt: item.endedAt,
              durationMs: item.durationMs,
              error: item.error,
              spanData: item.spanData,
              raw: item.raw,
            },
          });

        await tx
          .update(schema.traces)
          .set({ lastSeenAt: now })
          .where(sql`${schema.traces.id} = ${item.traceId}`);
      }

      accepted += 1;
    }
  });

  return { accepted, rejected };
}

function normalizeTraceItem(rawItem: RawTraceItem): NormalizedTrace | null {
  if (rawItem.object === "trace") {
    const id = readString(rawItem.id);
    if (!id) {
      return null;
    }

    return {
      kind: "trace",
      id,
      workflowName: readString(rawItem.workflow_name) ?? "Agent workflow",
      groupId: readNullableString(rawItem.group_id),
      metadata: readRecord(rawItem.metadata) ?? {},
      raw: rawItem,
    };
  }

  if (rawItem.object === "trace.span") {
    const id = readString(rawItem.id);
    const traceId = readString(rawItem.trace_id);
    const spanData = readRecord(rawItem.span_data);
    if (!id || !traceId || !spanData) {
      return null;
    }

    const startedAt = readDate(rawItem.started_at);
    const endedAt = readDate(rawItem.ended_at);

    return {
      kind: "span",
      id,
      traceId,
      parentId: readNullableString(rawItem.parent_id),
      spanType: readString(spanData.type) ?? "unknown",
      name: inferSpanName(spanData),
      model: readString(spanData.model),
      startedAt,
      endedAt,
      durationMs: calculateDurationMs(startedAt, endedAt),
      error: readRecord(rawItem.error),
      spanData,
      raw: rawItem,
    };
  }

  return null;
}

function inferSpanName(spanData: Record<string, unknown>): string | null {
  const explicitName = readString(spanData.name);
  if (explicitName) {
    return explicitName;
  }

  const type = readString(spanData.type);
  if (type === "generation") {
    return readString(spanData.model);
  }
  if (type === "handoff") {
    const fromAgent = readString(spanData.from_agent);
    const toAgent = readString(spanData.to_agent);
    if (fromAgent || toAgent) {
      return [fromAgent, toAgent].filter(Boolean).join(" -> ");
    }
  }
  if (type === "mcp_tools") {
    return readString(spanData.server);
  }

  return null;
}

function readString(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value : null;
}

function readNullableString(value: unknown): string | null {
  if (value === null || value === undefined) {
    return null;
  }
  return readString(value);
}

function readRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function readDate(value: unknown): Date | null {
  if (typeof value !== "string") {
    return null;
  }

  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) {
    return null;
  }

  return new Date(timestamp);
}

function calculateDurationMs(startedAt: Date | null, endedAt: Date | null) {
  if (!startedAt || !endedAt) {
    return null;
  }

  return Math.max(0, endedAt.getTime() - startedAt.getTime());
}
