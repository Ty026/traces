<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import { api, copy, date, type Key } from "./api";
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
  async function copied(value: string) {
    try {
      await copy(value);
      notify("Copied to clipboard");
    } catch {
      notify("Clipboard unavailable. Select and copy the text manually.");
    }
  }
  onMount(load);
</script>

<section class="keys-page">
  <div class="page-heading">
    <div>
      <div class="eyebrow">CONNECT YOUR AGENTS</div>
      <h1>API keys</h1>
      <p>Give your agents a secure way to send their traces.</p>
    </div>
    <button
      class="button primary"
      onclick={() => {
        creating = true;
        modalError = "";
      }}><Icon name="plus" size={16} />Create API key</button
    >
  </div>
  <div class="key-summary">
    <span class="key-symbol"><Icon name="key" size={24} /></span>
    <div>
      <strong>{active} active {active === 1 ? "key" : "keys"}</strong>
      <p>Keys can only send traces. Workspace access uses GitHub sign-in.</p>
    </div>
    <span class="badge">Ingest only</span>
  </div>
  {#if error}<div class="notice error">
      {error}<button class="button" onclick={load}>Try again</button>
    </div>{:else if loading}<div class="detail-loading">
      <span class="spinner"></span>Loading keys…
    </div>{:else if !keys.length}<div class="empty-state keys-empty">
      <span class="empty-icon"><Icon name="key" size={26} /></span>
      <h2>A key for every connection</h2>
      <p>
        Create your first key, give it a name you recognize,<br />and add it to
        your agent's tracing exporter.
      </p>
      <button class="button" onclick={() => (creating = true)}
        ><Icon name="plus" size={15} />Create your first key</button
      >
    </div>
  {:else}<div class="trace-table-wrap">
      <table class="keys-table">
        <thead
          ><tr
            ><th>Name</th><th>Key</th><th>Status</th><th>Last used</th><th
              >Created / expires</th
            ><th></th></tr
          ></thead
        ><tbody
          >{#each keys as key (key.id)}<tr
              ><td><strong>{key.name}</strong></td><td class="mono"
                >{key.prefix}••••••</td
              ><td
                >{#if key.revokedAt}<span class="badge">Revoked</span
                  >{:else if key.expiresAt && key.expiresAt < Date.now()}<span
                    class="badge">Expired</span
                  >{:else}<span class="badge success"
                    ><span class="status-dot"></span>Active</span
                  >{/if}</td
              ><td>{key.lastUsedAt ? date(key.lastUsedAt) : "Never"}</td><td
                >{date(key.createdAt)}<span class="cell-sub"
                  >{key.expiresAt
                    ? `Expires ${date(key.expiresAt)}`
                    : "No expiration"}</span
                ></td
              ><td
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
      <Icon name="key" size={18} />
      <div>
        <strong>Environment-managed token</strong>
        <p>
          The compatibility token is enabled. Remove TRACE_INGEST_TOKEN and
          restart the service to revoke it.
        </p>
      </div>
      <span class="badge">Environment</span>
    </div>{/if}
  <section class="connection-guide">
    <div>
      <span class="eyebrow">QUICK START</span>
      <h2>Make the connection.</h2>
      <p>
        Set your tracing exporter's endpoint to this URL and use your key as a
        Bearer token.
      </p>
    </div>
    <div class="endpoint-box">
      <span>INGEST ENDPOINT</span>
      <div>
        <code>{location.origin}/v1/traces/ingest</code><button
          class="icon-button"
          onclick={() => copied(location.origin + "/v1/traces/ingest")}
          aria-label="Copy ingest endpoint"
          ><Icon name="copy" size={16} /></button
        >
      </div>
    </div>
    <pre class="connection-example">Authorization: Bearer tr_…<br
      />Content-Type: application/json<br />OpenAI-Beta: traces=v1<br /><br
      />{'{ "data": [trace_or_span, …] }'}</pre>
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
          placeholder="e.g. Research agent · production"
          bind:value={name}
        /></label
      ><label class="field"
        >Expiration <span class="muted">Optional</span><input
          type="datetime-local"
          bind:value={expiry}
        /></label
      >
      <div class="notice compact">
        <Icon name="key" size={15} />This key can send traces only.
      </div>
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
    ><span class="empty-icon"><Icon name="check" size={25} /></span>
    <h2 id="created-title">Your key is ready</h2>
    <p>
      Copy it now and store it somewhere safe.<br />You won't be able to view it
      again.
    </p>
    <div class="secret-key">
      <code data-testid="new-key">{created}</code><button
        class="icon-button"
        aria-label="Copy API key"
        onclick={() => copied(created)}><Icon name="copy" /></button
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
      Agents using this key will no longer be able to send traces. You can
      create a replacement key at any time.
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
