<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let projects = $state([]);
  let error = $state(null);
  let loading = $state(true);

  // Detail pane state
  let selected = $state(null);
  let delivery = $state(null);
  let deliveryLoading = $state(false);
  let deliveryError = $state(null);
  let confirmRemove = $state(false);
  let removeError = $state(null);

  async function load() {
    loading = true;
    error = null;
    try {
      projects = await invoke("list_projects");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function openDetail(p) {
    selected = p;
    delivery = null;
    deliveryError = null;
    confirmRemove = false;
    removeError = null;
    deliveryLoading = true;
    try {
      delivery = await invoke("get_delivery", { slug: p.slug });
    } catch (e) {
      deliveryError = String(e);
    } finally {
      deliveryLoading = false;
    }
  }

  function closeDetail() {
    selected = null;
    delivery = null;
    deliveryError = null;
    confirmRemove = false;
    removeError = null;
  }

  async function doRemove() {
    removeError = null;
    try {
      await invoke("remove_project", { slug: selected.slug });
      closeDetail();
      await load();
    } catch (e) {
      removeError = String(e);
      confirmRemove = false;
    }
  }

  onMount(load);

  const markers = { done: "✓", in_progress: "▸", blocked: "✗", todo: "·" };
  const tokenTotal = (t) =>
    t ? t.input + t.output + t.reasoning + t.cache_write + t.cache_read : 0;
  const fmtNum = (n) => n.toLocaleString();
  const fmtTokens = (n) =>
    n >= 1e6 ? (n / 1e6).toFixed(1) + "M" : n >= 1e3 ? (n / 1e3).toFixed(1) + "k" : String(n);
  function fmtAgo(iso) {
    if (!iso) return null;
    const then = new Date(iso).getTime();
    if (Number.isNaN(then)) return null;
    const s = Math.max(0, (Date.now() - then) / 1000);
    if (s < 3600) return Math.round(s / 60) + "m ago";
    if (s < 86400) return Math.round(s / 3600) + "h ago";
    if (s < 86400 * 30) return Math.round(s / 86400) + "d ago";
    if (s < 86400 * 365) return Math.round(s / (86400 * 30)) + "mo ago";
    return Math.round(s / (86400 * 365)) + "y ago";
  }
  const subtitle = $derived(
    projects.length === 1 ? "1 project" : `${projects.length} projects`,
  );
</script>

<header class="top">
  <h1>Portfolio</h1>
  <p class="subtitle">
    {#if loading}Loading…{:else if error}Could not load{:else}{subtitle}{/if}
  </p>
  <button class="refresh" onclick={load} disabled={loading}>Refresh</button>
</header>

<main>
  {#if error}
    <div class="state error">
      <p>Could not read the portfolio.</p>
      <pre>{error}</pre>
      <button onclick={load}>Try again</button>
    </div>
  {:else if loading}
    <div class="state">Reading roadmaps…</div>
  {:else if projects.length === 0}
    <div class="state">
      <p>No projects registered yet.</p>
      <p class="hint">Register one with <code>orchestrator add &lt;repo-path&gt;</code>.</p>
    </div>
  {:else}
    <div class="grid">
      {#each projects as p (p.slug)}
        <button class="card" onclick={() => openDetail(p)}>
          <div class="card-head">
            <h2>{p.name}</h2>
            {#if p.has_roadmap}
              <span class="badge badge-{p.maturity}">{p.maturity}</span>
            {:else}
              <span class="badge badge-none">no roadmap</span>
            {/if}
          </div>
          <div class="repo-row">
            <span class="repo">{p.repo || p.repo_path}</span>
            {#if p.last_activity}
              <span class="ago" title="Last activity">active {fmtAgo(p.last_activity)}</span>
            {/if}
          </div>
          {#if p.has_roadmap}
            <div
              class="bar"
              role="img"
              aria-label="{p.progress.done} of {p.progress.total} phases done"
            >
              <div class="bar-fill" style="width:{Math.round(p.progress.percent)}%"></div>
            </div>
            <p class="prog">
              {p.progress.done}/{p.progress.total} phases · {Math.round(p.progress.percent)}%
              {#if p.cost}
                <span class="cost-chip"
                  >${p.cost.cost_usd.toFixed(2)} · {fmtTokens(tokenTotal(p.cost.tokens))} tok</span
                >
              {/if}
            </p>
            <ol class="phases">
              {#each p.phases as phase (phase.id)}
                <li class="ph ph-{phase.status}">
                  <span class="dot">{markers[phase.status] ?? "·"}</span>{phase.name}
                </li>
              {/each}
            </ol>
            {#if p.parked.length > 0}
              <p class="parked">Parked: {p.parked.join(", ")}</p>
            {/if}
          {:else}
            <p class="placeholder-note">No roadmap yet — click to scaffold one.</p>
            {#if p.cost}
              <p class="prog">
                <span class="cost-chip"
                  >${p.cost.cost_usd.toFixed(2)} · {fmtTokens(tokenTotal(p.cost.tokens))} tok</span
                >
              </p>
            {/if}
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</main>

{#if selected}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scrim" onclick={closeDetail}>
    <aside class="detail" onclick={(e) => e.stopPropagation()}>
      <div class="detail-head">
        <div>
          <h2>{selected.name}</h2>
          {#if delivery?.repo_url}
            <a class="repo-link" href={delivery.repo_url} target="_blank" rel="noreferrer"
              >{selected.repo} ↗</a
            >
          {:else}
            <span class="repo">{selected.repo}</span>
          {/if}
        </div>
        <button class="close" onclick={closeDetail} aria-label="Close">✕</button>
      </div>

      <section class="detail-section">
        <h3>Phases</h3>
        {#if selected.has_roadmap}
          <ol class="phases">
            {#each selected.phases as phase (phase.id)}
              <li class="ph ph-{phase.status}">
                <span class="dot">{markers[phase.status] ?? "·"}</span>{phase.name}
              </li>
            {/each}
          </ol>
        {:else}
          <p class="muted">No <code>roadmap.yaml</code> yet. Generate one with a coding agent:</p>
          <pre class="cmd">orchestrator scaffold "{selected.repo_path}"</pre>
          <p class="muted">
            That prints a prompt to paste into Claude Code / Codex; the agent writes the file.
          </p>
          {#if selected.issue}<pre class="err">roadmap error: {selected.issue}</pre>{/if}
        {/if}
      </section>

      <section class="detail-section">
        <h3>Delivery</h3>
        {#if deliveryLoading}
          <p class="muted">Loading…</p>
        {:else if deliveryError}
          <pre class="err">{deliveryError}</pre>
        {:else if !delivery}
          <p class="muted">
            Not ingested yet. Run <code>orchestrator ingest {selected.slug}</code> to fetch
            merged &amp; open PRs and last activity.
          </p>
        {:else}
          <p class="source">
            Source: {delivery.source}{#if delivery.last_activity} · last activity {fmtAgo(delivery.last_activity)}{/if} · fetched {delivery.fetched_at}
          </p>
          {#if delivery.note}<p class="muted">{delivery.note}</p>{/if}

          {#if delivery.source === "github"}
            <h4>Merged PRs ({delivery.merged_prs.length})</h4>
            {#if delivery.merged_prs.length === 0}
              <p class="muted">None.</p>
            {:else}
              <ul class="links">
                {#each delivery.merged_prs as pr (pr.number)}
                  <li>
                    <a href={pr.url} target="_blank" rel="noreferrer">#{pr.number}</a>
                    {pr.title}
                  </li>
                {/each}
              </ul>
            {/if}

            <h4>Open PRs ({delivery.open_prs.length})</h4>
            {#if delivery.open_prs.length === 0}
              <p class="muted">None in flight.</p>
            {:else}
              <ul class="links">
                {#each delivery.open_prs as pr (pr.number)}
                  <li>
                    <a href={pr.url} target="_blank" rel="noreferrer">#{pr.number}</a>
                    {pr.title}
                  </li>
                {/each}
              </ul>
            {/if}
          {/if}

          {#if delivery.commits.length > 0}
            <h4>Recent commits ({delivery.commits.length})</h4>
            <ul class="commits">
              {#each delivery.commits as c (c.hash)}
                <li><code>{c.hash}</code> {c.subject} <span class="muted">{c.date}</span></li>
              {/each}
            </ul>
          {:else if delivery.source === "none"}
            <p class="muted">No delivery data available.</p>
          {/if}
        {/if}
      </section>

      <section class="detail-section">
        <h3>Cost</h3>
        {#if !selected.cost}
          <p class="muted">
            No cost data. Run <code>orchestrator cost --import &lt;CodeBurn-Export.json&gt;</code>.
          </p>
        {:else}
          <p class="source">From CodeBurn export · generated {selected.cost.generated}</p>
          <p class="cost-big">
            ${selected.cost.cost_usd.toFixed(2)}
            <span class="muted">· {fmtNum(selected.cost.api_calls)} calls</span>
          </p>
          <ul class="tokens">
            <li><span>Input</span><span>{fmtNum(selected.cost.tokens.input)}</span></li>
            <li><span>Output</span><span>{fmtNum(selected.cost.tokens.output)}</span></li>
            <li><span>Reasoning</span><span>{fmtNum(selected.cost.tokens.reasoning)}</span></li>
            <li><span>Cache write</span><span>{fmtNum(selected.cost.tokens.cache_write)}</span></li>
            <li><span>Cache read</span><span>{fmtNum(selected.cost.tokens.cache_read)}</span></li>
            <li class="tok-total">
              <span>Total</span><span>{fmtNum(tokenTotal(selected.cost.tokens))}</span>
            </li>
          </ul>
        {/if}
      </section>

      <section class="detail-section remove-row">
        {#if removeError}<pre class="err">{removeError}</pre>{/if}
        {#if !confirmRemove}
          <button class="remove-btn" onclick={() => (confirmRemove = true)}>
            Remove from portfolio
          </button>
        {:else}
          <span class="muted">Remove <strong>{selected.name}</strong>? (its roadmap.yaml is kept)</span>
          <div class="remove-actions">
            <button class="remove-btn danger" onclick={doRemove}>Confirm remove</button>
            <button onclick={() => (confirmRemove = false)}>Cancel</button>
          </div>
        {/if}
      </section>
    </aside>
  </div>
{/if}

<footer class="foot">Orchestrator — read-mostly, local-first.</footer>
