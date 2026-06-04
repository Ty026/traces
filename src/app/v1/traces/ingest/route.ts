import {
  getIngestAuthError,
  ingestTraceItems,
  parseIngestEnvelope,
} from "@/lib/traces/ingest";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function POST(request: Request) {
  const authError = getIngestAuthError(request);
  if (authError) {
    return authError;
  }

  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return Response.json({ error: "Invalid JSON body" }, { status: 400 });
  }

  const parsed = parseIngestEnvelope(body);
  if (!parsed.success) {
    return Response.json(
      {
        error: "Invalid tracing ingest envelope",
        issues: parsed.error.issues,
      },
      { status: 400 },
    );
  }

  const result = await ingestTraceItems(parsed.data.data);
  return Response.json(result);
}
