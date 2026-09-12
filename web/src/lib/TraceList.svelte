<script lang="ts">
  import { onMount, tick } from "svelte";
  import { goto, afterNavigate } from "$app/navigation";
  import Icon from "./Icon.svelte";
  import { api, date, duration, relative, type Trace } from "./api";
  let { active, notify }: { active: boolean; notify: (s: string) => void } =
    $props();
  let rows = $state<Trace[]>([]),
    loading = $state(true),
    error = $state(""),
    q = $state(""),
    status = $state("all"),
    spanType = $state("all"),
    model = $state(""),
    range = $state("7d");
  let cursor = $state<string | null>(null),
    next = $state<string | null>(null),
    history = $state<(string | null)[]>([]),
    fresh = $state(false),
    live = $state(true),
    request = 0;
  let since = $state<number | null>(null),
    until = $state<number | null>(null),
    customFrom = $state(""),
    customTo = $state("");
  let tableElement = $state<HTMLDivElement | null>(null);
  let savedScroll = 0;
  $effect(() => {
    if (active && !loading && tableElement) {
      tick().then(() => {
        if (active && tableElement) tableElement.scrollTop = savedScroll;
      });
    }
  });
  function readScroll(href: string) {
    try {
      const state = JSON.parse(
        sessionStorage.getItem("trace-list-position") || "{}",
      );
      savedScroll =
        state.href === href && Number.isFinite(state.scroll) ? state.scroll : 0;
    } catch {
      savedScroll = 0;
    }
  }
  const filtered = $derived(
    !!q || status !== "all" || spanType !== "all" || !!model || range !== "7d",
  );
  function params() {
    const p = new URLSearchParams();
    if (q) p.set("q", q);
    if (status !== "all") p.set("status", status);
    if (spanType !== "all") p.set("spanType", spanType);
    if (model) p.set("model", model);
    if (since !== null) p.set("since", String(since));
    if (until !== null) p.set("until", String(until));
    if (cursor) p.set("cursor", cursor);
    p.set("range", range);
    return p;
  }
  async function load(background = false) {
    const seq = ++request;
    if (!background) {
      loading = true;
      error = "";
    }
    try {
      const data = await api<{ items: Trace[]; nextCursor: string | null }>(
        "/api/traces?" + params(),
      );
      if (seq !== request) return;
      if (background && JSON.stringify(rows) !== JSON.stringify(data.items))
        fresh = true;
      else if (!background) {
        rows = data.items;
        next = data.nextCursor;
        fresh = false;
      }
    } catch (e) {
      if (seq === request && !background) error = (e as Error).message;
    } finally {
      if (seq === request) loading = false;
    }
  }
  function dateRange() {
    const ms: Record<string, number> = {
      "1h": 3600000,
      "24h": 86400000,
      "7d": 7 * 86400000,
      "30d": 30 * 86400000,
    };
    since =
      range === "custom"
        ? customFrom
          ? new Date(customFrom).getTime()
          : null
        : ms[range]
          ? Date.now() - ms[range]
          : null;
    until =
      range === "custom" && customTo ? new Date(customTo).getTime() : null;
  }
  async function apply() {
    if (range === "custom" && customFrom && customTo && customFrom > customTo) {
      notify("The start date must be before the end date.");
      return;
    }
    cursor = null;
    history = [];
    savedScroll = 0;
    dateRange();
    await goto("/traces?" + params(), {
      replaceState: true,
      noScroll: true,
      keepFocus: true,
    });
    await load();
  }
  async function clear() {
    q = "";
    status = spanType = "all";
    model = "";
    range = "7d";
    await apply();
  }
  async function turn(forward: boolean) {
    savedScroll = 0;
    if (forward && next) {
      history = [...history, cursor];
      cursor = next;
    } else {
      cursor = history.at(-1) ?? null;
      history = history.slice(0, -1);
    }
    await goto("/traces?" + params(), { replaceState: true, noScroll: true });
    await load();
  }
  function open(trace: Trace) {
    savedScroll = tableElement?.scrollTop ?? 0;
    sessionStorage.setItem(
      "trace-list-position",
      JSON.stringify({
        href: location.pathname + location.search,
        scroll: savedScroll,
      }),
    );
    sessionStorage.setItem(
      "trace-list-href",
      location.pathname + location.search,
    );
    goto("/traces/" + encodeURIComponent(trace.id));
  }
  let mounted = false;
  function restore(p: URLSearchParams) {
    q = p.get("q") || "";
    status = p.get("status") || "all";
    spanType = p.get("spanType") || "all";
    model = p.get("model") || "";
    range = p.get("range") || "7d";
    cursor = p.get("cursor");
    dateRange();
    if (p.has("since")) since = Number(p.get("since"));
    if (p.has("until")) until = Number(p.get("until"));

    const localDate = (n: number | null) =>
      n === null || !Number.isFinite(n)
        ? ""
        : new Date(n - new Date(n).getTimezoneOffset() * 60000)
            .toISOString()
            .slice(0, 16);
    if (range === "custom") {
      customFrom = localDate(since);
      customTo = localDate(until);
    }
  }
  afterNavigate(({ to }) => {
    if (!mounted || to?.url.pathname !== "/traces") return;
    const p = to.url.searchParams;
    if (
      (p.get("q") || "") !== q ||
      (p.get("status") || "all") !== status ||
      (p.get("spanType") || "all") !== spanType ||
      (p.get("model") || "") !== model ||
      (p.get("range") || "7d") !== range ||
      p.get("cursor") !== cursor ||
      (p.has("since") && Number(p.get("since")) !== since) ||
      (p.has("until") && Number(p.get("until")) !== until)
    ) {
      restore(p);
      readScroll(to.url.pathname + to.url.search);
      load();
    }
  });
  onMount(() => {
    const href = location.pathname.startsWith("/traces/")
      ? sessionStorage.getItem("trace-list-href") || "/traces"
      : location.pathname + location.search;
    restore(new URL(href, location.origin).searchParams);
    readScroll(href);
    mounted = true;
    load();
    const timer = setInterval(() => {
      if (active && live && !loading && !document.hidden) load(true);
    }, 5000);
    return () => {
      mounted = false;
      request++;
      clearInterval(timer);
    };
  });
