<script setup lang="ts">
import { computed } from 'vue'
import { useData, withBase } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value.startsWith('zh'))

const links = computed(() => ({
  rendering: withBase(isZh.value ? '/zh/configuration#rendering' : '/configuration#rendering-cards'),
  platforms: withBase(isZh.value ? '/zh/platforms' : '/platforms'),
  media: withBase(isZh.value ? '/zh/configuration#media-and-network' : '/configuration#media-limits-proxy'),
  gettingStarted: withBase(isZh.value ? '/zh/getting-started' : '/getting-started'),
  commands: withBase(isZh.value ? '/zh/commands' : '/commands'),
  deployment: withBase(isZh.value ? '/zh/deployment' : '/deployment'),
}))

const cardStyles = [
  { id: 'blur_background', label: 'blur_background', variant: 'frosted' },
  { id: 'mesh_glow', label: 'mesh_glow', variant: 'mesh-glow' },
  { id: 'blur', label: 'blur', variant: 'blur' },
  { id: 'soft_prism', label: 'soft_prism', variant: 'soft-prism' },
] as const

const featuredPlatforms = computed(() => [
  { name: 'Bilibili', color: '#fb7299' },
  { name: isZh.value ? '抖音' : 'Douyin', color: '#14b8a6' },
  { name: 'X', color: '#71717a' },
  { name: 'YouTube', color: '#ef4444' },
  { name: isZh.value ? '小红书' : 'Xiaohongshu', color: '#ff2442' },
])

const mediaBadges = computed(() => [
  { label: 'FFmpeg Mux', tone: 'coral' },
  { label: 'yt-dlp', tone: 'amber' },
  { label: isZh.value ? '合并转发' : 'Forward', tone: 'neutral' },
])

const operatorCommands = ['/status', '/login bili', '/disable'] as const

const infraStats = computed(() => [
  { value: '16', label: isZh.value ? '平台' : 'Platforms' },
  { value: '0', label: isZh.value ? '浏览器依赖' : 'No Browser' },
  { value: 'amd64 · arm64', label: isZh.value ? '多架构镜像' : 'Multi-arch' },
])
</script>

