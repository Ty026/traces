import Link from "next/link";
import { RefreshCw, Search } from "lucide-react";

import { AppShell } from "@/components/traces/app-shell";
import { TraceTable } from "@/components/traces/trace-table";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { getKnownSpanTypes, listTraces } from "@/lib/traces/queries";
import type { TraceListFilters } from "@/lib/traces/types";

export const dynamic = "force-dynamic";

type SearchParams = Promise<Record<string, string | string[] | undefined>>;

export default async function TracesPage({
  searchParams,
}: {
  searchParams: SearchParams;
}) {
  const params = await searchParams;
  const filters = parseFilters(params);
  const [traces, spanTypes] = await Promise.all([
    listTraces(filters),
    getKnownSpanTypes(),
  ]);

  return (
    <AppShell>
      <main className="mx-auto max-w-[1600px] px-4 py-6 sm:px-6">
        <div className="mb-5 flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">Traces</h1>
            <p className="mt-1 text-sm text-muted-foreground">
              Inspect agent runs received through the OpenAI-compatible ingest endpoint.
            </p>
          </div>
          <Button asChild variant="outline" size="sm">
            <Link href="/traces">
              <RefreshCw className="size-4" />
              Refresh
            </Link>
          </Button>
        </div>

        <form
          action="/traces"
          className="mb-4 grid gap-3 border-y py-4 lg:grid-cols-[1fr_180px_180px_auto]"
        >
          <label className="relative block">
            <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              name="q"
              defaultValue={filters.q ?? ""}
              placeholder="Search workflow, trace id, or group id"
              className="pl-9"
            />
          </label>
          <Select name="status" defaultValue={filters.status ?? "all"}>
            <SelectTrigger aria-label="Health filter">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All health</SelectItem>
              <SelectItem value="errors">Errors only</SelectItem>
            </SelectContent>
          </Select>
          <Select name="spanType" defaultValue={filters.spanType ?? "all"}>
            <SelectTrigger aria-label="Span type filter">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All span types</SelectItem>
              {spanTypes.map((spanType) => (
                <SelectItem key={spanType} value={spanType}>
                  {spanType}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button type="submit">
            <Search className="size-4" />
            Filter
          </Button>
        </form>

        <section className="overflow-hidden rounded-md border bg-card">
          <TraceTable traces={traces} />
        </section>
      </main>
    </AppShell>
  );
}

function parseFilters(
  params: Record<string, string | string[] | undefined>,
): TraceListFilters {
  const q = firstValue(params.q);
  const status = firstValue(params.status);
  const spanType = firstValue(params.spanType);

  return {
    ...(q ? { q } : {}),
    status: status === "errors" ? "errors" : "all",
    ...(spanType && spanType !== "all" ? { spanType } : {}),
  };
}

function firstValue(value: string | string[] | undefined) {
  if (Array.isArray(value)) {
    return value[0]?.trim();
  }
  return value?.trim();
}
