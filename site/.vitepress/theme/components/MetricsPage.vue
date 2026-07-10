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
}
</style>