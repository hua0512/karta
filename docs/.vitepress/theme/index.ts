import type { Theme } from 'vitepress'
import DefaultTheme from 'vitepress/theme'
import Layout from './Layout.vue'
import KartaBentoFeatures from './components/KartaBentoFeatures.vue'
import KartaConfigPlayground from './components/KartaConfigPlayground.vue'
import KartaHeroDemo from './components/KartaHeroDemo.vue'
import KartaPipelineFlow from './components/KartaPipelineFlow.vue'
import KartaPlatformGrid from './components/KartaPlatformGrid.vue'
import KartaQuickStartCta from './components/KartaQuickStartCta.vue'
import './custom.css'

export default {
  extends: DefaultTheme,
  Layout,
  enhanceApp({ app }) {
    app.component('KartaBentoFeatures', KartaBentoFeatures)
    app.component('KartaHeroDemo', KartaHeroDemo)
    app.component('KartaPipelineFlow', KartaPipelineFlow)
    app.component('KartaPlatformGrid', KartaPlatformGrid)
    app.component('KartaConfigPlayground', KartaConfigPlayground)
    app.component('KartaQuickStartCta', KartaQuickStartCta)
  },
} satisfies Theme
