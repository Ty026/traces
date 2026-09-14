<script lang="ts">
  import { pretty } from "./api";
  let { value }: { value: unknown } = $props();
  let expanded = $state(false);
  const text = $derived(pretty(value));
  const shown = $derived(expanded ? text : text.slice(0, 24000));
  // Svelte escapes every token; payloads are never interpreted as HTML.
  const tokens = $derived(
    shown.split(
      /("(?:\\.|[^"\\])*"\s*:|"(?:\\.|[^"\\])*"|\b(?:true|false|null)\b|\b-?\d+(?:\.\d+)?\b)/g,
    ),
  );
</script>

<pre class="json-view"><code
    >{#each tokens as token, i (i)}<span
        class:json-key={/^".*":$/.test(token.trim())}
        class:json-string={token.startsWith('"') && !token.trim().endsWith(":")}
        class:json-literal={/^(true|false|null|-?\d)/.test(token)}>{token}</span
      >{/each}</code
  ></pre>
{#if !expanded && text.length > 24000}<button
    class="text-button"
    onclick={() => (expanded = true)}>Show full JSON</button
  >{/if}
