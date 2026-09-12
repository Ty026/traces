export type Trace = {
  id: string;
  workflowName: string;
  groupId: string | null;
  firstSeen: number;
  lastSeen: number;
  spanCount: number;
  errorCount: number;
  unfinishedCount: number;
  generationCount: number;
  functionCount: number;
  startedAt: number | null;
  endedAt: number | null;
  durationMs: number | null;
  raw?: unknown;
  metadata?: unknown;
};
export type Span = {
  id: string;
  parentId: string | null;
  spanType: string;
  name: string | null;
  model: string | null;
  startedAt: number | null;
  endedAt: number | null;
  durationMs: number | null;
  hasError: boolean;
  inputTokens: number;
  outputTokens: number;
};
export type User = { id: number; login: string; email: string };
export type Key = {
  id: string;
  name: string;
  prefix: string;
  createdAt: number;
  expiresAt: number | null;
  lastUsedAt: number | null;
  revokedAt: number | null;
};
export type Status = {
  pending: number;
  queuedBytes: number;
  committed: number;
  writerHealthy: boolean;
  maintenanceHealthy: boolean;
  retentionDays: number;
};
export async function api<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const response = await fetch(path, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      "X-Traces-Request": "1",
      ...options.headers,
    },
    credentials: "same-origin",
  });
  if (response.status === 401) {
    window.dispatchEvent(new Event("session-expired"));
    throw new Error("Your session expired. Sign in again.");
  }
  if (!response.ok) {
    const body = await response.json().catch(() => ({}));
    throw new Error(body.error || `Request failed (${response.status})`);
  }
  return response.status === 204 ? (undefined as T) : response.json();
}
export function duration(ms: number | null) {
  if (ms === null) return "—";
  return ms < 1000
    ? `${ms} ms`
    : ms < 60000
      ? `${(ms / 1000).toFixed(2)} s`
      : `${(ms / 60000).toFixed(1)} min`;
}
export function date(ms: number | null) {
  return ms === null
    ? "—"
    : new Date(ms).toLocaleString("en-US", {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
}
export function relative(ms: number) {
  const s = Math.max(0, (Date.now() - ms) / 1000);
  return s < 60
    ? "Just now"
    : s < 3600
      ? `${Math.floor(s / 60)}m ago`
      : s < 86400
        ? `${Math.floor(s / 3600)}h ago`
        : `${Math.floor(s / 86400)}d ago`;
}
export function pretty(value: unknown) {
  return typeof value === "string"
    ? value
    : (JSON.stringify(value, null, 2) ?? "");
}
export async function copy(value: string) {
  await navigator.clipboard.writeText(value);
}
