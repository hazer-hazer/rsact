import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import App from './App.vue';
import { SAMPLE } from './lib/sample.js';

// The render-path safety net. The previous string-JS viewer shipped a
// ReferenceError that only bit at runtime because the smoke test skipped the
// real mount; here we actually mount the app with fixture data and drive it.
describe('App (mount)', () => {
  const factory = () =>
    mount(App, { props: { snapshots: SAMPLE.snapshots, index: SAMPLE.index } });

  it('mounts without throwing and renders a table per group', () => {
    const w = factory();
    // scenario + bench + size groups from the sample.
    expect(w.findAll('table').length).toBeGreaterThanOrEqual(3);
    expect(w.find('h1').text()).toContain('rsact');
  });

  it('renders domain-aware markers (an improvement ▲ and a regression ▼ exist)', () => {
    const html = factory().html();
    expect(html).toContain('▲');
    expect(html).toContain('▼');
  });

  it('shows the empty-state prompt when there are no snapshots', () => {
    const w = mount(App, { props: { snapshots: [], index: {} } });
    expect(w.text()).toContain('No snapshots yet');
    expect(w.findAll('table').length).toBe(0);
  });

  it('toggles a row: selection grows and an inline chart appears', async () => {
    const w = factory();
    expect(w.find('tr.chartrow').exists()).toBe(false);
    await w.find('tr.metric').trigger('click');
    await nextTick();
    expect(w.find('tr.chartrow').exists()).toBe(true);
    expect(w.findAll('.trendchart').length).toBeGreaterThanOrEqual(1);
  });

  it('select-all then clear empties the selection', async () => {
    const w = factory();
    await w.findAll('button')[0].trigger('click'); // select all
    await nextTick();
    expect(w.findAll('tr.chartrow').length).toBeGreaterThan(0);
    await w.findAll('button')[1].trigger('click'); // clear
    await nextTick();
    expect(w.findAll('tr.chartrow').length).toBe(0);
  });
});
