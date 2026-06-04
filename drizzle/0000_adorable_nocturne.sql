CREATE TABLE "spans" (
	"id" text PRIMARY KEY NOT NULL,
	"trace_id" text NOT NULL,
	"parent_id" text,
	"span_type" text NOT NULL,
	"name" text,
	"model" text,
	"started_at" timestamp with time zone,
	"ended_at" timestamp with time zone,
	"duration_ms" integer,
	"error" jsonb,
	"span_data" jsonb NOT NULL,
	"raw" jsonb NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "traces" (
	"id" text PRIMARY KEY NOT NULL,
	"workflow_name" text NOT NULL,
	"group_id" text,
	"metadata" jsonb NOT NULL,
	"raw" jsonb NOT NULL,
	"first_seen_at" timestamp with time zone DEFAULT now() NOT NULL,
	"last_seen_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
ALTER TABLE "spans" ADD CONSTRAINT "spans_trace_id_traces_id_fk" FOREIGN KEY ("trace_id") REFERENCES "public"."traces"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "spans_trace_id_idx" ON "spans" USING btree ("trace_id");--> statement-breakpoint
CREATE INDEX "spans_parent_id_idx" ON "spans" USING btree ("parent_id");--> statement-breakpoint
CREATE INDEX "spans_span_type_idx" ON "spans" USING btree ("span_type");--> statement-breakpoint
CREATE INDEX "spans_started_at_idx" ON "spans" USING btree ("started_at");--> statement-breakpoint
CREATE INDEX "traces_group_id_idx" ON "traces" USING btree ("group_id");--> statement-breakpoint
CREATE INDEX "traces_last_seen_at_idx" ON "traces" USING btree ("last_seen_at");--> statement-breakpoint
CREATE INDEX "traces_workflow_name_idx" ON "traces" USING btree ("workflow_name");