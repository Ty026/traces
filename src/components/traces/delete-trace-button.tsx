"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Loader2, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export function DeleteTraceButton({
  traceId,
  workflowName,
  redirectTo,
  compact = false,
}: {
  traceId: string;
  workflowName: string;
  redirectTo?: string;
  compact?: boolean;
}) {
  const router = useRouter();
  const [isDeleting, setIsDeleting] = useState(false);

  const handleDelete = async () => {
    const confirmed = window.confirm(
      `Delete workflow "${workflowName}"?\n\nThis removes the trace and all spans. This cannot be undone.`,
    );

    if (!confirmed) return;

    setIsDeleting(true);

    try {
      const response = await fetch(`/api/traces/${encodeURIComponent(traceId)}`, {
        method: "DELETE",
      });

      if (!response.ok) {
        throw new Error(await readDeleteError(response));
      }

      if (redirectTo) {
        router.replace(redirectTo);
      } else {
        router.refresh();
      }
    } catch (error) {
      window.alert(
        error instanceof Error ? error.message : "Failed to delete trace",
      );
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <Button
      type="button"
      variant={compact ? "ghost" : "outline"}
      size={compact ? "icon" : "sm"}
      onClick={handleDelete}
      disabled={isDeleting}
      aria-label={`Delete ${workflowName}`}
      title={`Delete ${workflowName}`}
      className={cn(
        "text-destructive hover:text-destructive",
        !compact && "border-destructive/30 hover:bg-destructive/10",
      )}
    >
      {isDeleting ? <Loader2 className="animate-spin" /> : <Trash2 />}
      {compact ? null : "Delete"}
    </Button>
  );
}

async function readDeleteError(response: Response) {
  try {
    const body = (await response.json()) as { error?: unknown };
    if (typeof body.error === "string") return body.error;
  } catch {
    /* fall through */
  }

  return `Failed to delete trace (${response.status})`;
}
