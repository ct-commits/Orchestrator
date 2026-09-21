<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let projects = $state([]);
  let error = $state(null);
  let loading = $state(true);

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

  onMount(load);

  const markers = { done: "✓", in_progress: "▸", blocked: "✗", todo: "·" };
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
        <section class="card">
          <div class="card-head">
            <h2>{p.name}</h2>
            <span class="badge badge-{p.maturity}">{p.maturity}</span>
          </div>
          <p class="repo">{p.repo}</p>
          <div
            class="bar"
            role="img"
            aria-label="{p.progress.done} of {p.progress.total} phases done"
          >
            <div class="bar-fill" style="width:{Math.round(p.progress.percent)}%"></div>
          </div>
          <p class="prog">
            {p.progress.done}/{p.progress.total} phases · {Math.round(p.progress.percent)}%
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
        </section>
      {/each}
    </div>
  {/if}
</main>

<footer class="foot">Orchestrator — read-mostly, local-first.</footer>
