import Link from "next/link";
import { Activity, Database, RadioTower } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { ThemeToggle } from "@/components/theme-toggle";

export function AppShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-background text-foreground">
      <header className="sticky top-0 z-20 border-b bg-background/85 backdrop-blur supports-[backdrop-filter]:bg-background/70">
        <div className="mx-auto flex h-14 max-w-[1600px] items-center justify-between px-4 sm:px-6">
          <Link href="/traces" className="flex items-center gap-3">
            <span className="flex size-8 items-center justify-center rounded-md bg-primary text-primary-foreground shadow-sm">
              <Activity className="size-4" />
            </span>
            <div className="leading-tight">
              <div className="text-sm font-semibold">Agent Traces</div>
              <div className="text-xs text-muted-foreground">
                OpenAI-compatible ingest
              </div>
            </div>
          </Link>
          <div className="flex items-center gap-2">
            <div className="hidden items-center gap-2 sm:flex">
              <Badge variant="outline" className="gap-1.5">
                <RadioTower className="size-3" />
                /v1/traces/ingest
              </Badge>
              <Badge variant="muted" className="gap-1.5">
                <Database className="size-3" />
                Postgres
              </Badge>
            </div>
            <ThemeToggle />
          </div>
        </div>
      </header>
      {children}
    </div>
  );
}
