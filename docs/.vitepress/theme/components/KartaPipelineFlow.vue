<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useData, withBase } from 'vitepress'

interface PipelineStep {
  id: string
  num: string
  titleZh: string
  titleEn: string
  shortZh: string
  shortEn: string
  detailZh: string
  detailEn: string
  configKeys: string[]
  configHashEn: string
  configHashZh: string
  icon: 'input' | 'cache' | 'parsers' | 'render' | 'output'
}

const { lang } = useData()
const isZh = computed(() => lang.value.startsWith('zh'))

const steps: PipelineStep[] = [
  {
    id: 'capture',
    num: '01',
    titleZh: '输入捕获',
    titleEn: 'Input Capture',
    shortZh: 'URL / BV / 口令 / QQ JSON',
    shortEn: 'URL / BV / Share Code / QQ JSON',
    detailZh:
      '自动从普通文本消息、裸 BV/av 号、抖音分享口令以及 QQ JSON 分享卡片中提取目标链接，按群黑白名单与权限规则决定是否进入解析流程。',
    detailEn:
      'Extracts targets anywhere in a message — standard URLs, bare BV/av IDs, Douyin share codes, or QQ JSON share cards — while honoring group toggles and access control.',
    configKeys: ['access_control', 'group_blacklist_enabled', 'disabled_platforms'],
    configHashEn: '#access-control',
    configHashZh: '#access-control',
    icon: 'input',
  },
  {
    id: 'cache',
    num: '02',
    titleZh: '匹配与缓存',
    titleEn: 'Match & Cache',
    shortZh: 'RegexSet + LRU + Single-Flight',
    shortEn: 'RegexSet + LRU + Single-Flight',
    detailZh:
      '通过预编译 RegexSet 快速路由至对应平台。相同链接并发请求自动合并为单次 Single-Flight 解析，结果进入 LRU 缓存，高并发群聊零重复请求。',
    detailEn:
      'Routes URLs in microseconds via a precompiled RegexSet. Identical in-flight links coalesce into one single-flight parse and populate an LRU cache under bounded concurrency.',
    configKeys: ['cache_size', 'max_concurrent_parses', 'parse_timeout'],
    configHashEn: '#advanced-concurrency-caching-tools',
    configHashZh: '#advanced',
    icon: 'cache',
  },
  {
    id: 'parsers',
    num: '03',
    titleZh: '16 平台解析器',
    titleEn: '16 Platform Parsers',
    shortZh: '默认匿名优先',
    shortEn: 'Anonymous-first',
    detailZh:
      '统一抽象 Bilibili、抖音、Twitter/X、YouTube 等 16 个平台解析逻辑：绝大多数开箱即用无需 Cookie，按需注入凭据即可解锁高画质或会员内容。',
    detailEn:
      'Unified Rust parsers for Bilibili, Douyin, Twitter/X, YouTube, and 12 more platforms. Most work anonymously out of the box; optional cookies unlock higher tiers or stories.',
    configKeys: ['bili_cookie', 'douyin_cookie', 'xhs_cookie', 'proxy'],
    configHashEn: '#platform-credentials',
    configHashZh: '#platform-credentials',
    icon: 'parsers',
  },
  {
    id: 'render',
    num: '04',
    titleZh: '原生渲染与媒体',
    titleEn: 'Native Render & Media',
    shortZh: 'Rust 卡片 + FFmpeg + yt-dlp',
    shortEn: 'Rust Card + FFmpeg + yt-dlp',
    detailZh:
      '纯 Rust 合成温润纸面卡片（零浏览器开销），流式下载音视频并受控并发；需要时自动调用 FFmpeg 合并 DASH 流或由 yt-dlp 拉取外部平台视频。',
    detailEn:
      'Synthesizes warm-paper cards natively in Rust with zero headless browser overhead. Streams downloads concurrently, merges DASH via FFmpeg, and delegates to yt-dlp when needed.',
    configKeys: ['render_type', 'image_header_style', 'ffmpeg_path', 'ytdlp_path'],
    configHashEn: '#rendering-cards',
    configHashZh: '#rendering',
    icon: 'render',
  },
  {
    id: 'output',
    num: '05',
    titleZh: '聊天 / CLI 输出',
    titleEn: 'Chat / CLI Output',
    shortZh: '卡片 + 合并转发 + 音视频',
    shortEn: 'Card + Forward Gallery + Media',
    detailZh:
      '在 QQ 群聊中发送原生卡片、合并转发图集与音视频，并自动更新表情回应（👀 → ✨）；在终端里则通过 karta-cli 直接输出 JSON 或本地渲染图片。',
    detailEn:
      'Delivers the card, merged-forward galleries, and video/audio in QQ with live emoji reactions (👀 → ✨), or outputs JSON and local PNG cards directly from karta-cli.',
    configKeys: ['forward_contents', 'reaction_enabled', 'max_size', 'duration_maximum'],
    configHashEn: '#forwarding-delivery-reactions',
    configHashZh: '#output-behavior',
    icon: 'output',
  },
]

