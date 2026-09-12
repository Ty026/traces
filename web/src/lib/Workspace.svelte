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
          throw new Error("Unable to connect. Please try again.");
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
    >{traceId ? "Trace detail" : section === "keys" ? "API keys" : "Traces"} · Agent
    Traces</title
  ><meta
    name="description"
    content="A clear view into every agent run."
  /></svelte:head
>
{#if !loaded}<div class="initial-loading">
    <span class="brand-mark"><Icon name="trace" size={24} /></span><span
      class="spinner"
    ></span>
  </div>
{:else if !user}
  <div class="login-page">
    <div class="login-wordmark">
      <span class="brand-mark"><Icon name="trace" size={22} /></span>Agent
      Traces
    </div>
    <button
      class="icon-button login-theme"
      onclick={theme}
      aria-label="Toggle theme"><Icon name={dark ? "sun" : "moon"} /></button
    >
    <main class="login-card">
      <span class="eyebrow">A CLEARER VIEW</span>
      <h1>Every step.<br />The whole story.</h1>
      <p>
        Follow your agents from the first thought to the final response. Find
        the details that make a difference.
      </p>
      {#if error}<div class="notice error">
          {error}<button class="text-button" onclick={() => location.reload()}
            >Try again</button
          >
        </div>{/if}
      {#if authError}<div class="notice error">
          {authError === "not_allowed"
            ? "Your verified GitHub emails are not on the access list. Contact your administrator."
            : "GitHub sign-in was canceled. You can try again."}
        </div>{/if}
      {#if ready}<a href="/auth/github" class="button primary github-button"
          ><Icon name="github" size={20} />Continue with GitHub<Icon
            name="arrow"
            size={16}
          /></a
        >
        <div class="login-note">
          Access is limited to approved team members.
        </div>
      {:else}<div class="notice">
          <strong>Almost ready</strong>
          <p>
            Your administrator needs to configure GitHub sign-in and the email
            access list.
          </p>
        </div>{/if}
      <div class="login-preview" aria-hidden="true">
        <div>
          <span class="status-dot"></span>Research assistant<span class="muted"
            >4.28 s</span
          >
        </div>
        <div class="preview-step">
          <Icon name="layers" size={14} />Plan a response<i style="width:65%"
          ></i>
        </div>
        <div class="preview-step indent">
          <Icon name="code" size={14} />Search documents<i style="width:40%"
          ></i>
        </div>
        <div class="preview-step">
          <Icon name="check" size={14} />Generate answer<i style="width:80%"
          ></i>
        </div>
      </div>
    </main>
    <footer>Built for the details.</footer>
  </div>
{:else}
  <div class="workspace">
    <aside class="sidebar">
      <a href="/traces" class="wordmark"
        ><span class="brand-mark"><Icon name="trace" size={20} /></span><span
          >Agent Traces</span
        ></a
      >
      <div class="workspace-label">WORKSPACE</div>
      <nav aria-label="Main navigation">
        <a href="/traces" class:active={section === "traces"}
          ><Icon name="trace" /><span>Traces</span><span class="nav-shortcut"
            >G T</span
          ></a
        ><a href="/keys" class:active={section === "keys"}
          ><Icon name="key" /><span>API keys</span></a
        >
      </nav>
      <div class="sidebar-bottom">
        <div class="retention">
          <Icon name="clock" size={14} /><span
            >{status?.retentionDays === 0
              ? "No automatic expiration"
              : `${status?.retentionDays ?? 30}-day retention`}</span
          >
        </div>
        <div class="sidebar-rule"></div>
        <div class="account">
          <span class="avatar">{user.login.slice(0, 2).toUpperCase()}</span>
          <div><strong>{user.login}</strong><span>Administrator</span></div>
          <button
            class="icon-button"
            onclick={logout}
            title="Sign out"
            aria-label="Sign out"><Icon name="logout" size={17} /></button
          >
        </div>
        <button class="theme-button" onclick={theme}
          ><Icon name={dark ? "sun" : "moon"} size={15} />{dark
            ? "Light appearance"
            : "Dark appearance"}</button
        >
      </div>
    </aside>
    <div class="main-shell">
      <header class="topbar">
        <div class="breadcrumb">
          Workspace<Icon name="arrow" size={12} /><span
            >{section === "keys" ? "API keys" : "Traces"}</span
          >{#if traceId}<Icon name="arrow" size={12} /><span class="mono muted"
              >{traceId.slice(0, 18)}…</span
            >{/if}
        </div>
        <span class="private-badge"
          ><span class="status-dot"></span>Private workspace</span
        >
      </header>
      {#if status && (!status.writerHealthy || !status.maintenanceHealthy)}<div
          class="service-warning"
          role="alert"
        >
          <Icon name="alert" size={16} />{!status.writerHealthy
            ? `Storage is unavailable. ${status.pending} records are waiting; new ingestion is paused.`
            : "Data cleanup or backup needs attention. Check the service logs."}
        </div>{/if}
      <main class="main-content">
        {#if section === "keys"}<Keys {notify} />{:else}<div
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
{#if toast}<div class="toast" role="status">
    <Icon name="check" size={16} />{toast}
  </div>{/if}
