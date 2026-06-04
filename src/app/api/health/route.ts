import { sql } from "drizzle-orm";

import { db } from "@/lib/db";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET() {
  try {
    await db.execute(sql`select 1`);

    return Response.json({
      ok: true,
      database: "ok",
      ingestAuthConfigured: Boolean(process.env.TRACE_INGEST_TOKEN),
    });
  } catch (error) {
    return Response.json(
      {
        ok: false,
        database: "error",
        error: error instanceof Error ? error.message : "Unknown error",
      },
      { status: 500 },
    );
  }
}
