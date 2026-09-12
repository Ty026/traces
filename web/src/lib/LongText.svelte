<script lang="ts">
  let { text }: { text: string } = $props();
  let expanded = $state(false);
</script>

<div class="prose">{expanded ? text : text.slice(0, 24000)}</div>
{#if !expanded && text.length > 24000}<button
    class="text-button"
    onclick={() => (expanded = true)}
    >Show full text ({Math.ceil(text.length / 1024)} KB)</button
  >{/if}
