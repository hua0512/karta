<script setup lang="ts">
import { computed } from 'vue'
import { useData } from 'vitepress'
import DefaultTheme from 'vitepress/theme'
import KartaBentoFeatures from './components/KartaBentoFeatures.vue'
import KartaHeroDemo from './components/KartaHeroDemo.vue'
import KartaPipelineFlow from './components/KartaPipelineFlow.vue'
import KartaPlatformGrid from './components/KartaPlatformGrid.vue'
import KartaQuickStartCta from './components/KartaQuickStartCta.vue'

const { lang } = useData()
const isZh = computed(() => lang.value.startsWith('zh'))
</script>

<template>
  <DefaultTheme.Layout>
    <template #home-hero-info-before>
      <div class="karta-hero-badge-row">
        <a
          href="https://github.com/hua0512/karta/releases"
          target="_blank"
          rel="noopener noreferrer"
          class="karta-release-pill karta-focusable"
        >
          <span class="karta-release-dot" aria-hidden="true" />
          <span class="karta-release-ver karta-tabular">v0.1.0</span>
          <span class="karta-release-sep" aria-hidden="true" />
          <span class="karta-release-label">
            {{ isZh ? '纯 Rust 原生卡片渲染引擎' : 'Pure Rust · Native Card Engine' }}
          </span>
          <span class="karta-release-arrow" aria-hidden="true">→</span>
        </a>
      </div>
    </template>

    <template #home-hero-after>
      <div class="karta-home-section karta-hero-after-wrap">
        <KartaHeroDemo />
      </div>
    </template>

    <template #home-features-after>
      <div class="karta-home-section karta-features-after-wrap">
        <div class="karta-home-band">
          <div class="karta-band-block">
            <KartaBentoFeatures />
          </div>
          <div class="karta-band-block">
            <KartaPipelineFlow />
          </div>
          <div class="karta-band-block">
            <KartaPlatformGrid :compact="true" />
          </div>
          <div class="karta-band-block">
            <KartaQuickStartCta />
          </div>
        </div>
      </div>
    </template>
  </DefaultTheme.Layout>
</template>