<template>
  <section
    class="karta-bento"
    :aria-label="isZh ? '核心特性' : 'Core Capabilities'"
  >
    <div class="karta-bento-header">
      <div>
        <span class="karta-bento-kicker">
          {{ isZh ? '核心能力' : 'CORE CAPABILITIES' }}
        </span>
        <h2 class="karta-bento-title">
          {{ isZh ? '纯 Rust 打造的解析与原生出图引擎' : 'Engineered in Rust, built for real chats' }}
        </h2>
      </div>
      <p class="karta-bento-subtitle">
        {{
          isZh
            ? '零浏览器开销、默认匿名可用，兼顾群聊交互质感与运维可靠性。'
            : 'Zero headless browsers, anonymous by default, crafted for both QQ groups and terminals.'
        }}
      </p>
    </div>

    <div class="karta-bento-grid">
      <!-- Card 1 (Span 7): Native Rust Card Renderer -->
      <a
        :href="links.rendering"
        class="karta-bento-card karta-span-7 karta-card-renderer karta-focusable"
      >
        <div class="karta-card-top">
          <div class="karta-icon-box" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
              <rect x="3" y="4" width="18" height="16" rx="3" />
              <path d="M3 10h18" />
              <path d="M7.5 14.5h4.5" />
              <path d="M7.5 17h7" />
              <circle cx="17.2" cy="7" r="1" fill="currentColor" stroke="none" />
            </svg>
          </div>
          <span class="karta-metric-pill karta-tabular">
            <span class="karta-metric-dot" aria-hidden="true" />
            {{ isZh ? '~40ms · 零 Chromium' : '~40ms · Zero Chromium' }}
          </span>
        </div>

        <div class="karta-card-body">
          <h3 class="karta-card-title">
            {{ isZh ? '原生 Rust 卡片渲染引擎' : 'Native Rust Card Renderer' }}
          </h3>
          <p class="karta-card-desc">
            {{
              isZh
                ? '纯 Rust 合成高质感卡片：支持自适应明暗磨砂玻璃（auto / light / dark）与四种背景风格，内置竖屏视频左右分栏、无空洞 Bento 拼图、标签胶囊、原生清晰转发框与彩色 Emoji，也可随时切回纯文本。'
                : 'Synthesize adaptive light & dark frosted-glass cards natively in Rust with 4 backdrop styles, portrait split-hero covers, hole-free Bento galleries, tag pills, native repost frames, and crisp emoji — or switch to clean text anytime.'
            }}
          </p>
        </div>

        <div class="karta-swatch-strip" aria-hidden="true">
          <div
            v-for="style in cardStyles"
            :key="style.id"
            class="karta-swatch-item"
          >
            <div class="karta-mini-paper" :class="`is-${style.variant}`">
              <span class="karta-mini-header" />
              <span class="karta-mini-lines">
                <i class="w-lg" />
                <i class="w-sm" />
              </span>
            </div>
            <span class="karta-swatch-label">{{ style.label }}</span>
          </div>
        </div>

        <div class="karta-card-foot">
          <span>{{ isZh ? '探索渲染风格与配色' : 'Customize card styles' }}</span>
          <span class="karta-arrow" aria-hidden="true">→</span>
        </div>
      </a>

      <!-- Card 2 (Span 5): 16 Platforms, One Engine -->
      <a
        :href="links.platforms"
        class="karta-bento-card karta-span-5 karta-card-platforms karta-focusable"
      >
        <div class="karta-card-top">
          <div class="karta-icon-box" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
              <rect x="3" y="3" width="7.5" height="7.5" rx="2" />
              <rect x="13.5" y="3" width="7.5" height="7.5" rx="2" />
              <rect x="3" y="13.5" width="7.5" height="7.5" rx="2" />
              <path d="M17.25 13.5v7.5M13.5 17.25h7.5" />
            </svg>
          </div>
          <span class="karta-soft-pill">
            {{ isZh ? '匿名优先' : 'Anonymous-first' }}
          </span>
        </div>

        <div class="karta-card-body">
          <h3 class="karta-card-title">
            {{ isZh ? '16 个平台，一套引擎' : '16 Platforms, One Engine' }}
          </h3>
          <p class="karta-card-desc">
            {{
              isZh
                ? '统一匹配标准 URL、裸 BV/av 号、分享口令与 QQ JSON 卡片，绝大多数平台无需 Cookie 开箱即用。'
                : 'Match standard URLs, bare BV/av IDs, share codes, and QQ JSON cards across 16 platforms — most work anonymously out of the box.'
            }}
          </p>
        </div>

        <div class="karta-platform-cluster" aria-hidden="true">
          <span
            v-for="p in featuredPlatforms"
            :key="p.name"
            class="karta-plat-chip"
          >
            <span class="karta-plat-dot" :style="{ backgroundColor: p.color }" />
            {{ p.name }}
          </span>
          <span class="karta-plat-chip is-more karta-tabular">+11</span>
        </div>

        <div class="karta-card-foot">
          <span>{{ isZh ? '查看全部 16 个平台' : 'Explore all 16 platforms' }}</span>
          <span class="karta-arrow" aria-hidden="true">→</span>
        </div>
      </a>

      <!-- Card 3 (Span 4): Full Media Pipeline -->
      <a
        :href="links.media"
        class="karta-bento-card karta-span-4 karta-focusable"
      >
        <div class="karta-card-top">
          <div class="karta-icon-box" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
              <rect x="2.75" y="4.5" width="18.5" height="15" rx="2.75" />
              <path d="M10 9.25 15.25 12 10 14.75V9.25Z" fill="currentColor" stroke="none" />
              <path d="M7 4.5v15M17 4.5v15" opacity="0.35" />
            </svg>
          </div>
        </div>

        <div class="karta-card-body">
          <h3 class="karta-card-title">
            {{ isZh ? '完整音视频管线' : 'Full Media Pipeline' }}
          </h3>
          <p class="karta-card-desc">
            {{
              isZh
                ? '受控并发流式下载、FFmpeg 自动合并 DASH 流、按需调用 yt-dlp，并严守体积与时长上限。'
                : 'Stream downloads with bounded concurrency, mux DASH streams via FFmpeg, shell out to yt-dlp, and bundle galleries cleanly.'
            }}
          </p>
        </div>

        <div class="karta-chip-row" aria-hidden="true">
          <span
            v-for="b in mediaBadges"
            :key="b.label"
            class="karta-mini-badge"
            :class="`tone-${b.tone}`"
          >
            {{ b.label }}
          </span>
        </div>

        <div class="karta-card-foot">
          <span>{{ isZh ? '媒体与代理配置' : 'Media limits & proxy' }}</span>
          <span class="karta-arrow" aria-hidden="true">→</span>
        </div>
      </a>

      <!-- Card 4 (Span 4): QQ Bot & Standalone CLI -->
      <a
        :href="links.gettingStarted"
        class="karta-bento-card karta-span-4 karta-focusable"
      >
        <div class="karta-card-top">
          <div class="karta-icon-box" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
              <rect x="3" y="4" width="18" height="16" rx="3" />
              <path d="m7.5 10 2.75 2.25L7.5 14.5" />
              <path d="M12.5 14.5h4" />
            </svg>
          </div>
        </div>

        <div class="karta-card-body">
          <h3 class="karta-card-title">
            {{ isZh ? 'QQ 机器人与独立 CLI' : 'QQ Bot & Standalone CLI' }}
          </h3>
          <p class="karta-card-desc">
            {{
              isZh
                ? '既可作为 Kovi OneBot V11 机器人贴表情回应群聊，也能在终端用 karta-cli 一键解析或渲染本地卡片。'
                : 'Run as a Kovi OneBot V11 bot with live status reactions, or invoke karta-cli directly to inspect JSON and render cards.'
            }}
          </p>
        </div>

        <div class="karta-cli-chip" aria-hidden="true">
          <span class="karta-cli-prompt">$</span>
          <span class="karta-cli-cmd">karta-cli --render &lt;url&gt;</span>
        </div>

        <div class="karta-card-foot">
          <span>{{ isZh ? '快速上手指南' : 'Try bot or CLI' }}</span>
          <span class="karta-arrow" aria-hidden="true">→</span>
        </div>
      </a>

      <!-- Card 5 (Span 4): Built for Operators -->
      <a
        :href="links.commands"
        class="karta-bento-card karta-span-4 karta-focusable"
      >
        <div class="karta-card-top">
          <div class="karta-icon-box" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
              <path d="M4 7h10" />
              <circle cx="17" cy="7" r="2.25" />
              <path d="M20 12H10" />
              <circle cx="7" cy="12" r="2.25" />
              <path d="M4 17h10" />
              <circle cx="17" cy="17" r="2.25" />
            </svg>
          </div>
        </div>

        <div class="karta-card-body">
          <h3 class="karta-card-title">
            {{ isZh ? '为日常运维而生' : 'Built for Operators' }}
          </h3>
          <p class="karta-card-desc">
            {{
              isZh
                ? '按群启停、黑白名单访问控制与扫码自动续期；运行期状态独立持久化，绝不覆盖手写 config.json。'
                : 'Per-group toggles, user/group access control, QR cookie refresh, and runtime state isolated from your hand-edited config.json.'
            }}
          </p>
        </div>

        <div class="karta-chip-row" aria-hidden="true">
          <code
            v-for="cmd in operatorCommands"
            :key="cmd"
            class="karta-cmd-chip"
          >
            {{ cmd }}
          </code>
        </div>

        <div class="karta-card-foot">
          <span>{{ isZh ? '查看管理命令' : 'Admin commands' }}</span>
          <span class="karta-arrow" aria-hidden="true">→</span>
        </div>
      </a>

      <!-- Card 6 (Span 12): Horizontal Highlight Bar -->
      <a
        :href="links.deployment"
        class="karta-bento-card karta-span-12 karta-card-infra karta-focusable"
      >
        <div class="karta-infra-left">
          <div class="karta-icon-box is-compact" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 3 4.5 7v10L12 21l7.5-4V7L12 3Z" />
              <path d="M4.5 7 12 11l7.5-4" />
              <path d="M12 11v10" />
            </svg>
          </div>
          <div class="karta-infra-copy">
            <div class="karta-infra-title-row">
              <h3 class="karta-card-title">
                {{
                  isZh
                    ? '默认匿名可用 · 单文件静态部署'
                    : 'Anonymous-First · Single Static Binary'
                }}
              </h3>
              <span class="karta-infra-arrow" aria-hidden="true">→</span>
            </div>
            <p class="karta-card-desc">
              {{
                isZh
                  ? '绝大多数平台零 Cookie 即开即用；提供单文件静态二进制与 amd64/arm64 多架构 Docker 镜像，内置 LRU 缓存与 Single-Flight 并发去重。'
                  : 'Most platforms work with zero cookies. Ship a single static binary or multi-arch amd64/arm64 Docker image backed by LRU caching and single-flight request deduplication.'
              }}
            </p>
          </div>
        </div>

        <div class="karta-infra-stats" aria-hidden="true">
          <div
            v-for="stat in infraStats"
            :key="stat.label"
            class="karta-stat-pill"
          >
            <strong class="karta-stat-val karta-tabular">{{ stat.value }}</strong>
            <span class="karta-stat-label">{{ stat.label }}</span>
          </div>
        </div>
      </a>
    </div>
  </section>
