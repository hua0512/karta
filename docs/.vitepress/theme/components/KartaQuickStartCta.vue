<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { useData, withBase } from 'vitepress'

type TabId = 'docker' | 'cli'

const { lang } = useData()
const isZh = computed(() => lang.value.startsWith('zh'))

const activeTab = ref<TabId>('docker')
const copied = ref(false)
let copyTimer: ReturnType<typeof setTimeout> | null = null

const snippets: Record<TabId, { label: string; raw: string }> = {
  docker: {
    label: 'Docker',
    raw: `docker run -it --rm \\
  -v ./kovi.conf.toml:/app/kovi.conf.toml \\
  -v ./data:/app/data \\
  ghcr.io/hua0512/karta:latest`,
  },
  cli: {
    label: 'karta-cli',
    raw: `karta-cli --render --render-type image \\
  "https://b23.tv/YTg9oSw"`,
  },
}

const primaryHref = computed(() =>
  withBase(isZh.value ? '/zh/getting-started' : '/getting-started')
)
const secondaryHref = computed(() =>
  withBase(isZh.value ? '/zh/deployment' : '/deployment')
)

function selectTab(tab: TabId) {
  activeTab.value = tab
  copied.value = false
}

async function copyCurrentSnippet() {
  const text = snippets[activeTab.value].raw
  let ok = false

  if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      ok = true
    } catch {
      ok = false
    }
  }

  if (!ok && typeof document !== 'undefined') {
    const textarea = document.createElement('textarea')
    textarea.value = text
    textarea.setAttribute('readonly', '')
    textarea.style.position = 'fixed'
    textarea.style.opacity = '0'
    document.body.appendChild(textarea)
    textarea.select()
    try {
      ok = document.execCommand('copy')
    } catch {
      ok = false
    }
    document.body.removeChild(textarea)
  }

  if (ok) {
    copied.value = true
    if (copyTimer) clearTimeout(copyTimer)
    copyTimer = setTimeout(() => {
      copied.value = false
    }, 2000)
  }
}

onBeforeUnmount(() => {
  if (copyTimer) clearTimeout(copyTimer)
})
</script>

<template>
  <section
    class="karta-cta"
    :aria-label="isZh ? '快速启动 Karta' : 'Quick Start Karta'"
  >
    <div class="karta-cta-shell">
      <div class="karta-cta-glow" aria-hidden="true" />

      <div class="karta-cta-grid">
        <!-- Left: Launch Copy & Actions -->
        <div class="karta-cta-copy">
          <span class="karta-cta-kicker">
            {{ isZh ? '即刻启程' : 'READY TO LAUNCH' }}
          </span>
          <h2 class="karta-cta-title">
            {{ isZh ? '30 秒启动你的 Karta' : 'Launch Karta in 30 seconds' }}
          </h2>
          <p class="karta-cta-subtitle">
            {{
              isZh
                ? '拉取多架构容器镜像快速接入 QQ 群聊，或直接运行 karta-cli 在本地终端渲染第一张原生卡片。'
                : 'Pull the multi-arch container to connect your QQ groups, or invoke karta-cli directly to render your first native card locally.'
            }}
          </p>

          <div class="karta-cta-actions">
            <a :href="primaryHref" class="karta-cta-btn-primary karta-focusable">
              <span>{{ isZh ? '快速开始' : 'Get Started' }}</span>
              <span class="karta-cta-arrow" aria-hidden="true">→</span>
            </a>
            <a :href="secondaryHref" class="karta-cta-btn-secondary karta-focusable">
              <span>{{ isZh ? 'Docker 部署' : 'Deployment Guide' }}</span>
            </a>
          </div>
        </div>

        <!-- Right: Interactive Tabbed Terminal -->
        <div class="karta-cta-terminal">
          <div class="karta-term-bar">
            <div class="karta-term-tabs" role="tablist" :aria-label="isZh ? '启动方式' : 'Launch method'">
              <button
                type="button"
                role="tab"
                class="karta-term-tab karta-focusable"
                :class="{ 'is-active': activeTab === 'docker' }"
                :aria-selected="activeTab === 'docker'"
                @click="selectTab('docker')"
              >
                Docker
              </button>
              <button
                type="button"
                role="tab"
                class="karta-term-tab karta-focusable"
                :class="{ 'is-active': activeTab === 'cli' }"
                :aria-selected="activeTab === 'cli'"
                @click="selectTab('cli')"
              >
                karta-cli
              </button>
            </div>

            <button
              type="button"
              class="karta-copy-btn karta-focusable"
              :class="{ 'is-copied': copied }"
              :aria-label="
                copied
                  ? isZh
                    ? '已复制命令'
                    : 'Command copied'
                  : isZh
                    ? '复制命令'
                    : 'Copy command'
              "
              @click="copyCurrentSnippet"
            >
              <svg
                v-if="!copied"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <rect x="9" y="9" width="12" height="12" rx="2" />
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
              </svg>
              <svg
                v-else
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.1"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <path d="m5 13 4 4L19 7" />
              </svg>
              <span>
                {{
                  copied
                    ? isZh
                      ? '已复制'
                      : 'Copied'
                    : isZh
                      ? '复制'
                      : 'Copy'
                }}
              </span>
            </button>
          </div>

          <div class="karta-term-body" role="tabpanel">
            <pre v-if="activeTab === 'docker'" class="karta-term-pre"><code><span class="tok-prompt">$</span> <span class="tok-cmd">docker</span> run <span class="tok-flag">-it --rm</span> <span class="tok-op">\</span>
  <span class="tok-flag">-v</span> <span class="tok-str">./kovi.conf.toml:/app/kovi.conf.toml</span> <span class="tok-op">\</span>
  <span class="tok-flag">-v</span> <span class="tok-str">./data:/app/data</span> <span class="tok-op">\</span>
  <span class="tok-img">ghcr.io/hua0512/karta:latest</span></code></pre>

            <pre v-else class="karta-term-pre"><code><span class="tok-prompt">$</span> <span class="tok-cmd">karta-cli</span> <span class="tok-flag">--render --render-type</span> image <span class="tok-op">\</span>
  <span class="tok-str">"https://b23.tv/YTg9oSw"</span></code></pre>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.karta-cta {
  width: 100%;
}

