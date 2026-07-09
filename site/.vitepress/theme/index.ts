import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import './custom.scss'

// Task 4 registers <MetricsDashboard> here. Kept minimal for now.
export default {
  extends: DefaultTheme,
} satisfies Theme