const AUTO_INTERVAL_MS = 4500

const activeStep = ref(0)
const autoPlaying = ref(false)
const reducedMotion = ref(false)
let advanceTimer: ReturnType<typeof setInterval> | null = null

const currentStep = computed(() => steps[activeStep.value])

const configLink = computed(() =>
  withBase(
    isZh.value
      ? `/zh/configuration${currentStep.value.configHashZh}`
      : `/configuration${currentStep.value.configHashEn}`
  )
)

function stopAutoAdvance() {
  if (advanceTimer !== null) {
    clearInterval(advanceTimer)
    advanceTimer = null
  }
  autoPlaying.value = false
}

function startAutoAdvance() {
  stopAutoAdvance()
  if (reducedMotion.value) return
  autoPlaying.value = true
  advanceTimer = setInterval(() => {
    activeStep.value = (activeStep.value + 1) % steps.length
  }, AUTO_INTERVAL_MS)
}

function selectStep(idx: number) {
  stopAutoAdvance()
  activeStep.value = idx
}

function onNodeHover() {
  if (autoPlaying.value) {
    stopAutoAdvance()
  }
}

function toggleAutoPlay() {
  if (reducedMotion.value) return
  if (autoPlaying.value) {
    stopAutoAdvance()
  } else {
    startAutoAdvance()
  }
}

onMounted(() => {
  if (typeof window === 'undefined') return
  const mq = window.matchMedia?.('(prefers-reduced-motion: reduce)')
  if (mq?.matches) {
    reducedMotion.value = true
    autoPlaying.value = false
    return
  }
  startAutoAdvance()
})

onBeforeUnmount(() => {
  stopAutoAdvance()
})
</script>