.karta-cta-shell {
  position: relative;
  border-radius: 22px;
  padding: 28px 24px;
  overflow: hidden;
  background: var(--karta-cta-bg);
  border: 1px solid var(--karta-cta-border);
  box-shadow:
    var(--karta-shadow-lg),
    0 1px 0 var(--karta-cta-inset) inset;
}

/* Top sunset hairline + ambient bloom */
.karta-cta-shell::before {
  content: '';
  position: absolute;
  top: 0;
  left: 12%;
  right: 12%;
  height: 1px;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(255, 94, 58, 0.65) 35%,
    rgba(251, 191, 36, 0.6) 65%,
    transparent 100%
  );
  pointer-events: none;
}

.karta-cta-glow {
  position: absolute;
  top: -110px;
  left: 50%;
  transform: translateX(-50%);
  width: 680px;
  height: 240px;
  background: var(--karta-cta-glow);
  pointer-events: none;
}

.karta-cta-grid {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: 1fr;
  gap: 26px;
  align-items: center;
}

@media (min-width: 640px) {
  .karta-cta-shell {
    padding: 36px 34px;
  }
}

@media (min-width: 960px) {
  .karta-cta-shell {
    padding: 42px 44px;
  }
  .karta-cta-grid {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.12fr);
    gap: 38px;
  }
}

.karta-cta-copy {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.karta-cta-kicker {
  display: inline-block;
  font-size: 11.5px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--vp-c-brand-1);
}

.karta-cta-title {
  margin: 0 !important;
  padding: 0 !important;
  border: none !important;
  font-size: 26px !important;
  font-weight: 750;
  letter-spacing: -0.03em;
  line-height: 1.22;
  color: var(--vp-c-text-1);
}

@media (min-width: 640px) {
  .karta-cta-title {
    font-size: 30px !important;
  }
}

.karta-cta-subtitle {
  margin: 0;
  font-size: 14.5px;
  line-height: 1.65;
  color: var(--vp-c-text-2);
}

.karta-cta-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  margin-top: 8px;
}

.karta-cta-btn-primary,
.karta-cta-btn-secondary {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 10px 20px;
  border-radius: 999px;
  font-size: 14px;
  font-weight: 600;
  text-decoration: none !important;
  transition:
    transform 0.2s cubic-bezier(0.22, 1, 0.36, 1),
    box-shadow 0.24s ease,
    border-color 0.2s ease,
    background-position 0.35s ease;
}

