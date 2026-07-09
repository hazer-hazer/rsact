# Metrics

Per-commit performance and footprint, recorded in CI and charted live. Counts
(nodes, signals, allocations) are machine-independent; heap **bytes** and flash
sizes are CI-runner figures — compare trends within this store only.

<ClientOnly>
  <MetricsDashboard />
</ClientOnly>

> The standalone `metrics-probe html` viewer remains the **local** dev view (over
> your git-ignored local store). This page is the **CI public** view over the
> durable `metrics-data` store.
