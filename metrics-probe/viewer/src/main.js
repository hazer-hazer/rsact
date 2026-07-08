import { createApp } from 'vue';
import App from './App.vue';
import { SAMPLE } from './lib/sample.js';

// Data arrives inlined in a <script type="application/json" id="metrics-data">
// block that `metrics-probe html` fills in. If the placeholder wasn't replaced
// (i.e. `vite dev`/`preview`), fall back to the dev sample fixture.
function loadData() {
  try {
    const el = document.getElementById('metrics-data');
    const d = JSON.parse(el.textContent);
    if (d && Array.isArray(d.snapshots)) return d;
  } catch {
    /* placeholder still literal → dev */
  }
  return import.meta.env.DEV ? SAMPLE : { snapshots: [], index: {} };
}

const { snapshots, index } = loadData();
createApp(App, { snapshots, index }).mount('#app');
