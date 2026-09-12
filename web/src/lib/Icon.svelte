<script lang="ts">
  let { name, size = 18 }: { name: string; size?: number } = $props();
  const paths: Record<string, string> = {
    // Staggered bars: a waterfall of steps, the product's core view.
    trace: "M4 6h8M8 12h10M12 18h8",
    search: "m21 21-4.3-4.3M19 11a8 8 0 1 1-16 0 8 8 0 0 1 16 0",
    key: "m15.5 7.5 2.3 2.3a1 1 0 0 0 1.4 0l2.1-2.1a1 1 0 0 0 0-1.4L19 4m2-2-9.6 9.6M13 15.5a5.5 5.5 0 1 1-11 0 5.5 5.5 0 0 1 11 0",
    arrow: "m9 5 7 7-7 7",
    back: "m14 6-6 6 6 6",
    plus: "M12 5v14M5 12h14",
    close: "m6 6 12 12M6 18 18 6",
    sun: "M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1 1m12 12 1 1M5 19l1-1M18 6l1-1M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0",
    moon: "M20 15a9 9 0 0 1-11-11A9 9 0 1 0 20 15",
    refresh:
      "M20 7v5h-5M4 17v-5h5M5 8a8 8 0 0 1 13-3l2 3M4 16l2 3a8 8 0 0 0 13-3",
    copy: "M8 8h12v12H8zM16 8V4H4v12h4",
    download: "M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5",
    trash: "M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7",
    check: "m5 12 4 4L19 6",
    clock: "M12 7v5l3 2M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0",
    code: "m8 7-5 5 5 5m8-10 5 5-5 5m-3-14-2 18",
    alert: "M12 8v5m0 3h.01M12 3 2 21h20L12 3",
    link: "m10 13 4-4M8 16l-1 1a4 4 0 0 1-6-6l4-4a4 4 0 0 1 6 0m2 1 1-1a4 4 0 0 1 6 6l-4 4a4 4 0 0 1-6 0",
    logout: "M9 4H4v16h5m5-13 5 5-5 5m-6-5h11",
    layers: "m12 3 10 5-10 5L2 8l10-5M2 12l10 5 10-5M2 16l10 5 10-5",
    github:
      "M9 19c-4 1-4-2-6-2m12 5v-4c0-1 .1-2-1-3 3 0 6-1 6-5 0-1-1-3-2-3 0-1 0-2 0-3-2 0-3 1-3 1a13 13 0 0 0-6 0s-1-1-3-1c0 1 0 2 0 3-1 1-2 2-2 3 0 4 3 5 6 5-1 1-1 2-1 3v4",
    activity: "M2 12h4l3-8 6 16 3-8h4",
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"><path d={paths[name] || paths.code} /></svg
>