</template>

<style scoped>
.karta-bento {
  width: 100%;
}

.karta-bento-header {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: 12px 24px;
  margin-bottom: 24px;
}

.karta-bento-kicker {
  display: inline-block;
  font-size: 11.5px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--vp-c-brand-1);
  margin-bottom: 6px;
}

.karta-bento-title {
  margin: 0 !important;
  padding: 0 !important;
  border: none !important;
  font-size: 24px !important;
  font-weight: 700;
  letter-spacing: -0.025em;
  line-height: 1.26;
  color: var(--vp-c-text-1);
}

.karta-bento-subtitle {
  margin: 0;
  max-width: 440px;
  font-size: 14px;
  line-height: 1.55;
  color: var(--vp-c-text-2);
}

/* 12-column responsive Bento Grid */
.karta-bento-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 16px;
}

@media (min-width: 640px) {
  .karta-bento-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .karta-span-7,
  .karta-span-12 {
    grid-column: span 2;
  }
}

@media (min-width: 960px) {
  .karta-bento-grid {
    grid-template-columns: repeat(12, minmax(0, 1fr));
  }
  .karta-span-7 {
    grid-column: span 7;
  }
  .karta-span-5 {
    grid-column: span 5;
  }
  .karta-span-4 {
    grid-column: span 4;
  }
  .karta-span-12 {
    grid-column: span 12;
  }
}

