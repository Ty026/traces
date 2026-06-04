import { deleteTrace } from "@/lib/traces/mutations";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function DELETE(
  _request: Request,
  { params }: { params: Promise<{ traceId: string }> },
) {
  const { traceId } = await params;
  const decodedTraceId = decodeURIComponent(traceId);

  if (!decodedTraceId) {
    return Response.json({ error: "traceId is required" }, { status: 400 });
  }

  try {
    const deleted = await deleteTrace(decodedTraceId);

    if (!deleted) {
      return Response.json({ error: "Trace not found" }, { status: 404 });
    }

    return new Response(null, { status: 204 });
  } catch {
    return Response.json({ error: "Failed to delete trace" }, { status: 500 });
  }
}
