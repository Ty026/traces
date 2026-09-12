<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import { api, copy, date, relative, type Key } from "./api";
  let { notify }: { notify: (message: string) => void } = $props();
  let keys = $state<Key[]>([]),
    loading = $state(true),
    error = $state(""),
    legacy = $state(false),
    creating = $state(false),
    saving = $state(false),
    name = $state(""),
    expiry = $state(""),
    created = $state(""),
    revoke = $state<Key | null>(null),
    modalError = $state("");
  const active = $derived(
    keys.filter(
      (k) => !k.revokedAt && (!k.expiresAt || k.expiresAt > Date.now()),
    ).length,
  );
  const endpoint = `${location.origin}/v1/traces/ingest`;
  const example = `curl ${endpoint} \\
  -H "Authorization: Bearer $TRACE_API_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{"data":[{"object":"trace","id":"trace_123","workflow_name":"My agent"}]}'`;
  async function load() {
    try {
      const data = await api<{ items: Key[]; legacyEnabled: boolean }>(
        "/api/keys",
      );
      keys = data.items;
      legacy = data.legacyEnabled;
      error = "";
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  }
  async function create() {
    saving = true;
    modalError = "";
    try {
      const result = await api<{ key: string }>("/api/keys", {
        method: "POST",
        body: JSON.stringify({
          name,
          expiresAt: expiry ? new Date(expiry).getTime() : null,
        }),
      });
      created = result.key;
      creating = false;
      name = "";
      expiry = "";
      await load();
    } catch (e) {
      modalError = (e as Error).message;
    } finally {
      saving = false;
    }
  }
  async function remove() {
    if (!revoke) return;
    saving = true;
    modalError = "";
    try {
      await api("/api/keys/" + revoke.id, { method: "DELETE" });
      revoke = null;
      notify("API key revoked");
      await load();
    } catch (e) {
      modalError = (e as Error).message;
    } finally {
      saving = false;
    }
  }
  async function copied(value: string, message = "Copied") {
    try {
      await copy(value);
      notify(message);
    } catch {
      notify("Couldn't access the clipboard. Select the text and copy it.");
    }
  }
  function startCreate() {
    creating = true;
    modalError = "";
  }
  onMount(load);
</script>

<section class="keys-page">
  <div class="page-heading">
    <div>
      <h1>
        API keys{#if keys.length}<span class="heading-count">{active} active</span
          >{/if}
      </h1>
      <p>
        Agents use a key to send traces to this server. Keys can't read, export
        or delete data.
      </p>
    </div>
    {#if keys.length || error}<button
        class="button primary"
        onclick={startCreate}
        ><Icon name="plus" size={15} />Create API key</button
      >{/if}
  </div>
  {#if error}<div class="notice error">
      {error}<button class="button" onclick={load}>Try again</button>
    </div>{:else if loading}<div class="detail-loading">
      <span class="spinner"></span>Loading keys…
    </div>{:else if !keys.length}<div class="empty-state">
      <h2>No API keys yet</h2>
      <p>Create one key for each agent or environment that sends traces.</p>
      <button class="button primary" onclick={startCreate}
        ><Icon name="plus" size={15} />Create API key</button
      >
    </div>
  {:else}<div class="table-wrap">
      <table class="keys-table">
        <thead
          ><tr
            ><th>Name</th><th>Key</th><th>Status</th><th>Last used</th><th
              >Created</th
            ><th>Expires</th><th><span class="sr-only">Actions</span></th></tr
          ></thead
        ><tbody
          >{#each keys as key (key.id)}<tr class:inactive={!!key.revokedAt}
              ><td><strong>{key.name}</strong></td><td class="mono muted"
                >{key.prefix}…</td
              ><td
                >{#if key.revokedAt}<span class="status neutral"
                    ><span class="status-dot"></span>Revoked</span
                  >{:else if key.expiresAt && key.expiresAt < Date.now()}<span
                    class="status neutral"
                    ><span class="status-dot"></span>Expired</span
                  >{:else}<span class="status success"
                    ><span class="status-dot"></span>Active</span
                  >{/if}</td
              ><td title={key.lastUsedAt ? date(key.lastUsedAt) : undefined}
                >{key.lastUsedAt ? relative(key.lastUsedAt) : "Never"}</td
              ><td>{date(key.createdAt)}</td><td
                >{key.expiresAt ? date(key.expiresAt) : "Never"}</td
              ><td class="row-actions"
                >{#if !key.revokedAt}<button
                    class="text-button danger-hover"
                    onclick={() => {
                      revoke = key;
                      modalError = "";
                    }}>Revoke</button
                  >{/if}</td
              ></tr
            >{/each}</tbody
        >
      </table>
    </div>{/if}
  {#if legacy}<div class="legacy-key">
      <Icon name="key" size={16} />
      <p>
        <strong>An environment token is also accepted.</strong> To revoke it,
        remove <code>TRACE_INGEST_TOKEN</code> and restart the server.
      </p>
    </div>{/if}
  <section class="connection-guide" aria-labelledby="send-title">
    <h2 id="send-title">Send traces</h2>
    <div class="endpoint">
      <span>Endpoint</span><code>POST {endpoint}</code><button
        class="icon-button"
        onclick={() => copied(endpoint, "Endpoint copied")}
        aria-label="Copy ingest endpoint"><Icon name="copy" size={14} /></button
      >
    </div>
    <div class="code-block">
      <pre>{example}</pre>
      <button
        class="icon-button"
        onclick={() => copied(example, "Example copied")}
        aria-label="Copy example request"><Icon name="copy" size={14} /></button
      >
    </div>
    <ul class="guide-notes">
      <li>
        The body is <code>{'{"data": [...]}'}</code> with up to 1,000 trace and span
        records, 16 MiB at most.
      </li>
      <li>
        A <code>200</code> response means the records are queued and will be written
        within about a second.
      </li>
      <li>
        On <code>503</code>, wait for the <code>Retry-After</code> interval and send
        the same request again.
      </li>
    </ul>
  </section>
</section>
{#if creating}<Modal
    titleId="create-title"
    onclose={() => {
      if (!saving) creating = false;
    }}
    ><form
      onsubmit={(e) => {
        e.preventDefault();
        create();
      }}
    >
      <div class="modal-heading">
        <h2 id="create-title">Create API key</h2>
        <button
          type="button"
          class="icon-button"
          aria-label="Close"
          onclick={() => (creating = false)}><Icon name="close" /></button
        >
      </div>
      <p>Use a separate key for each agent or environment.</p>
      <label class="field"
        >Name<input
          required
          maxlength="100"
          placeholder="Research agent (production)"
          bind:value={name}
        /></label
      ><label class="field"
        ><span>Expires <span class="muted">(optional)</span></span><input
          type="datetime-local"
          bind:value={expiry}
        /></label
      >
      {#if modalError}<div class="notice error">{modalError}</div>{/if}
      <div class="modal-actions">
        <button
          class="button"
          type="button"
          onclick={() => (creating = false)}
          disabled={saving}>Cancel</button
        ><button class="button primary" disabled={saving || !name.trim()}
          >{saving ? "Creating…" : "Create key"}</button
        >
      </div>
    </form></Modal
  >{/if}
{#if created}<Modal titleId="created-title" onclose={() => (created = "")}
    ><h2 id="created-title">API key created</h2>
    <p>Copy the key now. It won't be shown again.</p>
    <div class="secret-key">
      <code data-testid="new-key">{created}</code><button
        class="icon-button"
        aria-label="Copy API key"
        onclick={() => copied(created, "API key copied")}
        ><Icon name="copy" size={16} /></button
      >
    </div>
    <div class="modal-actions">
      <button class="button primary" onclick={() => (created = "")}>Done</button
      >
    </div></Modal
  >{/if}
{#if revoke}<Modal
    titleId="revoke-title"
    onclose={() => {
      if (!saving) revoke = null;
    }}
    ><h2 id="revoke-title">Revoke “{revoke.name}”?</h2>
    <p>
      Requests using this key will be rejected from now on. Traces it already
      sent are kept.
    </p>
    {#if modalError}<div class="notice error">{modalError}</div>{/if}
    <div class="modal-actions">
      <button class="button" onclick={() => (revoke = null)} disabled={saving}
        >Cancel</button
      ><button class="button destructive" onclick={remove} disabled={saving}
        >{saving ? "Revoking…" : "Revoke key"}</button
      >
    </div></Modal
  >{/if}
