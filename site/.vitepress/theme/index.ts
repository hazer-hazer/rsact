import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import MetricsDashboard from './components/MetricsDashboard.vue'
import './custom.scss'
import MetricsPage from './components/MetricsPage.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('MetricsDashboard', MetricsDashboard)
    app.component('MetricsPage', MetricsPage)
  },
} satisfies Theme