/* Base Bento Card */
.karta-bento-card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 22px 24px;
  border-radius: 18px;
  text-decoration: none !important;
  color: inherit;
  background: var(--karta-bento-bg);
  border: 1px solid var(--karta-border);
  box-shadow:
    var(--karta-shadow-sm),
    0 1px 0 var(--karta-bento-inset) inset;
  overflow: hidden;
  transition:
    transform 0.24s cubic-bezier(0.22, 1, 0.36, 1),
    border-color 0.24s ease,
    box-shadow 0.28s ease;
}

/* Top specular hairline */
.karta-bento-card::before {
  content: '';
  position: absolute;
  top: 0;
  left: 16%;
  right: 16%;
  height: 1px;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(249, 115, 22, 0.32) 50%,
    transparent 100%
  );
  opacity: 0;
  transition: opacity 0.26s ease;
  pointer-events: none;
}

/* Subtle warm radial glow on hover */
.karta-bento-card::after {
  content: '';
  position: absolute;
  inset: 0;
  background: radial-gradient(
    420px circle at 18% 0%,
    rgba(251, 146, 60, 0.1) 0%,
    transparent 62%
  );
  opacity: 0;
  transition: opacity 0.28s ease;
  pointer-events: none;
}

.karta-bento-card:hover {
  transform: translateY(-2.5px);
  border-color: rgba(251, 146, 60, 0.42);
  box-shadow:
    var(--karta-shadow-md),
    0 0 0 1px rgba(251, 146, 60, 0.14),
    0 16px 36px -18px var(--karta-glow);
}

.karta-bento-card:hover::before,
.karta-bento-card:hover::after {
  opacity: 1;
}

.karta-card-top,
.karta-card-body,
.karta-swatch-strip,
.karta-platform-cluster,
.karta-chip-row,
.karta-cli-chip,
.karta-card-foot,
.karta-infra-left,
.karta-infra-stats {
  position: relative;
  z-index: 1;
}

.karta-card-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.karta-icon-box {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: 11px;
  color: var(--vp-c-brand-1);
  background: var(--karta-gradient-subtle);
  border: 1px solid var(--karta-border);
  flex-shrink: 0;
  transition: transform 0.24s cubic-bezier(0.22, 1, 0.36, 1), border-color 0.24s ease;
}

