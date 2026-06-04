import { eq } from "drizzle-orm";

import { db, schema } from "@/lib/db";

export async function deleteTrace(traceId: string) {
  const deleted = await db
    .delete(schema.traces)
    .where(eq(schema.traces.id, traceId))
    .returning({ id: schema.traces.id });

  return deleted.length > 0;
}