.karta-cta-btn-primary {
  color: #ffffff !important;
  background-image: linear-gradient(125deg, #ff5e3a 0%, #f43f5e 52%, #f97316 100%);
  background-size: 160% 160%;
  background-position: 0% 50%;
  border: 1px solid rgba(255, 255, 255, 0.18);
  box-shadow:
    0 10px 24px -8px rgba(244, 63, 94, 0.5),
    0 1px 0 rgba(255, 255, 255, 0.28) inset;
}

.karta-cta-btn-primary:hover {
  background-position: 100% 50%;
  transform: translateY(-1.5px);
  box-shadow:
    0 14px 30px -8px rgba(249, 115, 22, 0.58),
    0 1px 0 rgba(255, 255, 255, 0.34) inset;
}

.karta-cta-arrow {
  transition: transform 0.2s cubic-bezier(0.22, 1, 0.36, 1);
}

.karta-cta-btn-primary:hover .karta-cta-arrow {
  transform: translateX(3px);
}

.karta-cta-btn-secondary {
  color: var(--vp-c-text-1) !important;
  background: var(--karta-surface);
  border: 1px solid var(--karta-border-strong);
}

.karta-cta-btn-secondary:hover {
  border-color: var(--karta-border-hover);
  transform: translateY(-1px);
  box-shadow: 0 8px 20px -10px var(--karta-glow);
}

/* Tabbed Code Terminal */
.karta-cta-terminal {
  border-radius: 16px;
  overflow: hidden;
  background: #121218;
  border: 1px solid rgba(255, 255, 255, 0.11);
  box-shadow:
    0 18px 42px -12px rgba(0, 0, 0, 0.42),
    0 1px 0 rgba(255, 255, 255, 0.06) inset;
  min-width: 0;
}

.karta-term-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  background: rgba(255, 255, 255, 0.03);
  border-bottom: 1px solid rgba(255, 255, 255, 0.07);
}

.karta-term-tabs {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px;
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.32);
  border: 1px solid rgba(255, 255, 255, 0.06);
}

.karta-term-tab {
  appearance: none;
  border: none;
  background: transparent;
  color: #a1a1aa;
  font-family: var(--vp-font-family-mono);
  font-size: 12px;
  font-weight: 550;
  padding: 5px 12px;
  border-radius: 7px;
  cursor: pointer;
  transition: color 0.18s ease, background-color 0.18s ease;
}

.karta-term-tab:hover {
  color: #f4f4f5;
}

.karta-term-tab.is-active {
  color: #ffffff;
  background: rgba(255, 255, 255, 0.11);
  box-shadow: 0 1px 0 rgba(255, 255, 255, 0.1) inset;
}

.karta-copy-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 11px;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(255, 255, 255, 0.05);
  color: #d4d4d8;
  font-size: 12px;
  font-weight: 550;
  cursor: pointer;
  transition:
    color 0.18s ease,
    background-color 0.18s ease,
    border-color 0.18s ease;
}

.karta-copy-btn svg {
  width: 13.5px;
  height: 13.5px;
}

.karta-copy-btn:hover {
  color: #ffffff;
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(251, 146, 60, 0.45);
}

.karta-copy-btn.is-copied {
  color: #4ade80;
  border-color: rgba(74, 222, 128, 0.35);
  background: rgba(74, 222, 128, 0.1);
}

.karta-term-body {
  padding: 18px 20px;
  overflow-x: auto;
}

.karta-term-pre {
  margin: 0 !important;
  padding: 0 !important;
  background: transparent !important;
  font-family: var(--vp-font-family-mono);
  font-size: 13px;
  line-height: 1.72;
  color: #e4e4e7;
}

.karta-term-pre code {
  font-family: inherit;
  background: transparent !important;
  padding: 0 !important;
  color: inherit;
}

.tok-prompt {
  color: #fb923c;
  user-select: none;
  font-weight: 700;
}

.tok-cmd {
  color: #f8fafc;
  font-weight: 600;
}

.tok-flag {
  color: #fda4af;
}

.tok-str {
  color: #fcd34d;
}

.tok-img {
  color: #93c5fd;
}

.tok-op {
  color: #71717a;
}

@media (prefers-reduced-motion: reduce) {
  .karta-cta-btn-primary,
  .karta-cta-btn-secondary,
  .karta-cta-arrow,
  .karta-term-tab,
  .karta-copy-btn {
    transition: none !important;
  }
  .karta-cta-btn-primary:hover,
  .karta-cta-btn-secondary:hover,
  .karta-cta-btn-primary:hover .karta-cta-arrow {
    transform: none !important;
  }
}
</style>
