<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import Icon from "./Icon.svelte";
  import TraceList from "./TraceList.svelte";
  import TraceDetail from "./TraceDetail.svelte";
  import Keys from "./Keys.svelte";
  import { api, type User, type Status } from "./api";
  let user = $state<User | null>(null),
    loaded = $state(false),
    ready = $state(false),
    error = $state(""),
    status = $state<Status | null>(null);
  let dark = $state(false),
    toast = $state("");
  const section = $derived(
    page.url.pathname.startsWith("/keys") ? "keys" : "traces",
  );
  const traceId = $derived(
    page.url.pathname.startsWith("/traces/")
      ? decodeURIComponent(page.url.pathname.slice(8))
      : null,
  );
  const authError = $derived(page.url.searchParams.get("auth_error"));
  let toastTimer: ReturnType<typeof setTimeout>;
  function notify(message: string) {
    toast = message;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ""), 3500);
  }
  function theme() {
    dark = !dark;
    document.documentElement.dataset.theme = dark ? "dark" : "light";
    localStorage.setItem("theme", dark ? "dark" : "light");
  }
  async function refreshStatus() {
    if (user) {
      try {
        status = await api<Status>("/api/status");
      } catch {
        /* request errors are handled by the session event and next refresh */
      }
    }
  }
  async function logout() {
    try {
      await api("/auth/logout", { method: "POST" });
      user = null;
      await goto("/");
    } catch (e) {
      notify((e as Error).message);
    }
  }
  onMount(() => {
    dark = document.documentElement.dataset.theme === "dark";
    const expired = () => {
      user = null;
      loaded = true;
    };
    window.addEventListener("session-expired", expired);
    let goPrefix = 0;
    const shortcuts = (event: KeyboardEvent) => {
      if (
        (event.target as HTMLElement)?.closest(
          'input,textarea,select,[contenteditable="true"]',
        ) ||
        document.querySelector("dialog[open]") ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey
      )
        return;
      if (event.key.toLowerCase() === "g") goPrefix = Date.now();
      else if (
        event.key.toLowerCase() === "t" &&
        Date.now() - goPrefix < 1000 &&
        user
      ) {
        event.preventDefault();
        goto("/traces");
        goPrefix = 0;
      }
    };
    window.addEventListener("keydown", shortcuts);

    Promise.all([
      fetch("/api/me").then(async (r) => {
        if (r.ok) user = await r.json();
        else if (r.status !== 401)
          throw new Error("Can't reach the server. Check that it's running.");
      }),
      api<{ ready: boolean }>("/auth/config").then((c) => (ready = c.ready)),
    ])
      .then(() => {
        if (user && page.url.pathname === "/")
          goto("/traces", { replaceState: true });
        refreshStatus();
      })
      .catch((e) => (error = e.message))
      .finally(() => (loaded = true));
    const timer = setInterval(refreshStatus, 5000);
    return () => {
      clearInterval(timer);
      clearTimeout(toastTimer);
      window.removeEventListener("session-expired", expired);
      window.removeEventListener("keydown", shortcuts);
    };
  });
</script>

<svelte:head
  ><title
    >{traceId ? "Trace" : section === "keys" ? "API keys" : "Traces"} · Agent Traces</title
  ><meta
    name="description"
    content="Self-hosted viewer for agent traces."
  /></svelte:head
>
{#if !loaded}<div class="initial-loading">
    <span class="brand-mark"><Icon name="trace" size={20} /></span><span
      class="spinner"
    ></span>
  </div>
{:else if !user}
  <div class="login-page">
    <button
      class="icon-button login-theme"
      onclick={theme}
      aria-label={dark ? "Switch to light theme" : "Switch to dark theme"}
      ><Icon name={dark ? "sun" : "moon"} /></button
    >
    <main class="login-card">
      <span class="brand-mark"><Icon name="trace" size={22} /></span>
      <h1>Sign in to Agent Traces</h1>
      <p>
        Use a GitHub account with a verified email on this server's access list.
      </p>
      {#if error}<div class="notice error">
          {error}<button class="text-button" onclick={() => location.reload()}
            >Try again</button
          >
        </div>{/if}
      {#if authError}<div class="notice error" role="alert">
          {authError === "not_allowed"
            ? "None of your verified GitHub emails are on the access list. Ask an administrator to add one to ALLOWED_EMAILS."
            : "GitHub sign-in didn't finish. Try again."}
        </div>{/if}
      {#if ready}<a href="/auth/github" class="button primary github-button"
          ><Icon name="github" size={18} />Continue with GitHub</a
        >
      {:else if !error}<div class="notice">
          <strong>GitHub sign-in isn't configured</strong>
          <p>
            Set <code>GITHUB_CLIENT_ID</code>, <code>GITHUB_CLIENT_SECRET</code>
            and <code>ALLOWED_EMAILS</code>, then restart the server.
          </p>
        </div>{/if}
    </main>
  </div>
{:else}
  <div class="workspace">
    <aside class="sidebar">
      <a href="/traces" class="wordmark"
        ><span class="brand-mark"><Icon name="trace" size={17} /></span><span
          >Agent Traces</span
        ></a
      >
      <nav aria-label="Main navigation">
        <a
          href="/traces"
          class:active={section === "traces"}
          aria-current={section === "traces" ? "page" : undefined}
          title="Traces (G then T)"
          ><Icon name="trace" size={17} /><span>Traces</span><kbd
            class="nav-shortcut">G T</kbd
          ></a
        ><a
          href="/keys"
          class:active={section === "keys"}
          aria-current={section === "keys" ? "page" : undefined}
          title="API keys"><Icon name="key" size={17} /><span>API keys</span></a
        >
      </nav>
      <div class="sidebar-bottom">
        <div class="retention" title="Traces expire after their last update">
          <Icon name="clock" size={14} /><span
            >{status?.retentionDays === 0
              ? "Traces kept indefinitely"
              : `Traces kept ${status?.retentionDays ?? 30} days`}</span
          >
        </div>
        <div class="account">
          <span class="avatar" aria-hidden="true"
            >{user.login.slice(0, 2).toUpperCase()}</span
          >
          <div>
            <strong>{user.login}</strong><span title={user.email}
              >{user.email}</span
            >
          </div>
        </div>
        <div class="sidebar-actions">
          <button
            class="icon-button"
            onclick={theme}
            title={dark ? "Switch to light theme" : "Switch to dark theme"}
            aria-label={dark ? "Switch to light theme" : "Switch to dark theme"}
            ><Icon name={dark ? "sun" : "moon"} size={16} /></button
          ><button
            class="icon-button"
            onclick={logout}
            title="Sign out"
            aria-label="Sign out"><Icon name="logout" size={16} /></button
          >
        </div>
      </div>
    </aside>
    <div class="main-shell">
      {#if status && (!status.writerHealthy || !status.maintenanceHealthy)}<div
          class="service-warning"
          role="alert"
        >
          <Icon name="alert" size={16} />{!status.writerHealthy
            ? `Storage is unavailable. ${status.pending.toLocaleString()} records are waiting to be written, and new traces are being rejected.`
            : "Cleanup or backup failed. Check the server logs."}
        </div>{/if}
      <main class="main-content">
        {#if section === "keys"}<Keys {notify} />{:else}<div
            class="list-frame"
            class:hidden={!!traceId}
          >
            <TraceList active={!traceId} {notify} />
          </div>
          {#if traceId}{#key traceId}<TraceDetail
                id={traceId}
                {notify}
              />{/key}{/if}{/if}
      </main>
    </div>
  </div>
{/if}
{#if toast}<div class="toast" role="status">{toast}</div>{/if}
