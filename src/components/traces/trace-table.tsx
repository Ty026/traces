import Link from "next/link";
import { AlertTriangle, ArrowUpRight, CheckCircle2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DeleteTraceButton } from "@/components/traces/delete-trace-button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatDateTime, formatDuration, formatNumber } from "@/lib/traces/format";
import type { TraceSummary } from "@/lib/traces/types";

export function TraceTable({ traces }: { traces: TraceSummary[] }) {
  if (traces.length === 0) {
    return (
      <div className="flex min-h-[280px] items-center justify-center border-y">
        <div className="max-w-sm text-center">
          <div className="text-sm font-medium">No traces found</div>
          <p className="mt-2 text-sm text-muted-foreground">
            Send data to /v1/traces/ingest or adjust the current filters.
          </p>
        </div>
      </div>
    );
  }

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Workflow</TableHead>
          <TableHead>Health</TableHead>
          <TableHead className="text-right">Spans</TableHead>
          <TableHead className="text-right">Generation</TableHead>
          <TableHead className="text-right">Functions</TableHead>
          <TableHead>Duration</TableHead>
          <TableHead>Last Seen</TableHead>
          <TableHead className="w-[96px]" />
        </TableRow>
      </TableHeader>
      <TableBody>
        {traces.map((trace) => (
          <TableRow key={trace.id}>
            <TableCell className="min-w-[280px]">
              <div className="font-medium">{trace.workflowName}</div>
              <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                <code className="font-mono">{trace.id}</code>
                {trace.groupId ? <span>group {trace.groupId}</span> : null}
              </div>
            </TableCell>
            <TableCell>
              {trace.errorCount > 0 ? (
                <Badge variant="destructive" className="gap-1.5">
                  <AlertTriangle className="size-3" />
                  {formatNumber(trace.errorCount)} errors
                </Badge>
              ) : (
                <Badge variant="outline" className="gap-1.5">
                  <CheckCircle2 className="size-3" />
                  clean
                </Badge>
              )}
            </TableCell>
            <TableCell className="text-right tabular-nums">
              {formatNumber(trace.spanCount)}
            </TableCell>
            <TableCell className="text-right tabular-nums">
              {formatNumber(trace.generationCount)}
            </TableCell>
            <TableCell className="text-right tabular-nums">
              {formatNumber(trace.functionCount)}
            </TableCell>
            <TableCell className="whitespace-nowrap tabular-nums">
              {formatDuration(trace.durationMs)}
            </TableCell>
            <TableCell className="whitespace-nowrap text-muted-foreground">
              {formatDateTime(trace.lastSeenAt)}
            </TableCell>
            <TableCell>
              <div className="flex items-center justify-end gap-1">
                <DeleteTraceButton
                  traceId={trace.id}
                  workflowName={trace.workflowName}
                  compact
                />
                <Button asChild variant="ghost" size="icon" aria-label="Open trace">
                  <Link href={`/traces/${encodeURIComponent(trace.id)}`}>
                    <ArrowUpRight className="size-4" />
                  </Link>
                </Button>
              </div>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
