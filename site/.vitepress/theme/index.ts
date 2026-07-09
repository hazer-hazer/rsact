import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import MetricsDashboard from './components/MetricsDashboard.vue'
import './custom.scss'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('MetricsDashboard', MetricsDashboard)
  },
} satisfies Theme
