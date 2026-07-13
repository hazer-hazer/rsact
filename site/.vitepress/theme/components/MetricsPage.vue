<template>
    <div :class="$style['metrics-page']">
        <div class="vp-doc">
            <h1>Metrics</h1>
            <p>
                Per-commit performance and footprint, recorded in CI and charted live. Counts
                (nodes, signals, allocations) are machine-independent; heap <b>bytes</b> and flash
                sizes are CI-runner figures — compare trends within this store only.
            </p>
        </div>

        <MetricsDashboard />
    </div>
</template>

<script lang="ts" setup>
import MetricsDashboard from './MetricsDashboard.vue';

</script>

<style lang="scss" module>
.metrics-page {
  padding: 5px 10px;
  box-sizing: border-box;
}

// Desktop: fill exactly the viewport below the fixed nav (VitePress adds
// padding-top: --vp-nav-height to .VPContent at >=960px), and don't scroll the
// page — the header/controls stay put and ONLY the table (.grid-scroll) scrolls.
@media (min-width: 960px) {
  .metrics-page {
    height: calc(100vh - var(--vp-nav-height));
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  // header block: natural height
  .metrics-page :global(.vp-doc) {
    flex: 0 0 auto;
  }
  // dashboard fills the rest and becomes a column so its .wrap can flex-fill
  .metrics-page :global(.metrics) {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
}
</style>