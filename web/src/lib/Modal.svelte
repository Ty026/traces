<script lang="ts">
  import type { Snippet } from "svelte";
  let {
    titleId,
    onclose,
    children,
  }: { titleId: string; onclose: () => void; children: Snippet } = $props();
  function open(dialog: HTMLDialogElement) {
    dialog.showModal();
    return {
      destroy() {
        dialog.close();
      },
    };
  }
</script>

<dialog
  class="modal"
  use:open
  aria-labelledby={titleId}
  oncancel={(e) => {
    e.preventDefault();
    onclose();
  }}
>
  {@render children()}
</dialog>