<template>
  <section
    class="karta-pipeline"
    :aria-label="isZh ? 'Karta 处理管线' : 'Karta processing pipeline'"
  >
    <div class="karta-section-header">
      <div>
        <span class="karta-section-kicker">
          {{ isZh ? '工作原理' : 'HOW IT WORKS' }}
        </span>
        <h2 class="karta-section-title">
          {{ isZh ? '五步全链路，从链接到原生卡片' : 'Five stages from link to native card' }}
        </h2>
      </div>

      <div class="karta-header-right">
        <p class="karta-section-subtitle">
          {{
            isZh
              ? '点击任意节点查看对应配置项与行为说明。'
              : 'Click any stage to inspect its behavior and related configuration keys.'
          }}
        </p>

        <button
          v-if="!reducedMotion"
          type="button"
          class="karta-auto-toggle karta-focusable"
          :class="{ active: autoPlaying }"
          :aria-label="
            autoPlaying
              ? isZh
                ? '暂停自动演示'
                : 'Pause flow playback'
              : isZh
                ? '播放管线演示'
                : 'Play flow animation'
          "
          :title="
            autoPlaying
              ? isZh
                ? '暂停自动演示'
                : 'Pause flow playback'
              : isZh
                ? '播放管线演示'
                : 'Play flow animation'
          "
          @click="toggleAutoPlay"
        >
          <svg v-if="autoPlaying" width="11" height="11" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <rect x="3.5" y="2.5" width="3.3" height="11" rx="1" />
            <rect x="9.2" y="2.5" width="3.3" height="11" rx="1" />
          </svg>
          <svg v-else width="11" height="11" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <path d="M4.2 2.8a1 1 0 0 1 1.5-.86l7.5 4.7a1 1 0 0 1 0 1.72l-7.5 4.7A1 1 0 0 1 4.2 12.2V2.8Z" />
          </svg>
          <span>
            {{ autoPlaying ? (isZh ? '演示中' : 'Auto') : isZh ? '播放' : 'Play' }}
          </span>
        </button>
      </div>
    </div>

    <!-- Horizontal (desktop) / Vertical (mobile) 5-Step Living Track -->
    <div class="karta-pipe-track" role="tablist" :aria-label="isZh ? '处理阶段' : 'Pipeline stages'">
      <template v-for="(step, idx) in steps" :key="step.id">
        <button
          type="button"
          role="tab"
          class="karta-pipe-node karta-focusable"
          :class="{
            active: idx === activeStep,
            passed: idx < activeStep,
          }"
          :aria-selected="idx === activeStep"
          @click="selectStep(idx)"
          @mouseenter="onNodeHover"
        >
          <div class="karta-node-top">
            <span class="karta-node-icon" aria-hidden="true">
              <!-- 01: Input Capture (Link + Spark) -->
              <svg v-if="step.icon === 'input'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                <path
                  d="M10 13a5 5 0 0 0 7.54.54l2.92-2.92a5 5 0 0 0-7.07-7.07l-1.72 1.71"
                  stroke="currentColor"
                  stroke-width="1.9"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
                <path
                  d="M14 11a5 5 0 0 0-7.54-.54l-2.92 2.92a5 5 0 0 0 7.07 7.07l1.71-1.71"
                  stroke="currentColor"
                  stroke-width="1.9"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
              <!-- 02: Match & Cache (Lightning Bolt) -->
              <svg v-else-if="step.icon === 'cache'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                <path
                  d="M13 2 3 14h8.6l-.8 8L21 10h-8.6L13 2Z"
                  stroke="currentColor"
                  stroke-width="1.9"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
              <!-- 03: 16 Platform Parsers (4-Module Grid) -->
              <svg v-else-if="step.icon === 'parsers'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                <rect x="3" y="3" width="7.5" height="7.5" rx="2.2" stroke="currentColor" stroke-width="1.9" />
                <rect x="13.5" y="3" width="7.5" height="7.5" rx="2.2" stroke="currentColor" stroke-width="1.9" />
                <rect x="3" y="13.5" width="7.5" height="7.5" rx="2.2" stroke="currentColor" stroke-width="1.9" />
                <rect x="13.5" y="13.5" width="7.5" height="7.5" rx="2.2" stroke="currentColor" stroke-width="1.9" />
              </svg>
              <!-- 04: Native Render & Media (Card Canvas + Mountain) -->
              <svg v-else-if="step.icon === 'render'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                <rect x="3" y="4" width="18" height="16" rx="3.2" stroke="currentColor" stroke-width="1.9" />
                <circle cx="8.6" cy="9.4" r="1.6" fill="currentColor" />
                <path
                  d="m21 15.6-5.2-4.6L7.4 20"
                  stroke="currentColor"
                  stroke-width="1.9"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
              <!-- 05: Chat / CLI Output (Paper Plane Send) -->
              <svg v-else width="18" height="18" viewBox="0 0 24 24" fill="none">
                <path
                  d="M21.5 2.5 10.8 13.2M21.5 2.5l-6.8 19-3.9-8.3-8.3-3.9 19-6.8Z"
                  stroke="currentColor"
                  stroke-width="1.9"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
            </span>
            <span class="karta-node-num karta-tabular">{{ step.num }}</span>
          </div>
          <div class="karta-node-title">{{ isZh ? step.titleZh : step.titleEn }}</div>
          <div class="karta-node-short">{{ isZh ? step.shortZh : step.shortEn }}</div>

          <!-- Subtle Auto-Advance Progress Hairline -->
          <span
            v-if="idx === activeStep && autoPlaying && !reducedMotion"
            :key="`prog-${activeStep}`"
            class="karta-node-progress"
            aria-hidden="true"
          />
        </button>

        <!-- Living Animated Energy Flow SVG Connector -->
        <div
          v-if="idx < steps.length - 1"
          class="karta-pipe-connector"
          :class="{
            energized: idx < activeStep,
            leading: idx === activeStep - 1,
          }"
          aria-hidden="true"
        >
          <!-- Horizontal Desktop Connector -->
          <svg class="karta-conn-h" viewBox="0 0 40 20" fill="none">
            <defs>
              <linearGradient
                :id="`karta-flow-h-${idx}`"
                x1="0"
                y1="10"
                x2="40"
                y2="10"
                gradientUnits="userSpaceOnUse"
              >
                <stop offset="0%" stop-color="#ff5e3a" />
                <stop offset="52%" stop-color="#f97316" />
                <stop offset="100%" stop-color="#fbbf24" />
              </linearGradient>
              <radialGradient :id="`karta-bead-h-${idx}`" cx="50%" cy="50%" r="50%">
                <stop offset="0%" stop-color="#fff7ed" stop-opacity="1" />
                <stop offset="45%" stop-color="#fb923c" stop-opacity="0.95" />
                <stop offset="100%" stop-color="#f97316" stop-opacity="0" />
              </radialGradient>
            </defs>

            <!-- Base Track -->
            <line
              x1="2"
              y1="10"
              x2="32"
              y2="10"
              stroke="var(--karta-border-strong)"
              stroke-width="2"
              stroke-linecap="round"
            />
            <!-- Continuous Animated Gradient Energy Pulse -->
            <line
              x1="2"
              y1="10"
              x2="32"
              y2="10"
              :stroke="`url(#karta-flow-h-${idx})`"
              :stroke-width="idx === activeStep - 1 ? 2.6 : 2"
              stroke-linecap="round"
              class="karta-flow-dash"
            />
            <!-- Glowing Traveling Highlight Bead on Active Leading Connector -->
            <g v-if="idx === activeStep - 1" class="karta-bead-track-h">
              <circle cx="4" cy="10" r="6.5" :fill="`url(#karta-bead-h-${idx})`" />
              <circle cx="4" cy="10" r="2.3" fill="#ffffff" />
            </g>
            <!-- Arrowhead -->
            <path
              d="m28.5 6 4.8 4-4.8 4"
              :stroke="idx < activeStep ? '#fb923c' : '#f97316'"
              :stroke-opacity="idx === activeStep - 1 ? 1 : idx < activeStep ? 0.82 : 0.45"
              stroke-width="1.85"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>

          <!-- Vertical Mobile Connector -->
          <svg class="karta-conn-v" viewBox="0 0 20 30" fill="none">
            <defs>
              <linearGradient
                :id="`karta-flow-v-${idx}`"
                x1="10"
                y1="0"
                x2="10"
                y2="30"
                gradientUnits="userSpaceOnUse"
              >
                <stop offset="0%" stop-color="#ff5e3a" />
                <stop offset="52%" stop-color="#f97316" />
                <stop offset="100%" stop-color="#fbbf24" />
              </linearGradient>
              <radialGradient :id="`karta-bead-v-${idx}`" cx="50%" cy="50%" r="50%">
                <stop offset="0%" stop-color="#fff7ed" stop-opacity="1" />
                <stop offset="45%" stop-color="#fb923c" stop-opacity="0.95" />
                <stop offset="100%" stop-color="#f97316" stop-opacity="0" />
              </radialGradient>
            </defs>
            <line
              x1="10"
              y1="2"
              x2="10"
              y2="23"
              stroke="var(--karta-border-strong)"
              stroke-width="2"
              stroke-linecap="round"
            />
            <line
              x1="10"
              y1="2"
              x2="10"
              y2="23"
              :stroke="`url(#karta-flow-v-${idx})`"
              :stroke-width="idx === activeStep - 1 ? 2.6 : 2"
              stroke-linecap="round"
              class="karta-flow-dash"
            />
            <g v-if="idx === activeStep - 1" class="karta-bead-track-v">
              <circle cx="10" cy="4" r="6" :fill="`url(#karta-bead-v-${idx})`" />
              <circle cx="10" cy="4" r="2.2" fill="#ffffff" />
            </g>
            <path
              d="m6.2 19.8 3.8 4.6 3.8-4.6"
              :stroke="idx < activeStep ? '#fb923c' : '#f97316'"
              :stroke-opacity="idx === activeStep - 1 ? 1 : idx < activeStep ? 0.82 : 0.45"
              stroke-width="1.85"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </div>
      </template>
    </div>

    <!-- Interactive Detail Strip -->
    <div class="karta-pipe-detail" role="tabpanel">
      <div class="karta-detail-main">
        <div class="karta-detail-head">
          <span class="karta-detail-step-pill karta-tabular">
            {{ isZh ? `阶段 ${currentStep.num}` : `Stage ${currentStep.num}` }}
          </span>
          <h3 class="karta-detail-title">
            {{ isZh ? currentStep.titleZh : currentStep.titleEn }}
          </h3>
        </div>
        <p class="karta-detail-desc">
          {{ isZh ? currentStep.detailZh : currentStep.detailEn }}
        </p>
      </div>

      <div class="karta-detail-footer">
        <div class="karta-detail-keys">
          <span class="karta-keys-label">{{ isZh ? '相关配置：' : 'Config keys:' }}</span>
          <code v-for="key in currentStep.configKeys" :key="key" class="karta-key-chip">{{ key }}</code>
        </div>
        <a :href="configLink" class="karta-config-link karta-focusable">
          <span>{{ isZh ? '查看配置文档' : 'Configuration reference' }}</span>
          <span aria-hidden="true">→</span>
        </a>
      </div>
    </div>
  </section>