.karta-icon-box svg {
  width: 20px;
  height: 20px;
}

.karta-icon-box.is-compact {
  width: 44px;
  height: 44px;
  border-radius: 12px;
}

.karta-bento-card:hover .karta-icon-box {
  transform: scale(1.04);
  border-color: rgba(251, 146, 60, 0.36);
}

.karta-metric-pill,
.karta-soft-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  font-size: 11.5px;
  font-weight: 600;
  letter-spacing: -0.01em;
  white-space: nowrap;
}

.karta-metric-pill {
  font-family: var(--vp-font-family-mono);
  color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  border: 1px solid rgba(249, 115, 22, 0.24);
}

.karta-metric-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: #f97316;
  box-shadow: 0 0 8px rgba(249, 115, 22, 0.7);
}

.karta-soft-pill {
  color: var(--vp-c-text-2);
  background: var(--karta-surface-soft);
  border: 1px solid var(--karta-border);
}

.karta-card-body {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.karta-card-title {
  margin: 0 !important;
  padding: 0 !important;
  border: none !important;
  font-size: 16.5px !important;
  font-weight: 650;
  letter-spacing: -0.018em;
  line-height: 1.35;
  color: var(--vp-c-text-1);
}

.karta-card-desc {
  margin: 0;
  font-size: 13.75px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}

/* Card 1: 4 Paper Palette Swatches */
.karta-swatch-strip {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-top: 2px;
}

@media (min-width: 520px) {
  .karta-swatch-strip {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
}

.karta-swatch-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
  border-radius: 12px;
  background: var(--karta-surface-soft);
  border: 1px solid var(--karta-border);
  transition: transform 0.24s cubic-bezier(0.22, 1, 0.36, 1), border-color 0.24s ease;
}

.karta-bento-card:hover .karta-swatch-item {
  border-color: var(--karta-border-strong);
}

.karta-mini-paper {
  position: relative;
  height: 44px;
  border-radius: 7px;
  overflow: hidden;
  background: #fefaf5;
  border: 1px solid rgba(120, 88, 66, 0.16);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 6px;
}

.karta-mini-paper.is-frosted {
  background:
    radial-gradient(circle at 20% 20%, rgba(255, 94, 58, 0.34), transparent 62%),
    radial-gradient(circle at 82% 78%, rgba(251, 191, 36, 0.34), transparent 60%),
    #fdf4ec;
}

.karta-mini-paper.is-frosted .karta-mini-header {
  height: 6px;
  width: 44%;
  border-radius: 999px;
  background: rgba(28, 25, 23, 0.14);
}

.karta-mini-paper.is-mesh-glow .karta-mini-header {
  position: absolute;
  inset: 0 0 auto 0;
  height: 18px;
  background:
    radial-gradient(ellipse at 80% 10%, rgba(244, 63, 94, 0.45), transparent 65%),
    radial-gradient(ellipse at 35% 0%, rgba(251, 191, 36, 0.42), transparent 62%);
}

.karta-mini-paper.is-blur .karta-mini-header {
  position: absolute;
  inset: 0 0 auto 0;
  height: 16px;
  background: linear-gradient(125deg, #27272a 0%, #fb7299 55%, #f97316 100%);
  opacity: 0.78;
}

.karta-mini-paper.is-soft-prism .karta-mini-header {
  position: absolute;
  inset: 0 0 auto 0;
  height: 18px;
  background:
    radial-gradient(ellipse at 105% -25%, rgba(244, 63, 94, 0.42) 0 58%, rgba(255, 255, 255, 0.75) 59%, transparent 62%),
    radial-gradient(ellipse at 92% -35%, rgba(249, 115, 22, 0.34) 0 74%, rgba(255, 255, 255, 0.6) 75%, transparent 78%);
}

.karta-mini-lines {
  display: flex;
  flex-direction: column;
  gap: 3px;
  margin-top: auto;
}

.karta-mini-lines i {
  display: block;
  height: 3.5px;
  border-radius: 999px;
  background: rgba(28, 25, 23, 0.2);
}

.karta-mini-lines i.w-lg {
  width: 76%;
}

.karta-mini-lines i.w-sm {
  width: 48%;
  opacity: 0.65;
}

.karta-swatch-label {
  font-family: var(--vp-font-family-mono);
  font-size: 10.5px;
  font-weight: 500;
  color: var(--vp-c-text-2);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Card 2: Platform Cluster */
.karta-platform-cluster {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: auto;
  padding-top: 4px;
}

.karta-plat-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  border-radius: 9px;
  font-size: 12px;
  font-weight: 550;
  color: var(--vp-c-text-1);
  background: var(--karta-surface-soft);
  border: 1px solid var(--karta-border);
}

.karta-plat-dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  flex-shrink: 0;
}

.karta-plat-chip.is-more {
  color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  border-color: rgba(249, 115, 22, 0.26);
  font-weight: 700;
}

/* Card 3 & 5: Mini Badges & Commands */
.karta-chip-row {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  margin-top: auto;
  padding-top: 2px;
}

.karta-mini-badge {
  display: inline-flex;
  align-items: center;
  padding: 4px 9px;
  border-radius: 8px;
  font-size: 11.5px;
  font-weight: 600;
  letter-spacing: -0.01em;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface-soft);
  color: var(--vp-c-text-1);
}

