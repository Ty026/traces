<script lang="ts">
  import JsonView from "./JsonView.svelte";
  import LongText from "./LongText.svelte";
  import { pretty } from "./api";
  let { value: original }: { value: unknown } = $props();
  const value = $derived.by(() => {
    if (typeof original === "string" && /^[\[{]/.test(original.trim())) {
      try {
        return JSON.parse(original);
      } catch {
        /* Literal text remains readable. */
      }
    }
    return original;
  });
  let messageLimit = $state(100);
  const messages = $derived(
    Array.isArray(value)
      ? value
      : value && typeof value === "object" && "role" in value
        ? [value]
        : null,
  );
  function object(value: unknown): Record<string, unknown> | null {
    return value && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : null;
  }
</script>

{#if value === null || value === undefined || value === ""}
  <div class="content-empty">Not recorded</div>
{:else if messages}
  {#each messages.slice(0, messageLimit) as message, i (i)}
    {@const m = object(message)}
    {#if m}
      <section class="message" data-role={String(m.role || m.type || "")}>
        <div class="message-role">
          {String(m.role || m.type || "message")}{#if m.name}<span
              class="mono">{String(m.name)}</span
            >{/if}{#if m.tool_call_id}<span class="mono"
              >{String(m.tool_call_id)}</span
            >{/if}
        </div>
        {#if m.content !== undefined}
          {#if Array.isArray(m.content)}
            {#each m.content as part, p (p)}
              {@const item = object(part)}
              {#if item && typeof item.text === "string"}<LongText
                  text={item.text}
                />{:else}<JsonView value={part} />{/if}
            {/each}
          {:else if typeof m.content === "string"}<LongText
              text={m.content}
            />{:else}<JsonView value={m.content} />{/if}
        {:else if typeof m.text === "string"}<LongText text={m.text} />
        {:else}<JsonView value={m} />{/if}
        {#if m.tool_calls}<div class="message-role secondary">tool calls</div>
          <JsonView value={m.tool_calls} />{/if}
      </section>
    {:else}<LongText text={pretty(message)} />{/if}
  {/each}
  {#if messages.length > messageLimit}<button
      class="text-button"
      onclick={() => (messageLimit += 100)}
      >Show 100 more ({messages.length - messageLimit} hidden)</button
    >{/if}
{:else if typeof value === "string"}<LongText text={value} />
{:else}<JsonView {value} />{/if}