</template>

<style scoped>
.karta-pipeline {
  margin: 12px 0;
}

.karta-section-header {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 22px;
}

.karta-section-kicker {
  display: inline-block;
  font-size: 11.5px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--vp-c-brand-1);
  margin-bottom: 6px;
}

.karta-section-title {
  margin: 0 !important;
  padding: 0 !important;
  border: none !important;
  font-size: 24px !important;
  font-weight: 700;
  letter-spacing: -0.02em;
  line-height: 1.28;
  color: var(--vp-c-text-1);
}

.karta-header-right {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
}

.karta-section-subtitle {
  margin: 0;
  font-size: 13.5px;
  color: var(--vp-c-text-2);
}

.karta-auto-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 11px;
  border-radius: 999px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  color: var(--vp-c-text-2);
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
}

.karta-auto-toggle:hover {
  border-color: var(--karta-border-hover);
  color: var(--vp-c-text-1);
}

.karta-auto-toggle.active {
  border-color: rgba(249, 115, 22, 0.42);
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}

/* 5-Step Track */
.karta-pipe-track {
  display: flex;
  align-items: stretch;
  gap: 0;
}

.karta-pipe-node {
  position: relative;
  overflow: hidden;
  flex: 1;
  min-width: 0;
  text-align: left;
  padding: 15px 14px 16px;
  border-radius: 15px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 6px;
  box-shadow: var(--karta-shadow-sm);
  transition:
    transform 0.22s cubic-bezier(0.22, 1, 0.36, 1),
    border-color 0.22s ease,
    box-shadow 0.22s ease,
    background-color 0.22s ease;
}

