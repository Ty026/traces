<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import Content from "./Content.svelte";
  import JsonView from "./JsonView.svelte";
  import {
    api,
    copy,
    date,
    duration,
    pretty,
    type Trace,
    type Span,
  } from "./api";
  let { id, notify }: { id: string; notify: (s: string) => void } = $props();
  let trace = $state<Trace | null>(null),
    spans = $state<Span[]>([]),
    loading = $state(true),
    error = $state(""),
    selected = $state(""),
    raw = $state<Record<string, unknown> | null>(null),
    bodyLoading = $state(false),
    bodyError = $state("");
  let tab = $state("content"),
    collapsed = $state(new Set<string>()),
    scrollTop = $state(0),
    viewport = $state(650),
    treeElement = $state<HTMLDivElement>(null!),
    pane = $state(43),
    resizing = $state(false),
    splitElement = $state<HTMLDivElement>(null!);
  let find = $state(""),
    matches = $state<Set<string> | null>(null),
    searching = $state(false),
    searchTruncated = $state(false),
    searchError = $state(""),
    searchSequence = 0,
    bodySequence = 0,
    updated = $state(false),
    deleting = $state(false),
    confirmDelete = $state(false);
  let listHref = $state("/traces");
  const tabs = [
    ["content", "Input & output"],
    ["data", "Step data"],
    ["raw", "Raw JSON"],
    ["trace", "Trace metadata"],
  ];
  const activeSpan = $derived(spans.find((s) => s.id === selected));
  const data = $derived((raw?.span_data as Record<string, unknown>) || {});
  const totals = $derived(
    spans.reduce(
      (a, s) => ({
        input: a.input + s.inputTokens,
        output: a.output + s.outputTokens,
      }),
      { input: 0, output: 0 },
    ),
  );
  const children = $derived.by(() => {
    const map = new Map<string, Span[]>();
    const ids = new Set(spans.map((s) => s.id));
    for (const span of spans) {
      const parent =
        span.parentId && ids.has(span.parentId) && span.parentId !== span.id
          ? span.parentId
          : "";
      let siblings = map.get(parent);
      if (!siblings) {
        siblings = [];
        map.set(parent, siblings);
      }
      siblings.push(span);
    }
    return map;
  });
  const rows = $derived.by(() => {
    const result: { span: Span; depth: number }[] = [],
      visited = new Set<string>();
    const walk = (roots: Span[]) => {
      const stack = roots.toReversed().map((span) => ({ span, depth: 0 }));
      while (stack.length) {
        const row = stack.pop()!;
        if (visited.has(row.span.id)) continue;
        visited.add(row.span.id);
        result.push(row);
        if (!collapsed.has(row.span.id) || matches)
          for (const child of (children.get(row.span.id) || []).toReversed())
            stack.push({ span: child, depth: Math.min(row.depth + 1, 20) });
      }
    };
    walk(children.get("") || []);
    // Include malformed cyclic components without rendering ordinary collapsed descendants as roots.
    const rootReachable = new Set<string>();
    const stack = [...(children.get("") || [])];
    while (stack.length) {
      const s = stack.pop()!;
      if (rootReachable.has(s.id)) continue;
      rootReachable.add(s.id);
      stack.push(...(children.get(s.id) || []));
    }
    walk(spans.filter((s) => !rootReachable.has(s.id) && !visited.has(s.id)));
    return matches ? result.filter((row) => matches!.has(row.span.id)) : result;
  });
  const rowHeight = 36;
  const start = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - 6));
  const visible = $derived(
    rows.slice(start, start + Math.ceil(viewport / rowHeight) + 12),
  );
  const totalDuration = $derived(
    Math.max(
      1,
      (trace?.endedAt || Date.now()) - (trace?.startedAt || Date.now()),
    ),
  );
  function bar(span: Span) {
    const left =
      span.startedAt && trace?.startedAt
        ? Math.max(
            0,
            Math.min(
              99,
              ((span.startedAt - trace.startedAt) / totalDuration) * 100,
            ),
          )
        : 0;
    const width = Math.max(
      1.5,
      Math.min(100 - left, ((span.durationMs || 0) / totalDuration) * 100),
    );
    return `left:${left}%;width:${width}%`;
  }
  // Groups step types by what they cost: model calls, tool calls, and orchestration.
  function kind(span: Span) {
    if (span.hasError) return "error";
    if (span.spanType === "generation" || span.spanType === "response")
      return "model";
    if (span.spanType === "function" || span.spanType === "mcp_tools")
      return "tool";
    return "agent";
  }
  const kindIcon: Record<string, string> = {
    error: "alert",
    model: "activity",
    tool: "code",
    agent: "layers",
  };
  function plural(n: number, word: string) {
    return `${n.toLocaleString()} ${word}${n === 1 ? "" : "s"}`;
  }
  async function load() {
    try {
      const [t, s] = await Promise.all([
        api<Trace>("/api/traces/" + encodeURIComponent(id)),
        api<{ items: Span[] }>(
          "/api/traces/" + encodeURIComponent(id) + "/spans",
        ),
      ]);
      trace = t;
      spans = s.items;
      updated = false;
      const pick =
        page.url.searchParams.get("span") || selected || spans[0]?.id;
      if (pick) await select(pick, false);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  }
  async function select(spanId: string, url = true) {
    selected = spanId;
    raw = null;
    bodyLoading = true;
    bodyError = "";
    const seq = ++bodySequence;
    if (url)
      await goto(
        `/traces/${encodeURIComponent(id)}?span=${encodeURIComponent(spanId)}`,
        { replaceState: true, noScroll: true, keepFocus: true },
      );
    try {
      const payload = await api<Record<string, unknown>>(
        `/api/traces/${encodeURIComponent(id)}/spans/${encodeURIComponent(spanId)}`,
      );
      if (seq === bodySequence) raw = payload;
    } catch (e) {
      if (seq === bodySequence) bodyError = (e as Error).message;
    } finally {
      if (seq === bodySequence) bodyLoading = false;
    }
  }
  function toggle(spanId: string) {
    const set = new Set(collapsed);
    if (set.has(spanId)) set.delete(spanId);
    else set.add(spanId);
    collapsed = set;
  }
  async function search() {
    const seq = ++searchSequence;
    searchError = "";
    if (!find.trim()) {
      matches = null;
      searchTruncated = false;
      return;
    }
    searching = true;
    try {
      const result = await api<{ ids: string[]; truncated: boolean }>(
        `/api/traces/${encodeURIComponent(id)}/search?q=${encodeURIComponent(find)}`,
      );
      if (seq === searchSequence) {
        matches = new Set(result.ids);
        searchTruncated = result.truncated;
        scrollTop = 0;
        treeElement.scrollTop = 0;
      }
    } catch (e) {
      if (seq === searchSequence) searchError = (e as Error).message;
    } finally {
      if (seq === searchSequence) searching = false;
    }
  }
  async function copied(value: unknown, message = "Copied") {
    try {
      await copy(pretty(value));
      notify(message);
    } catch {
      notify("Couldn't access the clipboard. Select the text and copy it.");
    }
  }
  function keyboard(e: KeyboardEvent) {
    if (
      !["ArrowDown", "ArrowUp", "j", "k", "ArrowLeft", "ArrowRight"].includes(
        e.key,
      )
    )
      return;
    e.preventDefault();
    const index = rows.findIndex((r) => r.span.id === selected);
    if (e.key === "ArrowLeft") {
      collapsed = new Set([...collapsed, selected]);
      return;
    }
    if (e.key === "ArrowRight") {
      const next = new Set(collapsed);
      next.delete(selected);
      collapsed = next;
      return;
    }
    const next = Math.max(
      0,
      Math.min(
        rows.length - 1,
        index + (["ArrowDown", "j"].includes(e.key) ? 1 : -1),
      ),
    );
    if (!rows[next]) return;
    select(rows[next].span.id);
    if (next * rowHeight < treeElement.scrollTop)
      treeElement.scrollTop = next * rowHeight;
    else if ((next + 1) * rowHeight > treeElement.scrollTop + viewport)
      treeElement.scrollTop = (next + 1) * rowHeight - viewport;
  }
  function resize(e: PointerEvent) {
    if (!resizing) return;
    const rect = splitElement.getBoundingClientRect();
    pane = Math.max(
      28,
      Math.min(65, ((e.clientX - rect.left) / rect.width) * 100),
    );
  }
  function endResize() {
    if (resizing) localStorage.setItem("trace-pane", String(pane));
    resizing = false;
  }
  async function remove() {
    deleting = true;
    try {
      await api("/api/traces/" + encodeURIComponent(id), { method: "DELETE" });
      notify("Trace deleted");
      await goto(listHref, { invalidateAll: true });
      location.reload();
    } catch (e) {
      notify((e as Error).message);
    } finally {
      deleting = false;
      confirmDelete = false;
    }
  }
  onMount(() => {
    listHref = sessionStorage.getItem("trace-list-href") || "/traces";
    if (!listHref.startsWith("/traces?") && listHref !== "/traces")
      listHref = "/traces";
    const saved = Number(localStorage.getItem("trace-pane"));
    if (saved >= 28 && saved <= 65) pane = saved;
    load();
    const timer = setInterval(async () => {
      if (!trace || document.hidden) return;
      try {
        const latest = await api<Trace>(
          "/api/traces/" + encodeURIComponent(id),
        );
        updated = latest.lastSeen !== trace.lastSeen;
      } catch {
        /* preserve current inspection during transient failures */
      }
    }, 5000);
    return () => {
      clearInterval(timer);
      bodySequence++;
      searchSequence++;
    };
  });
</script>

<svelte:window onpointermove={resize} onpointerup={endResize} />
<section class="detail-page">
  <a class="back-link" href={listHref}
    ><Icon name="back" size={14} />All traces</a
  >
  {#if loading}<div class="detail-loading">
      <span class="spinner"></span>Loading trace…
    </div>{:else if error}<div class="notice error" role="alert">
      {error}<button
        class="button"
        onclick={() => {
          error = "";
          loading = true;
          load();
        }}>Try again</button
      >
    </div>{:else if trace}
    <div class="detail-heading">
      <div class="detail-title">
        <h1>{trace.workflowName}</h1>
        <button
          class="id-copy mono"
          title="Copy trace ID"
          onclick={() => copied(id, "Trace ID copied")}
          >{id}<Icon name="copy" size={12} /></button
        >
      </div>
      <div class="heading-actions">
        <a
          class="button"
          href={`/api/traces/${encodeURIComponent(id)}/export`}
          download="trace.json"
          ><Icon name="download" size={14} /><span>Export JSON</span></a
        ><button
          class="icon-button danger-hover"
          onclick={() => (confirmDelete = true)}
          title="Delete trace"
          aria-label="Delete trace"><Icon name="trash" size={16} /></button
        >
      </div>
    </div>
    <dl class="trace-stats">
      <div>
        <dt>Status</dt>
        {#if trace.errorCount}<dd class="status error">
            <span class="status-dot"></span>{plural(trace.errorCount, "error")}
          </dd>{:else if trace.unfinishedCount || !trace.spanCount}<dd
            class="status neutral"
            title="No steps yet, or a step has no end time"
          >
            <span class="status-dot"></span>Open
          </dd>{:else}<dd class="status success">
            <span class="status-dot"></span>No errors
          </dd>{/if}
      </div>
      <div>
        <dt>Duration</dt>
        <dd class="mono">{duration(trace.durationMs)}</dd>
      </div>
      <div>
        <dt>Steps</dt>
        <dd class="mono">{trace.spanCount.toLocaleString()}</dd>
      </div>
      <div>
        <dt>Tokens</dt>
        <dd>
          <span class="mono"
            >{(totals.input + totals.output).toLocaleString()}</span
          ><small class="mono"
            >{totals.input.toLocaleString()} in / {totals.output.toLocaleString()}
            out</small
          >
        </dd>
      </div>
      <div>
        <dt>Last update</dt>
        <dd>{date(trace.lastSeen)}</dd>
      </div>
    </dl>
    {#if updated}<button class="new-data" onclick={load}
        ><Icon name="refresh" size={14} />Updates available. Refresh trace</button
      >{/if}
    <div
      class="execution-layout"
      bind:this={splitElement}
      style={`--pane:${pane}%`}
      class:resizing
    >
      <section class="execution-pane">
        <div class="panel-heading">
          <h2>Execution</h2>
          <span>{plural(spans.length, "step")}</span>{#if collapsed.size}<button
              class="text-button"
              onclick={() => (collapsed = new Set())}>Expand all</button
            >{/if}
        </div>
        <form
          class="trace-search"
          onsubmit={(e) => {
            e.preventDefault();
            search();
          }}
        >
          <Icon name="search" size={14} /><input
            aria-label="Find in this trace"
            placeholder="Search step contents"
            bind:value={find}
          />{#if matches}<button
              class="icon-button"
              type="button"
              aria-label="Clear trace search"
              onclick={() => {
                find = "";
                search();
              }}><Icon name="close" size={13} /></button
            >{:else}<button
              class="text-button"
              type="submit"
              disabled={searching}>{searching ? "Finding…" : "Find"}</button
            >{/if}
        </form>
        {#if matches}<div class="search-results">
            {plural(matches.size, "matching step")}{searchTruncated
              ? ". Showing the first 1,000."
              : ""}
          </div>{/if}{#if searchError}<div class="notice error compact">
            {searchError}
          </div>{/if}
        <div class="tree-columns" aria-hidden="true">
          <span class="tree-columns-name">Step</span><span class="ruler-cell"
            ><span class="ruler"
              >{#if trace.startedAt}{#each [0, 0.5, 1] as f (f)}<span
                    style={`left:${f * 100}%`}
                    >{f ? duration(Math.round(totalDuration * f)) : "0"}</span
                  >{/each}{/if}</span
            ><span class="ruler-duration">Duration</span></span
          >
        </div>
        <div
          class="tree-viewport"
          bind:this={treeElement}
          bind:clientHeight={viewport}
          onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
          onkeydown={keyboard}
          tabindex="0"
          role="tree"
          aria-label="Execution steps"
        >
          <div style={`height:${rows.length * rowHeight}px;position:relative`}>
            {#each visible as row, i (row.span.id)}{@const s =
                row.span}{@const k = kind(s)}
              <div
                class={`tree-row kind-${k}`}
                class:selected={selected === s.id}
                style={`position:absolute;top:${(start + i) * rowHeight}px;width:100%;height:${rowHeight}px`}
                role="treeitem"
                aria-selected={selected === s.id}
                aria-level={row.depth + 1}
                aria-expanded={children.has(s.id)
                  ? !collapsed.has(s.id)
                  : undefined}
              >
                <div class="tree-name">
                  {#if row.depth}<span
                      class="indent"
                      style={`width:${row.depth * 16}px`}
                    ></span>{/if}{#if children.has(s.id)}<button
                      class="tree-toggle"
                      class:expanded={!collapsed.has(s.id)}
                      onclick={() => toggle(s.id)}
                      aria-label={`${collapsed.has(s.id) ? "Expand" : "Collapse"} ${s.name || s.spanType}`}
                      ><Icon name="arrow" size={11} /></button
                    >{:else}<span class="tree-toggle"></span>{/if}<button
                    class="step-select"
                    onclick={() => select(s.id)}
                    title={`${s.name || s.spanType}, ${s.spanType}`}
                    ><span class="step-icon"
                      ><Icon name={kindIcon[k]} size={14} /></span
                    ><span>{s.name || s.spanType}</span></button
                  >
                </div>
                <button
                  class="timeline-cell"
                  onclick={() => select(s.id)}
                  aria-label={`Inspect ${s.name || s.spanType}, ${duration(s.durationMs)}`}
                  ><span class="timeline-track"><i style={bar(s)}></i></span
                  ><span class="mono">{duration(s.durationMs)}</span></button
                >
              </div>{/each}
          </div>
          {#if !rows.length}<div class="content-empty">
              {matches
                ? "No steps contain this text."
                : "This trace has no steps yet."}
            </div>{/if}
        </div>
        <div class="tree-footer">
          <span><kbd>↑</kbd><kbd>↓</kbd> move</span><span
            ><kbd>←</kbd><kbd>→</kbd> collapse / expand</span
          >
        </div>
      </section>
      <!-- ARIA window-splitter pattern is intentionally a focusable separator. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div
        class="pane-resizer"
        role="separator"
        aria-label="Resize execution panel"
        aria-orientation="vertical"
        aria-valuemin="28"
        aria-valuemax="65"
        aria-valuenow={Math.round(pane)}
        tabindex="0"
        onpointerdown={(e) => {
          e.preventDefault();
          resizing = true;
        }}
        onkeydown={(e) => {
          if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
            e.preventDefault();
            pane = Math.max(
              28,
              Math.min(65, pane + (e.key === "ArrowLeft" ? -2 : 2)),
            );
            localStorage.setItem("trace-pane", String(pane));
          }
        }}
      ></div>
      <section class="inspector">
        <div class="inspector-heading">
          <div>
            {#if activeSpan}<span
                class={`type-chip mono kind-${kind(activeSpan)}`}
                >{activeSpan.spanType}</span
              >{/if}
            <h2>
              {activeSpan?.name || activeSpan?.spanType || "No step selected"}
            </h2>
            {#if activeSpan}<div class="step-meta mono">
                <span>{duration(activeSpan.durationMs)}</span
                >{#if activeSpan.model}<span>{activeSpan.model}</span
                  >{/if}{#if activeSpan.inputTokens || activeSpan.outputTokens}<span
                    >{activeSpan.inputTokens.toLocaleString()} in / {activeSpan.outputTokens.toLocaleString()}
                    out</span
                  >{/if}
              </div>{/if}
          </div>
          <button
            class="icon-button"
            onclick={() =>
              copied(
                location.origin +
                  `/traces/${encodeURIComponent(id)}?span=${encodeURIComponent(selected)}`,
                "Link to step copied",
              )}
            disabled={!selected}
            title="Copy link to this step"
            aria-label="Copy step link"><Icon name="link" size={16} /></button
          >
        </div>
        <div class="tabs" role="tablist" aria-label="Step detail tabs">
          {#each tabs as [t, label] (t)}<button
              role="tab"
              aria-selected={tab === t}
              class:active={tab === t}
              onclick={() => (tab = t)}>{label}</button
            >{/each}
        </div>
        <div
          class="inspector-body"
          role="tabpanel"
          aria-label="Step details"
          aria-busy={bodyLoading}
        >
          {#if tab === "trace"}<div class="content-section">
              <div class="section-title">
                <h3>Metadata</h3>
                <button
                  class="icon-button"
                  aria-label="Copy trace metadata"
                  onclick={() => copied(trace?.metadata)}
                  ><Icon name="copy" size={14} /></button
                >
              </div>
              <JsonView value={trace.metadata} />
            </div>
            <div class="content-section">
              <div class="section-title">
                <h3>Original trace record</h3>
                <button
                  class="icon-button"
                  aria-label="Copy original trace record"
                  onclick={() => copied(trace?.raw)}
                  ><Icon name="copy" size={14} /></button
                >
              </div>
              <JsonView value={trace.raw} />
            </div>
          {:else if bodyLoading}<div class="content-loading">
              <span class="spinner"></span>Loading step…
            </div>{:else if bodyError}<div class="notice error">
              {bodyError}<button
                class="text-button"
                onclick={() => select(selected, false)}>Try again</button
              >
            </div>{:else if !raw}<div class="content-empty">
              Select a step to see its input and output.
            </div>
          {:else if tab === "raw" || tab === "data"}<div
              class="content-section"
            >
              <div class="section-title">
                <h3>{tab === "raw" ? "Original span record" : "span_data"}</h3>
                <button
                  class="icon-button"
                  aria-label="Copy JSON"
                  onclick={() => copied(tab === "raw" ? raw : data)}
                  ><Icon name="copy" size={14} /></button
                >
              </div>
              <JsonView value={tab === "raw" ? raw : data} />
            </div>
          {:else}
            {#if raw.error}<div class="step-error">
                <div class="section-title">
                  <h3><Icon name="alert" size={15} />Error</h3>
                  <button
                    class="icon-button"
                    aria-label="Copy error"
                    onclick={() => copied(raw?.error)}
                    ><Icon name="copy" size={14} /></button
                  >
                </div>
                <JsonView value={raw.error} />
              </div>{/if}
            {#each ["input", "output"] as part (part)}<div
                class="content-section"
              >
                <div class="section-title">
                  <h3>{part === "input" ? "Input" : "Output"}</h3>
                  <button
                    class="icon-button"
                    aria-label={`Copy ${part}`}
                    onclick={() => copied(data[part])}
                    ><Icon name="copy" size={14} /></button
                  >
                </div>
                <Content value={data[part]} />
              </div>{/each}
            {#if data.input === undefined && data.output === undefined}<div
                class="content-section"
              >
                <div class="section-title"><h3>span_data</h3></div>
                <JsonView value={data} />
              </div>{/if}
          {/if}
        </div>
      </section>
    </div>
  {/if}
</section>
{#if confirmDelete}<Modal
    titleId="delete-title"
    onclose={() => {
      if (!deleting) confirmDelete = false;
    }}
    ><h2 id="delete-title">Delete this trace?</h2>
    <p>
      This deletes "{trace?.workflowName}" and its {plural(
        trace?.spanCount ?? 0,
        "step",
      )}. If the agent sends more data with this trace ID, the trace will appear
      again.
    </p>
    <div class="modal-actions">
      <button
        class="button"
        onclick={() => (confirmDelete = false)}
        disabled={deleting}>Cancel</button
      ><button class="button destructive" onclick={remove} disabled={deleting}
        >{deleting ? "Deleting…" : "Delete trace"}</button
      >
    </div></Modal
  >{/if}
