import { codeToHtml } from "shiki";

import type {
  HighlightedCode,
  TraceCodeHighlights,
  TraceDetail,
} from "./types";

export async function buildTraceHighlights(
  detail: TraceDetail,
): Promise<TraceCodeHighlights> {
  const entries = await Promise.all(
    detail.spans.map(async (span) => {
      const [input, output] = await Promise.all([
        highlightValue(span.spanData.input),
        highlightValue(span.spanData.output),
      ]);

      return [span.id, { input, output }] as const;
    }),
  );

  return Object.fromEntries(entries);
}

async function highlightValue(value: unknown): Promise<HighlightedCode | undefined> {
  if (value === null || value === undefined) return undefined;

  const { code, language } = formatCodeValue(value);
  if (!code.trim()) return undefined;

  const html = await codeToHtml(code, {
    lang: language,
    themes: {
      light: "github-light",
      dark: "github-dark",
    },
    defaultColor: false,
  });

  return { code, html, language };
}

function formatCodeValue(value: unknown): Pick<HighlightedCode, "code" | "language"> {
  if (typeof value === "string") {
    const parsed = tryParseJson(value);
    if (parsed.ok) {
      return { code: JSON.stringify(parsed.value, null, 2), language: "json" };
    }

    return { code: value, language: "text" };
  }

  return { code: JSON.stringify(value, null, 2), language: "json" };
}

function tryParseJson(value: string):
  | { ok: true; value: unknown }
  | { ok: false } {
  try {
    return { ok: true, value: JSON.parse(value) };
  } catch {
    return { ok: false };
  }
}