.karta-pipe-node:hover {
  transform: translateY(-2px);
  border-color: var(--karta-border-hover);
}

.karta-pipe-node.passed {
  border-color: rgba(249, 115, 22, 0.24);
}

.karta-pipe-node.active {
  border-color: rgba(249, 115, 22, 0.58);
  background: var(--karta-gradient-subtle);
  box-shadow:
    var(--karta-shadow-md),
    0 0 0 1px rgba(249, 115, 22, 0.2),
    0 10px 26px -14px var(--karta-glow);
}

.karta-node-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
}

.karta-node-icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--karta-surface-soft);
  border: 1px solid var(--karta-border);
  color: var(--vp-c-text-2);
  transition: all 0.22s ease;
}

.karta-pipe-node.passed .karta-node-icon {
  color: var(--vp-c-brand-1);
  border-color: rgba(249, 115, 22, 0.24);
}

.karta-pipe-node.active .karta-node-icon {
  background: linear-gradient(135deg, rgba(255, 94, 58, 0.18), rgba(249, 115, 22, 0.22));
  border-color: rgba(249, 115, 22, 0.45);
  color: var(--vp-c-brand-1);
  box-shadow: 0 4px 12px -4px rgba(249, 115, 22, 0.45);
}

.karta-node-num {
  font-size: 11px;
  font-weight: 700;
  padding: 1.5px 6px;
  border-radius: 6px;
  background: var(--karta-surface-soft);
  color: var(--vp-c-text-3);
  border: 1px solid var(--karta-border);
  transition: all 0.2s ease;
}

.karta-pipe-node.active .karta-node-num {
  background: var(--vp-c-brand-soft);
  border-color: rgba(249, 115, 22, 0.32);
  color: var(--vp-c-brand-1);
}

.karta-node-title {
  font-size: 14px;
  font-weight: 650;
  color: var(--vp-c-text-1);
  line-height: 1.3;
}

.karta-node-short {
  font-size: 12px;
  color: var(--vp-c-text-2);
  line-height: 1.4;
}

