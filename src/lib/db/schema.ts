import {
  index,
  integer,
  jsonb,
  pgTable,
  text,
  timestamp,
} from "drizzle-orm/pg-core";

export const traces = pgTable(
  "traces",
  {
    id: text("id").primaryKey(),
    workflowName: text("workflow_name").notNull(),
    groupId: text("group_id"),
    metadata: jsonb("metadata").$type<Record<string, unknown>>().notNull(),
    raw: jsonb("raw").$type<Record<string, unknown>>().notNull(),
    firstSeenAt: timestamp("first_seen_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    lastSeenAt: timestamp("last_seen_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (table) => ({
    groupIdIdx: index("traces_group_id_idx").on(table.groupId),
    lastSeenAtIdx: index("traces_last_seen_at_idx").on(table.lastSeenAt),
    workflowNameIdx: index("traces_workflow_name_idx").on(table.workflowName),
  }),
);

export const spans = pgTable(
  "spans",
  {
    id: text("id").primaryKey(),
    traceId: text("trace_id")
      .notNull()
      .references(() => traces.id, { onDelete: "cascade" }),
    parentId: text("parent_id"),
    spanType: text("span_type").notNull(),
    name: text("name"),
    model: text("model"),
    startedAt: timestamp("started_at", { withTimezone: true }),
    endedAt: timestamp("ended_at", { withTimezone: true }),
    durationMs: integer("duration_ms"),
    error: jsonb("error").$type<Record<string, unknown> | null>(),
    spanData: jsonb("span_data").$type<Record<string, unknown>>().notNull(),
    raw: jsonb("raw").$type<Record<string, unknown>>().notNull(),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (table) => ({
    traceIdIdx: index("spans_trace_id_idx").on(table.traceId),
    parentIdIdx: index("spans_parent_id_idx").on(table.parentId),
    spanTypeIdx: index("spans_span_type_idx").on(table.spanType),
    startedAtIdx: index("spans_started_at_idx").on(table.startedAt),
  }),
);

export type TraceRow = typeof traces.$inferSelect;
export type SpanRow = typeof spans.$inferSelect;
