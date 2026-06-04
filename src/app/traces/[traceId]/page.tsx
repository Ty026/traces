import { notFound } from "next/navigation";

import { AppShell } from "@/components/traces/app-shell";
import { TraceDetailClient } from "@/components/traces/trace-detail-client";
import { buildTraceHighlights } from "@/lib/traces/highlight";
import { getTraceDetail } from "@/lib/traces/queries";

export const dynamic = "force-dynamic";

export default async function TraceDetailPage({
  params,
}: {
  params: Promise<{ traceId: string }>;
}) {
  const { traceId } = await params;
  const detail = await getTraceDetail(decodeURIComponent(traceId));

  if (!detail) {
    notFound();
  }

  const highlights = await buildTraceHighlights(detail);

  return (
    <AppShell>
      <TraceDetailClient detail={detail} highlights={highlights} />
    </AppShell>
  );
}