/* Subtle bottom progress bar during auto-advance */
.karta-node-progress {
  position: absolute;
  left: 0;
  bottom: 0;
  height: 2.5px;
  width: 100%;
  background: linear-gradient(90deg, #ff5e3a, #f97316, #fbbf24);
  transform-origin: left center;
  animation: karta-step-progress 4.5s linear forwards;
}

@keyframes karta-step-progress {
  from {
    transform: scaleX(0);
  }
  to {
    transform: scaleX(1);
  }
}

/* Living SVG Flow Connectors */
.karta-pipe-connector {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 32px;
}

.karta-conn-h {
  width: 32px;
  height: 20px;
  overflow: visible;
}

.karta-conn-v {
  display: none;
  width: 20px;
  height: 28px;
  overflow: visible;
}

.karta-flow-dash {
  stroke-dasharray: 6 10;
  opacity: 0.52;
  animation: karta-flow 1.45s linear infinite;
  transition: opacity 0.25s ease;
}

.karta-pipe-connector.energized .karta-flow-dash {
  opacity: 0.85;
  stroke-dasharray: 8 8;
  animation-duration: 1.15s;
}

.karta-pipe-connector.leading .karta-flow-dash {
  opacity: 1;
  stroke-dasharray: 10 7;
  animation-duration: 0.85s;
}

@keyframes karta-flow {
  from {
    stroke-dashoffset: 16;
  }
  to {
    stroke-dashoffset: 0;
  }
}

/* Glowing Traveling Highlight Bead */
.karta-bead-track-h {
  animation: karta-bead-slide-h 1.15s cubic-bezier(0.45, 0.05, 0.55, 0.95) infinite;
}

@keyframes karta-bead-slide-h {
  0% {
    transform: translateX(0px);
    opacity: 0;
  }
  18% {
    opacity: 1;
  }
  82% {
    opacity: 1;
  }
  100% {
    transform: translateX(27px);
    opacity: 0;
  }
}

.karta-bead-track-v {
  animation: karta-bead-slide-v 1.15s cubic-bezier(0.45, 0.05, 0.55, 0.95) infinite;
}

@keyframes karta-bead-slide-v {
  0% {
    transform: translateY(0px);
    opacity: 0;
  }
  18% {
    opacity: 1;
  }
  82% {
    opacity: 1;
  }
  100% {
    transform: translateY(19px);
    opacity: 0;
  }
}

@media (max-width: 860px) {
  .karta-pipe-track {
    flex-direction: column;
  }
  .karta-pipe-connector {
    width: 100%;
    height: 28px;
  }
  .karta-conn-h {
    display: none;
  }
  .karta-conn-v {
    display: block;
  }
}

/* Detail Strip */
.karta-pipe-detail {
  margin-top: 16px;
  border-radius: 16px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  padding: 18px 20px;
  box-shadow: var(--karta-shadow-sm);
}

.karta-detail-head {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}

.karta-detail-step-pill {
  font-size: 11px;
  font-weight: 700;
  padding: 2px 9px;
  border-radius: 999px;
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}

.karta-detail-title {
  margin: 0 !important;
  font-size: 16px !important;
  font-weight: 650;
  color: var(--vp-c-text-1);
}

.karta-detail-desc {
  margin: 0;
  font-size: 14px;
  line-height: 1.65;
  color: var(--vp-c-text-2);
}

.karta-detail-footer {
  margin-top: 14px;
  padding-top: 13px;
  border-top: 1px solid var(--vp-c-divider);
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.karta-detail-keys {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.karta-keys-label {
  font-size: 12.5px;
  color: var(--vp-c-text-3);
  margin-right: 2px;
}

.karta-key-chip {
  font-family: var(--vp-font-family-mono);
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 6px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface-soft);
  color: var(--vp-c-text-1);
}

.karta-config-link {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 13px;
  font-weight: 600;
  color: var(--vp-c-brand-1);
  text-decoration: none;
  white-space: nowrap;
}

.karta-config-link:hover {
  text-decoration: underline;
}

@media (prefers-reduced-motion: reduce) {
  .karta-flow-dash,
  .karta-bead-track-h,
  .karta-bead-track-v,
  .karta-node-progress {
    animation: none !important;
  }
  .karta-flow-dash {
    stroke-dasharray: none;
  }
  .karta-bead-track-h,
  .karta-bead-track-v {
    display: none;
  }
  .karta-pipe-node {
    transition: none !important;
  }
  .karta-pipe-node:hover {
    transform: none !important;
  }
}
</style>
