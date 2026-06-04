import Link from "next/link";

import { AppShell } from "@/components/traces/app-shell";
import { Button } from "@/components/ui/button";

export default function TraceNotFound() {
  return (
    <AppShell>
      <main className="mx-auto flex min-h-[70vh] max-w-2xl flex-col items-center justify-center px-6 text-center">
        <h1 className="text-2xl font-semibold">Trace not found</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          The trace may not have been ingested yet, or it may have been removed manually.
        </p>
        <Button asChild className="mt-5">
          <Link href="/traces">Back to traces</Link>
        </Button>
      </main>
    </AppShell>
  );
}