</script>

<section class="list-page">
  <div class="page-heading">
    <div>
      <div class="eyebrow">OBSERVABILITY</div>
      <h1>Traces</h1>
      <p>A closer look at every agent run.</p>
    </div>
    <div class="heading-actions">
      <button class="button subtle" class:live onclick={() => (live = !live)}
        ><Icon name={live ? "pause" : "play"} size={14} />{live
          ? "Live updates"
          : "Updates paused"}</button
      ><button class="button" onclick={() => load()} disabled={loading}
        ><Icon name="refresh" size={15} />Refresh</button
      >
    </div>
  </div>
  <form
    class="filterbar"
    onsubmit={(e) => {
      e.preventDefault();
      apply();
    }}
  >
    <label class="search-input"
      ><Icon name="search" size={17} /><input
        aria-label="Search traces"
        placeholder="Search traces, workflows, errors…"
        bind:value={q}
      /><kbd>↵</kbd></label
    ><label class="select-wrap"
      ><Icon name="clock" size={15} /><select
        aria-label="Time range"
        bind:value={range}
        onchange={() => {
          if (range !== "custom") apply();
        }}
        ><option value="1h">Last hour</option><option value="24h"
          >Last 24 hours</option
        ><option value="7d">Last 7 days</option><option value="30d"
          >Last 30 days</option
        ><option value="all">All time</option><option value="custom"
          >Custom range</option
        ></select
      ></label
    ><select aria-label="Status filter" bind:value={status} onchange={apply}
      ><option value="all">All statuses</option><option value="errors"
        >Has errors</option
      ><option value="open">Open / unknown</option><option value="no_errors"
        >No recorded errors</option
      ></select
    ><select aria-label="Span type" bind:value={spanType} onchange={apply}
      ><option value="all">All step types</option
      >{#each ["agent", "generation", "function", "handoff", "guardrail", "response", "mcp_tools", "custom", "unknown"] as type}<option
          value={type}>{type}</option
        >{/each}</select
    ><button class="button filter-submit" type="submit"
      ><Icon name="settings" size={15} />Apply</button
    >
  </form>
  <div class="filter-secondary">
    <label
      >Model <input
        aria-label="Model filter"
        placeholder="Any model"
        bind:value={model}
        onkeydown={(e) => {
          if (e.key === "Enter") apply();
        }}
        onblur={() => {
          if (
            model !== (new URLSearchParams(location.search).get("model") || "")
          )
            apply();
        }}
      /></label
    >{#if range === "custom"}<label
        >From <input
          type="datetime-local"
          aria-label="From date"
          bind:value={customFrom}
        /></label
      ><label
        >To <input
          type="datetime-local"
          aria-label="To date"
          bind:value={customTo}
        /></label
      ><button class="text-button" onclick={apply}>Set range</button
      >{/if}{#if filtered}<button class="text-button" onclick={clear}
        >Clear filters</button
      >{/if}<span class="result-count"
      >{loading ? "Loading…" : `${rows.length} runs on this page`}</span
    >
  </div>
  {#if fresh}<button class="new-data" onclick={() => load()}
      ><Icon name="refresh" size={14} />Updated traces available · Show latest</button
    >{/if}
  {#if error}<div class="notice error" role="alert">
      {error}<button class="button" onclick={() => load()}>Try again</button>
    </div>
  {:else if loading && !rows.length}<div class="table-skeleton">
      {#each Array(8) as _, i (i)}<div class="skeleton-row">
          <i></i><i></i><i></i>
        </div>{/each}
    </div>
  {:else if !rows.length}<div class="empty-state">
      <span class="empty-icon"
        ><Icon name={filtered ? "search" : "trace"} size={28} /></span
      >
      <h2>{filtered ? "No matching traces" : "Your next run starts here"}</h2>
      <p>
        {filtered
          ? "Try another search or widen the time range."
          : "Create an API key and connect your agent to start exploring its runs."}
      </p>
      {#if filtered}<button class="button" onclick={clear}>Clear filters</button
        >{:else}<a class="button primary" href="/keys"
          ><Icon name="key" size={16} />Create an API key</a
        >{/if}
    </div>
  {:else}<div
      class="trace-table-wrap"
      aria-busy={loading}
      bind:this={tableElement}
    >
      <table class="trace-table">
        <thead
          ><tr
            ><th>Workflow / Trace</th><th>Status</th><th>Steps</th><th
              >Duration</th
            ><th>Received</th><th><span class="sr-only">Open</span></th></tr
          ></thead
        ><tbody
          >{#each rows as trace (trace.id)}<tr
              ><td
                ><button class="trace-link" onclick={() => open(trace)}
                  ><span class="workflow-icon"
                    ><Icon name="layers" size={16} /></span
                  ><span
                    ><strong>{trace.workflowName}</strong><span
                      class="mono row-id"
                      >{trace.id}{#if trace.groupId}<span class="group-id">
                          · {trace.groupId}</span
                        >{/if}</span
                    ></span
                  ></button
                ></td
              ><td
                >{#if trace.errorCount}<span class="badge error"
                    ><span class="status-dot"></span>{trace.errorCount}
                    {trace.errorCount === 1 ? "error" : "errors"}</span
                  >{:else if trace.unfinishedCount || !trace.spanCount}<span
                    class="badge"
                    ><span class="status-dot neutral"></span>Open / unknown</span
                  >{:else}<span class="badge success"
                    ><span class="status-dot"></span>No errors</span
                  >{/if}</td
              ><td class="tabular"
                >{trace.spanCount}<span class="cell-sub"
                  >{trace.generationCount} model · {trace.functionCount} tool</span
                ></td
              ><td class="mono">{duration(trace.durationMs)}</td><td
                title={date(trace.lastSeen)}
                >{relative(trace.lastSeen)}<span class="cell-sub"
                  >{date(trace.lastSeen)}</span
                ></td
              ><td
                ><button
                  class="icon-button"
                  aria-label={`Open ${trace.workflowName}`}
                  onclick={() => open(trace)}
                  ><Icon name="arrow" size={16} /></button
                ></td
              ></tr
            >{/each}</tbody
        >
      </table>
    </div>
    <div class="pagination">
      <span
        >Page {history.length + 1}<span class="muted">
          · Newest received first</span
        ></span
      >
      <div>
        <button
          class="button"
          onclick={() => turn(false)}
          disabled={!history.length || loading}
          ><Icon name="back" size={14} />Previous</button
        ><button
          class="button"
          onclick={() => turn(true)}
          disabled={!next || loading}
          >Next<Icon name="arrow" size={14} /></button
        >
      </div>
    </div>{/if}
  <div class="page-footnote">
    <Icon name="activity" size={14} />A trace is a record of an agent run. Each
    step captures a model call, tool, or handoff.
  </div>
</section>