.karta-mini-badge.tone-coral {
  color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  border-color: rgba(249, 115, 22, 0.24);
}

.karta-mini-badge.tone-amber {
  color: var(--karta-amber-badge-color);
  background: rgba(245, 158, 11, 0.12);
  border-color: rgba(245, 158, 11, 0.24);
}

.karta-cmd-chip {
  font-family: var(--vp-font-family-mono);
  font-size: 11.5px;
  font-weight: 500;
  padding: 4px 8px;
  border-radius: 7px;
  color: var(--vp-c-text-1);
  background: var(--karta-surface-soft);
  border: 1px solid var(--karta-border);
}

/* Card 4: Terminal Chip */
.karta-cli-chip {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: auto;
  padding: 9px 12px;
  border-radius: 10px;
  background: var(--karta-code-bg);
  border: 1px solid var(--karta-code-border);
  font-family: var(--vp-font-family-mono);
  font-size: 12px;
  line-height: 1.3;
  overflow: hidden;
}

.karta-cli-prompt {
  color: #fb923c;
  font-weight: 700;
  user-select: none;
}

.karta-cli-cmd {
  color: #e4e4e7;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Card Footer Link Hint */
.karta-card-foot {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin-top: 2px;
  font-size: 13px;
  font-weight: 550;
  color: var(--vp-c-brand-1);
}

.karta-arrow,
.karta-infra-arrow {
  display: inline-block;
  transition: transform 0.2s cubic-bezier(0.22, 1, 0.36, 1);
}

.karta-bento-card:hover .karta-arrow,
.karta-bento-card:hover .karta-infra-arrow {
  transform: translateX(3px);
}

/* Card 6: Full-width Horizontal Highlight Bar */
.karta-card-infra {
  flex-direction: column;
  justify-content: space-between;
  gap: 18px;
  padding: 20px 24px;
}

@media (min-width: 960px) {
  .karta-card-infra {
    flex-direction: row;
    align-items: center;
    gap: 28px;
  }
}

.karta-infra-left {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  min-width: 0;
  flex: 1;
}

.karta-infra-copy {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.karta-infra-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.karta-infra-arrow {
  font-size: 15px;
  font-weight: 600;
  color: var(--vp-c-brand-1);
}

.karta-infra-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  flex-shrink: 0;
}

.karta-stat-pill {
  display: inline-flex;
  align-items: baseline;
  gap: 6px;
  padding: 8px 13px;
  border-radius: 12px;
  background: var(--karta-surface-soft);
  border: 1px solid var(--karta-border);
  white-space: nowrap;
}

.karta-stat-val {
  font-family: var(--vp-font-family-mono);
  font-size: 13px;
  font-weight: 700;
  color: var(--vp-c-text-1);
}

.karta-stat-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--vp-c-text-2);
}

@media (prefers-reduced-motion: reduce) {
  .karta-bento-card,
  .karta-icon-box,
  .karta-swatch-item,
  .karta-arrow,
  .karta-infra-arrow {
    transition: none !important;
  }
  .karta-bento-card:hover,
  .karta-bento-card:hover .karta-icon-box,
  .karta-bento-card:hover .karta-arrow,
  .karta-bento-card:hover .karta-infra-arrow {
    transform: none !important;
  }
}
</style>
