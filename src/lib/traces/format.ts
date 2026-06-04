export function formatDateTime(value: string | null) {
  if (!value) {
    return "n/a";
  }

  return new Intl.DateTimeFormat("en", {
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

export function formatDuration(ms: number | null) {
  if (ms === null || !Number.isFinite(ms)) {
    return "n/a";
  }

  if (ms < 1000) {
    return `${ms} ms`;
  }

  if (ms < 60_000) {
    return `${(ms / 1000).toFixed(2)} s`;
  }

  return `${(ms / 60_000).toFixed(1)} min`;
}

export function formatNumber(value: number) {
  return new Intl.NumberFormat("en").format(value);
}
