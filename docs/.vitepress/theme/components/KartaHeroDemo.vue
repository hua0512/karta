<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useData } from 'vitepress'

type PresetId = 'bilibili' | 'xiaohongshu' | 'twitter' | 'douyin'
type OutputMode = 'image' | 'text'
type HeaderStyle = 'blur_background' | 'mesh_glow' | 'blur' | 'soft_prism'

interface MetricItem {
  icon: 'play' | 'heart' | 'coin' | 'star' | 'comment' | 'repost' | 'eye'
  labelZh: string
  labelEn: string
  value: string
}

interface Preset {
  id: PresetId
  tabZh: string
  tabEn: string
  platformSlug: string
  accent: string
  frameBg: string
  glowA: string
  glowB: string
  senderZh: string
  senderEn: string
  senderInitial: string
  sentTime: string
  replyTime: string
  chatPrefixZh?: string
  chatPrefixEn?: string
  chatHighlight: string
  latencyMs: number
  cacheHit: boolean
  botReplyCardTitleZh: string
  botReplyCardTitleEn: string
  botReplyMetaZh: string
  botReplyMetaEn: string
  botMediaChipZh: string
  botMediaChipEn: string
  botSecondaryChipZh?: string
  botSecondaryChipEn?: string
  author: string
  authorHandle?: string
  avatarGrad: string
  avatarInitial: string
  cardTime: string
  title: string
  body?: string
  duration?: string
  galleryCaptionZh?: string
  galleryCaptionEn?: string
  readTimeZh?: string
  readTimeEn?: string
  repost?: {
    author: string
    handle: string
    time: string
    text: string
  }
  metrics: MetricItem[]
  textAttachmentsZh: string[]
  textAttachmentsEn: string[]
}

const { lang } = useData()
const isZh = computed(() => lang.value.startsWith('zh'))

const presets: Preset[] = [
  {
    id: 'bilibili',
    tabZh: 'Bilibili 视频',
    tabEn: 'Bilibili Video',
    platformSlug: 'bilibili',
    accent: '#fb7299',
    frameBg: 'linear-gradient(145deg, #f9d7e3 0%, #f5e2d8 52%, #e8d9ef 100%)',
    glowA: '#fb7299',
    glowB: '#fb923c',
    senderZh: '阿澈',
    senderEn: 'Alex',
    senderInitial: 'A',
    sentTime: '20:14',
    replyTime: '20:14',
    chatPrefixZh: '这个 Rust 渲染引擎讲解太丝滑了 ',
    chatPrefixEn: 'Super clean Rust rendering walkthrough ',
    chatHighlight: 'https://b23.tv/YTg9oSw',
    latencyMs: 142,
    cacheHit: false,
    botReplyCardTitleZh: '原生卡片 PNG',
    botReplyCardTitleEn: 'Native Card PNG',
    botReplyMetaZh: '1080×720 · 38 ms 渲染',
    botReplyMetaEn: '1080×720 · 38 ms render',
    botMediaChipZh: '🎬 1080p MP4 · 14.2 MB',
    botMediaChipEn: '🎬 1080p MP4 · 14.2 MB',
    botSecondaryChipZh: '⚡ DASH 流自动合并',
    botSecondaryChipEn: '⚡ DASH merged',
    author: 'Rust 视觉工坊',
    avatarGrad: 'linear-gradient(135deg, #fb7299 0%, #f97316 100%)',
    avatarInitial: 'R',
    cardTime: '2026-04-12 18:30',
    title: '从零用纯 Rust 手写磨砂玻璃卡片渲染器，比无头浏览器快 40 倍！',
    duration: '08:42',
    metrics: [
      { icon: 'play', labelZh: '播放', labelEn: 'Views', value: '128.4万' },
      { icon: 'heart', labelZh: '点赞', labelEn: 'Likes', value: '9.6万' },
      { icon: 'coin', labelZh: '投币', labelEn: 'Coins', value: '4.2万' },
      { icon: 'star', labelZh: '收藏', labelEn: 'Favs', value: '5.8万' },
    ],
    textAttachmentsZh: ['[封面图]', '[视频 08:42 · 1080p AVC]'],
    textAttachmentsEn: ['[Cover Image]', '[Video 08:42 · 1080p AVC]'],
  },
  {
    id: 'xiaohongshu',
    tabZh: '小红书图文',
    tabEn: 'Xiaohongshu',
    platformSlug: 'xiaohongshu',
    accent: '#ff2442',
    frameBg: 'linear-gradient(145deg, #fbd8dc 0%, #fce6d8 55%, #f9dfd2 100%)',
    glowA: '#ff3b5c',
    glowB: '#fbbf24',
    senderZh: '小满',
    senderEn: 'Mina',
    senderInitial: 'M',
    sentTime: '20:16',
    replyTime: '20:16',
    chatPrefixZh: '京都秋日咖啡馆胶片合集 ',
    chatPrefixEn: 'Kyoto autumn film cafes ',
    chatHighlight: 'https://xhslink.com/a/9kP2xM',
    latencyMs: 186,
    cacheHit: false,
    botReplyCardTitleZh: '原生图文卡片 PNG',
    botReplyCardTitleEn: 'Native Gallery Card PNG',
    botReplyMetaZh: '3×2 拼图预览 · 无水印',
    botReplyMetaEn: '3×2 preview · watermark-free',
    botMediaChipZh: '📦 合并转发 · 12 张原图',
    botMediaChipEn: '📦 Merged Forward · 12 Photos',
    botSecondaryChipZh: '🖼️ 自动去水印直链',
    botSecondaryChipEn: '🖼️ Clean originals',
    author: '胶片漫游日记',
    avatarGrad: 'linear-gradient(135deg, #ff2442 0%, #fb7185 100%)',
    avatarInitial: '漫',
    cardTime: '2026-04-11 09:22',
    title: '京都鸭川边 6 家暖木风手冲咖啡馆｜富士胶片直出调色参考',
    galleryCaptionZh: '共 12 张图片',
    galleryCaptionEn: '12 images total',
    metrics: [
      { icon: 'heart', labelZh: '点赞', labelEn: 'Likes', value: '2.4万' },
      { icon: 'star', labelZh: '收藏', labelEn: 'Favs', value: '1.9万' },
      { icon: 'comment', labelZh: '评论', labelEn: 'Comments', value: '648' },
    ],
    textAttachmentsZh: ['[合并转发 · 12 张图片]'],
    textAttachmentsEn: ['[Merged Forward · 12 Images]'],
  },
  {
    id: 'twitter',
    tabZh: 'Twitter/X 转发',
    tabEn: 'Twitter/X Repost',
    platformSlug: 'twitter',
    accent: '#2f3742',
    frameBg: 'linear-gradient(145deg, #dfe4ec 0%, #ece8e3 50%, #dce3ec 100%)',
    glowA: '#475569',
    glowB: '#94a3b8',
    senderZh: 'Lin',
    senderEn: 'Lin',
    senderInitial: 'L',
    sentTime: '20:19',
    replyTime: '20:19',
    chatHighlight: 'https://x.com/rustlang/status/1892410829401',
    latencyMs: 18,
    cacheHit: true,
    botReplyCardTitleZh: '嵌套转发卡片 PNG',
    botReplyCardTitleEn: 'Nested Repost Card PNG',
    botReplyMetaZh: 'LRU 缓存命中 · 零网络请求',
    botReplyMetaEn: 'LRU cache hit · 0 network calls',
    botMediaChipZh: '🖼️ 原生卡片 PNG · 18 ms 缓存',
    botMediaChipEn: '🖼️ Native Card PNG · 18 ms Cache',
    botSecondaryChipZh: '🔁 嵌套引用已展开',
    botSecondaryChipEn: '🔁 Quoted post nested',
    author: 'Rust Language',
    authorHandle: '@rustlang',
    avatarGrad: 'linear-gradient(135deg, #27272a 0%, #52525b 100%)',
    avatarInitial: '🦀',
    cardTime: '2026-04-10 21:05',
    title: 'Async closures & zero-copy image pipelines in production',
    body: 'Love seeing community tools replace heavy Chromium sidecars with single static Rust binaries.',
    repost: {
      author: 'Karta',
      handle: '@karta_rs',
      time: '2026-04-10 19:40',
      text: 'Karta renders frosted warm-paper cards natively in Rust across 16 social platforms — ~40ms per card, zero browser runtime.',
    },
    metrics: [
      { icon: 'eye', labelZh: '浏览', labelEn: 'Views', value: '412.6K' },
      { icon: 'repost', labelZh: '转发', labelEn: 'Reposts', value: '3.1K' },
      { icon: 'heart', labelZh: '点赞', labelEn: 'Likes', value: '18.4K' },
      { icon: 'comment', labelZh: '回复', labelEn: 'Replies', value: '294' },
    ],
    textAttachmentsZh: ['[无附件 · 纯卡片/转发文本]'],
    textAttachmentsEn: ['[Text & Nested Repost]'],
  },
  {
    id: 'douyin',
    tabZh: '抖音长文',
    tabEn: 'Douyin Article',
    platformSlug: 'douyin',
    accent: '#1e979e',
    frameBg: 'linear-gradient(145deg, #cceae8 0%, #e7ece9 52%, #d8e5ee 100%)',
    glowA: '#1e979e',
    glowB: '#38bdf8',
    senderZh: '子墨',
    senderEn: 'Zimo',
    senderInitial: 'Z',
    sentTime: '20:23',
    replyTime: '20:23',
    chatPrefixZh: '8.94 复制打开抖音，即可阅读文章 ',
    chatPrefixEn: '8.94 复制打开抖音，即可阅读文章 ',
    chatHighlight: 'ˇˇ5w0ncBiCzhF99➝➝',
    latencyMs: 214,
    cacheHit: false,
    botReplyCardTitleZh: '长文封面卡片 PNG',
    botReplyCardTitleEn: 'Editorial Cover Card PNG',
    botReplyMetaZh: '口令自动还原 · 匿名解析',
    botReplyMetaEn: 'Share code resolved · anonymous',
    botMediaChipZh: '📄 长文卡片 + 正文节选',
    botMediaChipEn: '📄 Long Article Card + Excerpt',
    botSecondaryChipZh: '📚 合并转发 · 完整图文',
    botSecondaryChipEn: '📚 Forwarded · full article',
    author: '山野影像志',
    avatarGrad: 'linear-gradient(135deg, #1e979e 0%, #0f766e 100%)',
    avatarInitial: '山',
    cardTime: '2026-04-09 14:18',
    title: '川西高海拔徒步 72 小时：轻量化装备清单与营地实拍笔记',
    body: '从海拔 3400 米出发，整套三季徒步装备控制在 4.8kg 以内。本文按日整理了路线海拔、饮水补给点与防风层实测体验……',
    readTimeZh: '约 6 分钟阅读',
    readTimeEn: '6 min read',
    metrics: [
      { icon: 'heart', labelZh: '点赞', labelEn: 'Likes', value: '5.3万' },
      { icon: 'star', labelZh: '收藏', labelEn: 'Favs', value: '3.1万' },
      { icon: 'comment', labelZh: '评论', labelEn: 'Comments', value: '1,420' },
    ],
    textAttachmentsZh: ['[长文头图]', '[合并转发 · 正文与插图]'],
    textAttachmentsEn: ['[Article Poster]', '[Merged Forward · Body & Inline Photos]'],
  },
]

const headerStyles: { id: HeaderStyle; labelZh: string; labelEn: string }[] = [
  { id: 'blur_background', labelZh: '全卡磨砂', labelEn: 'blur_background' },
  { id: 'mesh_glow', labelZh: '弥散光', labelEn: 'mesh_glow' },
  { id: 'blur', labelZh: '顶部模糊', labelEn: 'blur' },
  { id: 'soft_prism', labelZh: '棱镜光带', labelEn: 'soft_prism' },
]

const activeIndex = ref(0)
const outputMode = ref<OutputMode>('image')
const headerStyle = ref<HeaderStyle>('blur_background')
// Default to stage 2 (complete) for clean SSR/SSG initial HTML
const parseStage = ref<0 | 1 | 2>(2)
const autoCycleActive = ref(false)
const reducedMotion = ref(false)

let cycleTimer: ReturnType<typeof setInterval> | null = null
let stageTimers: ReturnType<typeof setTimeout>[] = []

const currentPreset = computed(() => presets[activeIndex.value])

const stages = computed(() => [
  {
    index: 0,
    label: isZh.value ? '1. 识别链接' : '1. Detect',
    sub:
      currentPreset.value.id === 'douyin'
        ? isZh.value
          ? '口令提取'
          : 'Share code'
        : 'RegexSet',
  },
  {
    index: 1,
    label: isZh.value ? '2. 解析 & 缓存' : '2. Parse & Cache',
    sub: currentPreset.value.cacheHit
      ? isZh.value
        ? 'LRU 命中'
        : 'LRU hit'
      : isZh.value
        ? '匿名解析'
        : 'Anonymous',
  },
  {
    index: 2,
    label: isZh.value ? '3. 原生渲染' : '3. Native Render',
    sub:
      outputMode.value === 'image'
        ? headerStyle.value
        : isZh.value
          ? '纯文本'
          : 'TextRenderer',
  },
])

const textOutput = computed(() => {
  const p = currentPreset.value
  const header = `[${p.platformSlug}] @${p.author} | ${p.title}`
  const extra = p.metrics.map((m) => `${m.labelZh}: ${m.value}`).join(' | ')
  const lines = [header, extra]

  if (p.repost) {
    lines.push('')
    lines.push('转发自:')
    lines.push(`  [${p.platformSlug}] @${p.repost.author} | ${p.repost.text}`)
  } else if (p.body) {
    lines.push('')
    lines.push(p.body)
  }

  return lines.join('\n')
})

function clearStageTimers() {
  for (const t of stageTimers) clearTimeout(t)
  stageTimers = []
}

function stopAutoCycle() {
  if (cycleTimer !== null) {
    clearInterval(cycleTimer)
    cycleTimer = null
  }
  autoCycleActive.value = false
}

function runStageAnimation() {
  clearStageTimers()
  if (reducedMotion.value) {
    parseStage.value = 2
    return
  }
  parseStage.value = 0
  stageTimers.push(
    setTimeout(() => {
      parseStage.value = 1
    }, 340)
  )
  stageTimers.push(
    setTimeout(() => {
      parseStage.value = 2
    }, 780)
  )
}

function selectPreset(index: number, fromUser = true) {
  if (fromUser) {
    stopAutoCycle()
  }
  activeIndex.value = index
  runStageAnimation()
}

function replayAnimation() {
  stopAutoCycle()
  runStageAnimation()
}

function setOutputMode(mode: OutputMode) {
  stopAutoCycle()
  outputMode.value = mode
}

function setHeaderStyle(style: HeaderStyle) {
  stopAutoCycle()
  headerStyle.value = style
}

function startAutoCycle() {
  stopAutoCycle()
  if (reducedMotion.value) return
  autoCycleActive.value = true
  cycleTimer = setInterval(() => {
    const next = (activeIndex.value + 1) % presets.length
    selectPreset(next, false)
  }, 6000)
}

function toggleAutoCycle() {
  if (reducedMotion.value) return
  if (autoCycleActive.value) {
    stopAutoCycle()
  } else {
    startAutoCycle()
  }
}

onMounted(() => {
  if (typeof window === 'undefined') return
  const mq = window.matchMedia?.('(prefers-reduced-motion: reduce)')
  if (mq?.matches) {
    reducedMotion.value = true
    autoCycleActive.value = false
    parseStage.value = 2
    return
  }
  startAutoCycle()
})

onBeforeUnmount(() => {
  stopAutoCycle()
  clearStageTimers()
})
</script>

<template>
  <section
    class="karta-demo-shell"
    :aria-label="isZh ? 'Karta 实时解析与卡片渲染演示' : 'Karta interactive parsing and native card showcase'"
  >
    <!-- Top Bar: Label + Unambiguous Preset Tabs + Auto-cycle Toggle -->
    <div class="karta-demo-topbar">
      <div class="karta-demo-heading">
        <span class="karta-live-dot" :class="{ active: autoCycleActive }" aria-hidden="true" />
        <span class="karta-demo-eyebrow">
          {{ isZh ? '交互预览 · 从链接到原生卡片' : 'Interactive Preview · Link to Native Card' }}
        </span>
      </div>

      <div class="karta-preset-tabs" role="tablist" :aria-label="isZh ? '平台示例' : 'Platform presets'">
        <button
          v-for="(preset, idx) in presets"
          :key="preset.id"
          type="button"
          role="tab"
          class="karta-preset-tab karta-focusable"
          :class="{ active: idx === activeIndex }"
          :aria-selected="idx === activeIndex"
          @click="selectPreset(idx, true)"
        >
          <span>{{ isZh ? preset.tabZh : preset.tabEn }}</span>
        </button>

        <button
          v-if="!reducedMotion"
          type="button"
          class="karta-cycle-btn karta-focusable"
          :class="{ playing: autoCycleActive }"
          :aria-label="
            autoCycleActive
              ? isZh
                ? '暂停自动轮播'
                : 'Pause auto-cycle'
              : isZh
                ? '开启自动轮播'
                : 'Resume auto-cycle'
          "
          :title="
            autoCycleActive
              ? isZh
                ? '暂停自动轮播'
                : 'Pause auto-cycle'
              : isZh
                ? '开启自动轮播'
                : 'Resume auto-cycle'
          "
          @click="toggleAutoCycle"
        >
          <svg v-if="autoCycleActive" width="12" height="12" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <rect x="3.5" y="2.5" width="3.2" height="11" rx="1" />
            <rect x="9.3" y="2.5" width="3.2" height="11" rx="1" />
          </svg>
          <svg v-else width="12" height="12" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <path d="M4.2 2.8a1 1 0 0 1 1.5-.86l7.5 4.7a1 1 0 0 1 0 1.72l-7.5 4.7A1 1 0 0 1 4.2 12.2V2.8Z" />
          </svg>
        </button>
      </div>
    </div>

    <!-- Main 2-Column Interactive Stage -->
    <div class="karta-demo-grid">
      <!-- LEFT PANEL: Complete QQ Chat Turn + Compact Pipeline + Delivered Bot Reply -->
      <div class="karta-chat-panel">
        <div class="karta-window-chrome">
          <div class="karta-window-dots" aria-hidden="true">
            <span />
            <span />
            <span />
          </div>
          <span class="karta-window-title">
            {{ isZh ? 'Karta 群聊 · OneBot V11' : 'Karta Group · OneBot V11' }}
          </span>
          <span class="karta-window-badge">QQ</span>
        </div>

        <div class="karta-chat-body">
          <!-- 1. Incoming User Message + Animated Emoji Reaction -->
          <div class="karta-msg-row">
            <div class="karta-msg-avatar">{{ currentPreset.senderInitial }}</div>
            <div class="karta-msg-col">
              <div class="karta-msg-meta">
                <span class="karta-msg-sender">{{ isZh ? currentPreset.senderZh : currentPreset.senderEn }}</span>
                <span class="karta-msg-time karta-tabular">{{ currentPreset.sentTime }}</span>
              </div>
              <div class="karta-msg-bubble">
                <span v-if="currentPreset.chatPrefixZh || currentPreset.chatPrefixEn">
                  {{ isZh ? currentPreset.chatPrefixZh : currentPreset.chatPrefixEn }}
                </span>
                <span class="karta-msg-link">{{ currentPreset.chatHighlight }}</span>
              </div>
              <div class="karta-reaction-row">
                <span class="karta-reaction-pill" :class="{ done: parseStage === 2, parsing: parseStage < 2 }">
                  <span class="karta-reaction-emoji" aria-hidden="true">{{ parseStage < 2 ? '👀' : '✨' }}</span>
                  <span class="karta-reaction-label">
                    {{
                      parseStage < 2
                        ? isZh
                          ? 'Karta 正在解析…'
                          : 'Karta parsing…'
                        : isZh
                          ? 'Karta 已完成'
                          : 'Karta done'
                    }}
                  </span>
                </span>
              </div>
            </div>
          </div>

          <!-- 2. Compact Animated 3-Step Live Parse Pipeline with Replay Button -->
          <div class="karta-stage-box">
            <div class="karta-stage-header">
              <div class="karta-stage-title-wrap">
                <span class="karta-stage-title">
                  {{ isZh ? '实时解析管线' : 'Live Parse Pipeline' }}
                </span>
              </div>
              <div class="karta-stage-actions">
                <span class="karta-latency-badge karta-tabular" :class="{ ready: parseStage === 2 }">
                  <template v-if="parseStage < 2">
                    {{ isZh ? '处理中…' : 'Running…' }}
                  </template>
                  <template v-else>
                    {{ currentPreset.latencyMs }} ms
                    <template v-if="currentPreset.cacheHit">
                      · {{ isZh ? '缓存命中' : 'cache' }}
                    </template>
                  </template>
                </span>
                <button
                  type="button"
                  class="karta-replay-btn karta-focusable"
                  :title="isZh ? '重放解析动画' : 'Replay parse animation'"
                  :aria-label="isZh ? '重放解析动画' : 'Replay parse animation'"
                  @click="replayAnimation"
                >
                  <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                    <path
                      d="M13.2 8a5.2 5.2 0 1 1-1.45-3.6L13.5 6.1M13.5 2.5v3.6H9.9"
                      stroke="currentColor"
                      stroke-width="1.65"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                  </svg>
                </button>
              </div>
            </div>

            <ol class="karta-stages-list">
              <li
                v-for="step in stages"
                :key="step.index"
                class="karta-stage-item"
                :class="{
                  done: parseStage > step.index || (parseStage === 2 && step.index === 2),
                  active: parseStage === step.index && parseStage < 2,
                }"
              >
                <span class="karta-stage-indicator" aria-hidden="true">
                  <svg
                    v-if="parseStage > step.index || (parseStage === 2 && step.index === 2)"
                    width="10"
                    height="10"
                    viewBox="0 0 16 16"
                    fill="none"
                  >
                    <path
                      d="M3.2 8.4 6.5 11.7 13 4.8"
                      stroke="currentColor"
                      stroke-width="2.3"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                  </svg>
                  <span v-else class="karta-stage-pulse" />
                </span>
                <div class="karta-stage-copy">
                  <span class="karta-stage-name">{{ step.label }}</span>
                  <span class="karta-stage-sub">{{ step.sub }}</span>
                </div>
              </li>
            </ol>
          </div>

          <!-- 3. Bot's Delivered Attachment Reply Bubble -->
          <div class="karta-msg-row karta-bot-row" :class="{ ready: parseStage === 2 }">
            <div class="karta-bot-chat-avatar" aria-hidden="true">K</div>
            <div class="karta-msg-col">
              <div class="karta-msg-meta">
                <span class="karta-msg-sender">Karta Bot</span>
                <span class="karta-bot-role-tag">BOT</span>
                <span class="karta-msg-time karta-tabular">{{ currentPreset.replyTime }}</span>
              </div>

              <div class="karta-bot-reply-bubble">
                <!-- Miniature Card Thumbnail Chip -->
                <div class="karta-mini-card-chip">
                  <div
                    class="karta-mini-thumb"
                    :style="{ background: currentPreset.frameBg }"
                    aria-hidden="true"
                  >
                    <div class="karta-mini-paper">
                      <span class="karta-mini-bar-a" :style="{ backgroundColor: currentPreset.accent }" />
                      <span class="karta-mini-bar-b" />
                      <span class="karta-mini-block" :style="{ backgroundColor: currentPreset.accent }" />
                    </div>
                  </div>
                  <div class="karta-mini-card-info">
                    <div class="karta-mini-card-title">
                      {{ isZh ? currentPreset.botReplyCardTitleZh : currentPreset.botReplyCardTitleEn }}
                    </div>
                    <div class="karta-mini-card-meta karta-tabular">
                      {{ isZh ? currentPreset.botReplyMetaZh : currentPreset.botReplyMetaEn }}
                    </div>
                  </div>
                  <span class="karta-mini-delivered" aria-hidden="true">
                    <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                      <path
                        d="M3 8.5 6.3 11.8 13.2 4.6"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                      />
                    </svg>
                  </span>
                </div>

                <!-- Media / Forward Delivery Chips -->
                <div class="karta-delivery-chips">
                  <span class="karta-delivery-chip primary">
                    {{ isZh ? currentPreset.botMediaChipZh : currentPreset.botMediaChipEn }}
                  </span>
                  <span
                    v-if="currentPreset.botSecondaryChipZh"
                    class="karta-delivery-chip"
                  >
                    {{ isZh ? currentPreset.botSecondaryChipZh : currentPreset.botSecondaryChipEn }}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- RIGHT PANEL: Authentic Karta Output (Image Card / Plain Text) -->
      <div class="karta-output-panel">
        <!-- Mini-Toolbar -->
        <div class="karta-output-toolbar">
          <div class="karta-seg-group" role="group" :aria-label="isZh ? '输出模式' : 'Output mode'">
            <button
              type="button"
              class="karta-seg-btn karta-focusable"
              :class="{ active: outputMode === 'image' }"
              @click="setOutputMode('image')"
            >
              {{ isZh ? '图片卡片' : 'Image Card' }}
            </button>
            <button
              type="button"
              class="karta-seg-btn karta-focusable"
              :class="{ active: outputMode === 'text' }"
              @click="setOutputMode('text')"
            >
              {{ isZh ? '纯文本' : 'Text' }}
            </button>
          </div>

          <div
            v-if="outputMode === 'image'"
            class="karta-style-pills"
            role="group"
            :aria-label="isZh ? '卡片背景主题' : 'Card header style'"
          >
            <button
              v-for="st in headerStyles"
              :key="st.id"
              type="button"
              class="karta-style-pill karta-focusable"
              :class="{ active: headerStyle === st.id }"
              @click="setHeaderStyle(st.id)"
            >
              {{ isZh ? st.labelZh : st.labelEn }}
            </button>
          </div>
          <div v-else class="karta-text-hint">
            <code>render_type: "text"</code>
          </div>
        </div>

        <!-- Output Stage (Locked min-height to eliminate vertical jump across all 4 presets) -->
        <div class="karta-output-stage" :class="{ loading: parseStage < 2 }">
          <!-- AUTHENTIC NATIVE IMAGE CARD -->
          <div
            v-if="outputMode === 'image'"
            class="karta-native-frame"
            :class="`style-${headerStyle}`"
            :style="{
              '--card-frame-bg': currentPreset.frameBg,
              '--card-accent': currentPreset.accent,
              '--card-glow-a': currentPreset.glowA,
              '--card-glow-b': currentPreset.glowB,
            }"
          >
            <article class="karta-native-paper" :class="`style-${headerStyle}`">
              <!-- Header Decoration Layers matching crates/karta-core/src/render/image/theme.rs -->
              <div v-if="headerStyle === 'mesh_glow'" class="karta-decor-mesh-glow" aria-hidden="true" />
              <div v-else-if="headerStyle === 'blur'" class="karta-decor-blur" aria-hidden="true" />
              <svg
                v-else-if="headerStyle === 'soft_prism'"
                class="karta-decor-soft-prism"
                viewBox="0 0 420 120"
                preserveAspectRatio="none"
                aria-hidden="true"
              >
                <ellipse cx="386" cy="-54" rx="328" ry="158" :fill="currentPreset.glowB" fill-opacity="0.18" stroke="#ffffff" stroke-opacity="0.45" stroke-width="1.2" vector-effect="non-scaling-stroke" />
                <ellipse cx="428" cy="-38" rx="236" ry="134" :fill="currentPreset.glowA" fill-opacity="0.24" stroke="#ffffff" stroke-opacity="0.55" stroke-width="1.2" vector-effect="non-scaling-stroke" />
                <ellipse cx="470" cy="-26" rx="160" ry="112" :fill="currentPreset.glowB" fill-opacity="0.22" stroke="#ffffff" stroke-opacity="0.6" stroke-width="1.2" vector-effect="non-scaling-stroke" />
              </svg>

              <div class="karta-paper-inner">
                <!-- Author Header + Borderless Top-Right Platform Mark -->
                <header class="karta-card-header">
                  <div class="karta-card-author">
                    <div class="karta-card-avatar" :style="{ background: currentPreset.avatarGrad }">
                      {{ currentPreset.avatarInitial }}
                    </div>
                    <div class="karta-card-author-meta">
                      <div class="karta-card-author-name">
                        {{ currentPreset.author }}
                        <span v-if="currentPreset.authorHandle" class="karta-card-handle">
                          {{ currentPreset.authorHandle }}
                        </span>
                      </div>
                      <div class="karta-card-time karta-tabular">
                        {{ currentPreset.cardTime }}
                        <template v-if="currentPreset.readTimeZh">
                          · {{ isZh ? currentPreset.readTimeZh : currentPreset.readTimeEn }}
                        </template>
                      </div>
                    </div>
                  </div>

                  <!-- Borderless Platform Brand Mark -->
                  <div class="karta-brand-mark" aria-hidden="true">
                    <svg v-if="currentPreset.id === 'bilibili'" width="66" height="22" viewBox="0 0 92 30" fill="none">
                      <path
                        d="M9.4 4.2 13 7.8M22.6 4.2 19 7.8"
                        stroke="#fb7299"
                        stroke-width="2.1"
                        stroke-linecap="round"
                      />
                      <rect x="4.5" y="8" width="23" height="16.5" rx="4.5" stroke="#fb7299" stroke-width="2.1" />
                      <path d="M11.5 14v3.2M20.5 14v3.2" stroke="#fb7299" stroke-width="2.2" stroke-linecap="round" />
                      <text
                        x="33"
                        y="21"
                        fill="#fb7299"
                        font-size="15.5"
                        font-weight="700"
                        font-family="Inter, sans-serif"
                        letter-spacing="-0.4"
                      >
                        bilibili
                      </text>
                    </svg>

                    <svg v-else-if="currentPreset.id === 'xiaohongshu'" width="56" height="22" viewBox="0 0 74 28" fill="none">
                      <text
                        x="72"
                        y="20"
                        text-anchor="end"
                        fill="#ff2442"
                        font-size="16"
                        font-weight="800"
                        font-family="'PingFang SC', 'Microsoft YaHei', sans-serif"
                        letter-spacing="0.5"
                      >
                        小红书
                      </text>
                    </svg>

                    <svg v-else-if="currentPreset.id === 'twitter'" width="19" height="19" viewBox="0 0 24 24" fill="#1d2025">
                      <path
                        d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"
                      />
                    </svg>

                    <svg v-else width="56" height="22" viewBox="0 0 78 28" fill="none">
                      <path
                        d="M15.6 5.2c.6 2.6 2.4 4.4 4.9 4.8v3.1c-1.8-.1-3.4-.7-4.8-1.7v6.6a5.4 5.4 0 1 1-4.8-5.3v3.2a2.3 2.3 0 1 0 1.7 2.2V5.2h3Z"
                        fill="#1e979e"
                      />
                      <text
                        x="26"
                        y="20"
                        fill="#1d2025"
                        font-size="15.5"
                        font-weight="700"
                        font-family="'PingFang SC', 'Microsoft YaHei', sans-serif"
                      >
                        抖音
                      </text>
                    </svg>
                  </div>
                </header>

                <!-- Flush-left Bold Title -->
                <h3 class="karta-card-title">{{ currentPreset.title }}</h3>

                <!-- Preset 1: Bilibili Bespoke Shader Wave + Code Window Cover -->
                <div v-if="currentPreset.id === 'bilibili'" class="karta-video-cover">
                  <svg class="karta-cover-art" viewBox="0 0 480 236" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
                    <defs>
                      <linearGradient id="karta-bili-sky" x1="0" y1="0" x2="1" y2="1">
                        <stop offset="0%" stop-color="#111022" />
                        <stop offset="52%" stop-color="#25173a" />
                        <stop offset="100%" stop-color="#481e2f" />
                      </linearGradient>
                      <radialGradient id="karta-bili-orb" cx="76%" cy="24%" r="58%">
                        <stop offset="0%" stop-color="#fb923c" stop-opacity="0.72" />
                        <stop offset="48%" stop-color="#fb7299" stop-opacity="0.34" />
                        <stop offset="100%" stop-color="#111022" stop-opacity="0" />
                      </radialGradient>
                      <linearGradient id="karta-bili-wave1" x1="0" y1="0" x2="1" y2="0">
                        <stop offset="0%" stop-color="#6366f1" stop-opacity="0.45" />
                        <stop offset="50%" stop-color="#fb7299" stop-opacity="0.65" />
                        <stop offset="100%" stop-color="#fbbf24" stop-opacity="0.6" />
                      </linearGradient>
                      <linearGradient id="karta-bili-wave2" x1="0" y1="0" x2="1" y2="0">
                        <stop offset="0%" stop-color="#f43f5e" stop-opacity="0.25" />
                        <stop offset="55%" stop-color="#fb923c" stop-opacity="0.52" />
                        <stop offset="100%" stop-color="#818cf8" stop-opacity="0.42" />
                      </linearGradient>
                    </defs>
                    <rect width="480" height="236" fill="url(#karta-bili-sky)" />
                    <rect width="480" height="236" fill="url(#karta-bili-orb)" />

                    <!-- Subtle Perspective Grid Lines -->
                    <g stroke="rgba(255,255,255,0.06)" stroke-width="1">
                      <line x1="0" y1="48" x2="480" y2="48" />
                      <line x1="0" y1="96" x2="480" y2="96" />
                      <line x1="0" y1="144" x2="480" y2="144" />
                      <line x1="0" y1="192" x2="480" y2="192" />
                      <line x1="80" y1="0" x2="80" y2="236" />
                      <line x1="160" y1="0" x2="160" y2="236" />
                      <line x1="240" y1="0" x2="240" y2="236" />
                      <line x1="320" y1="0" x2="320" y2="236" />
                      <line x1="400" y1="0" x2="400" y2="236" />
                    </g>

                    <!-- Luminous Coral / Indigo Shader Waves -->
                    <path
                      d="M-20 176 C 72 118, 168 208, 266 138 C 350 78, 422 140, 500 92 L 500 240 L -20 240 Z"
                      fill="url(#karta-bili-wave1)"
                    />
                    <path
                      d="M-20 198 C 90 156, 194 222, 302 162 C 384 116, 444 166, 500 132 L 500 240 L -20 240 Z"
                      fill="url(#karta-bili-wave2)"
                    />
                    <path
                      d="M-10 166 C 84 112, 174 198, 268 134 C 352 76, 424 134, 496 86"
                      fill="none"
                      stroke="rgba(255,237,213,0.45)"
                      stroke-width="1.5"
                    />

                    <!-- Minimalist Code Window Silhouette (Left) -->
                    <rect
                      x="28"
                      y="32"
                      width="186"
                      height="130"
                      rx="12"
                      fill="rgba(16, 15, 28, 0.72)"
                      stroke="rgba(255,255,255,0.16)"
                    />
                    <circle cx="44" cy="46" r="3" fill="#fb7185" />
                    <circle cx="55" cy="46" r="3" fill="#fbbf24" />
                    <circle cx="66" cy="46" r="3" fill="#34d399" />
                    <rect x="86" y="43" width="54" height="5" rx="2.5" fill="rgba(255,255,255,0.22)" />
                    <line x1="28" y1="58" x2="214" y2="58" stroke="rgba(255,255,255,0.08)" />
                    <!-- Syntax Bars -->
                    <rect x="44" y="72" width="46" height="6" rx="3" fill="#fb7299" />
                    <rect x="96" y="72" width="76" height="6" rx="3" fill="rgba(255,255,255,0.55)" />
                    <rect x="54" y="88" width="92" height="6" rx="3" fill="#fbbf24" fill-opacity="0.85" />
                    <rect x="152" y="88" width="40" height="6" rx="3" fill="rgba(255,255,255,0.3)" />
                    <rect x="54" y="104" width="66" height="6" rx="3" fill="#818cf8" />
                    <rect x="126" y="104" width="56" height="6" rx="3" fill="rgba(255,255,255,0.42)" />
                    <rect x="44" y="120" width="112" height="6" rx="3" fill="rgba(255,255,255,0.28)" />
                    <rect x="44" y="136" width="72" height="6" rx="3" fill="#fb923c" fill-opacity="0.85" />

                    <!-- Frosted Native Card Preview Silhouette (Right) -->
                    <rect
                      x="294"
                      y="26"
                      width="156"
                      height="168"
                      rx="16"
                      fill="rgba(254, 250, 245, 0.16)"
                      stroke="rgba(255, 255, 255, 0.28)"
                    />
                    <circle cx="322" cy="54" r="12" fill="#fb7299" fill-opacity="0.88" />
                    <rect x="342" y="47" width="76" height="6.5" rx="3.2" fill="rgba(255,255,255,0.82)" />
                    <rect x="342" y="59" width="48" height="5" rx="2.5" fill="rgba(255,255,255,0.45)" />
                    <rect x="310" y="80" width="124" height="64" rx="9" fill="rgba(255,255,255,0.16)" />
                    <path d="M326 130 L356 102 L382 122 L404 106 L422 130 Z" fill="rgba(255,255,255,0.24)" />
                    <rect x="310" y="156" width="32" height="5" rx="2.5" fill="rgba(255,255,255,0.48)" />
                    <rect x="352" y="156" width="32" height="5" rx="2.5" fill="rgba(255,255,255,0.48)" />
                    <rect x="394" y="156" width="32" height="5" rx="2.5" fill="rgba(255,255,255,0.48)" />
                  </svg>

                  <!-- Authentic Frosted-Glass Circular Play Button -->
                  <div class="karta-play-btn" aria-hidden="true">
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="white">
                      <path d="M8.5 6.2c0-1.1 1.2-1.8 2.2-1.2l8.5 5.3c.9.5.9 1.9 0 2.4l-8.5 5.3c-1 .6-2.2-.1-2.2-1.2V6.2Z" />
                    </svg>
                  </div>
                  <!-- Bottom-Right Duration Pill -->
                  <span class="karta-duration-pill karta-tabular">{{ currentPreset.duration }}</span>
                </div>

                <!-- Preset 2: Xiaohongshu Compact 3×2 Bespoke Editorial SVG Gallery -->
                <div v-else-if="currentPreset.id === 'xiaohongshu'" class="karta-gallery-wrap">
                  <div class="karta-gallery-grid">
                    <!-- Tile 1: Warm Latte Art Cup -->
                    <div class="karta-gallery-tile">
                      <svg viewBox="0 0 160 120" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
                        <defs>
                          <linearGradient id="xhs-wood" x1="0" y1="0" x2="1" y2="1">
                            <stop offset="0%" stop-color="#e8c7a3" />
                            <stop offset="100%" stop-color="#c38e63" />
                          </linearGradient>
                        </defs>
                        <rect width="160" height="120" fill="url(#xhs-wood)" />
                        <!-- Morning Sun Shadow Beam -->
                        <polygon points="0,0 78,0 44,120 0,120" fill="#fff7ed" fill-opacity="0.28" />
                        <!-- Saucer & Cup -->
                        <circle cx="84" cy="64" r="42" fill="#f7efe5" />
                        <circle cx="84" cy="64" r="42" fill="none" stroke="#dcc6b0" stroke-width="2" />
                        <ellipse cx="122" cy="68" rx="10" ry="5" fill="#f7efe5" />
                        <circle cx="84" cy="64" r="30" fill="#6f4228" />
                        <circle cx="84" cy="64" r="26" fill="#9a633e" />
                        <!-- Delicate Cream Tulip Latte Art -->
                        <path
                          d="M84 48 C77 48, 74 56, 84 63 C94 56, 91 48, 84 48 Z"
                          fill="#fef7ec"
                        />
                        <path
                          d="M68 62 C73 72, 80 76, 84 78 C88 76, 95 72, 100 62 C93 66, 88 68, 84 69 C80 68, 75 66, 68 62 Z"
                          fill="#fef7ec"
                          fill-opacity="0.92"
                        />
                        <path d="M84 76 L84 82" stroke="#fef7ec" stroke-width="2.2" stroke-linecap="round" />
                      </svg>
                    </div>

                    <!-- Tile 2: Sunlit Kyoto Wood Window -->
                    <div class="karta-gallery-tile">
                      <svg viewBox="0 0 160 120" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
                        <defs>
                          <linearGradient id="xhs-sun" x1="0" y1="0" x2="1" y2="1">
                            <stop offset="0%" stop-color="#fde68a" />
                            <stop offset="65%" stop-color="#f59e0b" />
                            <stop offset="100%" stop-color="#d97706" />
                          </linearGradient>
                        </defs>
                        <rect width="160" height="120" fill="#efe3d1" />
                        <!-- Window Glow -->
                        <rect x="26" y="14" width="108" height="82" rx="4" fill="url(#xhs-sun)" />
                        <!-- Maple Silhouette Outside -->
                        <circle cx="106" cy="36" r="14" fill="#dc2626" fill-opacity="0.65" />
                        <circle cx="118" cy="48" r="10" fill="#ea580c" fill-opacity="0.6" />
                        <!-- Shoji Wood Lattice -->
                        <g stroke="#6b4226" stroke-width="3.5">
                          <rect x="26" y="14" width="108" height="82" rx="3" fill="none" />
                          <line x1="62" y1="14" x2="62" y2="96" />
                          <line x1="98" y1="14" x2="98" y2="96" />
                          <line x1="26" y1="42" x2="134" y2="42" />
                          <line x1="26" y1="69" x2="134" y2="69" />
                        </g>
                        <!-- Diagonal Floor Sunbeam -->
                        <polygon points="26,96 134,96 154,120 8,120" fill="#fbbf24" fill-opacity="0.34" />
                      </svg>
                    </div>

                    <!-- Tile 3: Vintage Film Camera Roll -->
                    <div class="karta-gallery-tile">
                      <svg viewBox="0 0 160 120" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
                        <rect width="160" height="120" fill="#f5eadc" />
                        <!-- Trailing Film Strip -->
                        <rect x="64" y="42" width="96" height="46" fill="#2b211c" />
                        <rect x="74" y="51" width="26" height="28" rx="2" fill="#d97706" fill-opacity="0.72" />
                        <rect x="106" y="51" width="26" height="28" rx="2" fill="#ea580c" fill-opacity="0.62" />
                        <rect x="138" y="51" width="20" height="28" rx="2" fill="#fbbf24" fill-opacity="0.58" />
                        <!-- Film Sprocket Holes -->
                        <g fill="#f5eadc" fill-opacity="0.88">
                          <rect x="70" y="45" width="5" height="3.5" rx="1" />
                          <rect x="82" y="45" width="5" height="3.5" rx="1" />
                          <rect x="94" y="45" width="5" height="3.5" rx="1" />
                          <rect x="106" y="45" width="5" height="3.5" rx="1" />
                          <rect x="118" y="45" width="5" height="3.5" rx="1" />
                          <rect x="130" y="45" width="5" height="3.5" rx="1" />
                          <rect x="142" y="45" width="5" height="3.5" rx="1" />
                          <rect x="70" y="81.5" width="5" height="3.5" rx="1" />
                          <rect x="82" y="81.5" width="5" height="3.5" rx="1" />
                          <rect x="94" y="81.5" width="5" height="3.5" rx="1" />
                          <rect x="106" y="81.5" width="5" height="3.5" rx="1" />
                          <rect x="118" y="81.5" width="5" height="3.5" rx="1" />
                          <rect x="130" y="81.5" width="5" height="3.5" rx="1" />
                          <rect x="142" y="81.5" width="5" height="3.5" rx="1" />
                        </g>
                        <!-- 35mm Canister -->
                        <rect x="36" y="21" width="16" height="8" rx="2" fill="#44403c" />
                        <rect x="22" y="29" width="44" height="66" rx="5" fill="#f59e0b" />
                        <rect x="22" y="29" width="44" height="12" rx="3" fill="#1c1917" />
                        <rect x="22" y="83" width="44" height="12" rx="3" fill="#1c1917" />
                        <rect x="28" y="48" width="32" height="12" rx="2" fill="#1c1917" />
                        <rect x="33" y="52" width="22" height="4" rx="2" fill="#fbbf24" />
                        <text x="44" y="75" text-anchor="middle" fill="#1c1917" font-size="10" font-weight="800" font-family="Inter, sans-serif">
                          400
                        </text>
                      </svg>
                    </div>

                    <!-- Tile 4: Matcha Bowl & Bamboo Whisk -->
                    <div class="karta-gallery-tile">
                      <svg viewBox="0 0 160 120" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
                        <rect width="160" height="120" fill="#e6dfc8" />
                        <!-- Subtle Tatami Lines -->
                        <g stroke="#d5cbb0" stroke-width="1">
                          <line x1="0" y1="24" x2="160" y2="24" />
                          <line x1="0" y1="48" x2="160" y2="48" />
                          <line x1="0" y1="72" x2="160" y2="72" />
                          <line x1="0" y1="96" x2="160" y2="96" />
                        </g>
                        <!-- Dark Stoneware Chawan Bowl -->
                        <circle cx="70" cy="64" r="38" fill="#292524" />
                        <circle cx="70" cy="64" r="32" fill="#4d7c3f" />
                        <circle cx="70" cy="64" r="27" fill="#65a34e" />
                        <!-- Frothy Matcha Crema -->
                        <circle cx="62" cy="56" r="10" fill="#86c56c" fill-opacity="0.65" />
                        <circle cx="78" cy="68" r="6" fill="#a7dc8f" fill-opacity="0.45" />
                        <!-- Bamboo Whisk Silhouette -->
                        <g transform="translate(122 66) rotate(-18)">
                          <rect x="-5" y="10" width="10" height="26" rx="4" fill="#c89d66" />
                          <ellipse cx="0" cy="-2" rx="13" ry="18" fill="#dfb982" />
                          <path d="M-8 -16 L-5 10 M0 -19 L0 10 M8 -16 L5 10" stroke="#9c7240" stroke-width="1.4" />
                        </g>
                      </svg>
                    </div>

                    <!-- Tile 5: Twilight Bridge Reflection -->
                    <div class="karta-gallery-tile">
                      <svg viewBox="0 0 160 120" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
                        <defs>
                          <linearGradient id="xhs-dusk" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="0%" stop-color="#312e81" />
                            <stop offset="48%" stop-color="#be185d" />
                            <stop offset="76%" stop-color="#fb923c" />
                            <stop offset="100%" stop-color="#1e1b4b" />
                          </linearGradient>
                        </defs>
                        <rect width="160" height="120" fill="url(#xhs-dusk)" />
                        <!-- Distant Hills -->
                        <path d="M0 64 Q42 46 84 62 T160 56 L160 72 L0 72 Z" fill="#1e1b4b" fill-opacity="0.58" />
                        <!-- Arched Wooden Bridge -->
                        <path d="M0 72 Q80 54 160 72 L160 78 Q80 62 0 78 Z" fill="#18122b" />
                        <line x1="32" y1="65" x2="32" y2="75" stroke="#18122b" stroke-width="2.5" />
                        <line x1="80" y1="60" x2="80" y2="70" stroke="#18122b" stroke-width="2.5" />
                        <line x1="128" y1="65" x2="128" y2="75" stroke="#18122b" stroke-width="2.5" />
                        <!-- Warm Lantern Dots -->
                        <circle cx="32" cy="62" r="2.6" fill="#fde68a" />
                        <circle cx="80" cy="57" r="3" fill="#fef08a" />
                        <circle cx="128" cy="62" r="2.6" fill="#fde68a" />
                        <!-- Water Ripples -->
                        <rect x="0" y="76" width="160" height="44" fill="#171430" fill-opacity="0.55" />
                        <g stroke="#fdba74" stroke-linecap="round" stroke-opacity="0.65">
                          <line x1="68" y1="86" x2="92" y2="86" stroke-width="2" />
                          <line x1="72" y1="94" x2="88" y2="94" stroke-width="1.6" />
                          <line x1="25" y1="90" x2="40" y2="90" stroke-width="1.5" />
                          <line x1="120" y1="91" x2="136" y2="91" stroke-width="1.5" />
                          <line x1="62" y1="104" x2="98" y2="104" stroke-width="1.4" stroke-opacity="0.4" />
                        </g>
                      </svg>
                    </div>

                    <!-- Tile 6: Dimmed 6th Tile with Crisp Centered White +7 -->
                    <div class="karta-gallery-tile">
                      <svg viewBox="0 0 160 120" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
                        <defs>
                          <linearGradient id="xhs-tile6" x1="0" y1="0" x2="1" y2="1">
                            <stop offset="0%" stop-color="#9a3412" />
                            <stop offset="55%" stop-color="#b45309" />
                            <stop offset="100%" stop-color="#451a03" />
                          </linearGradient>
                        </defs>
                        <rect width="160" height="120" fill="url(#xhs-tile6)" />
                        <circle cx="52" cy="54" r="26" fill="#fed7aa" fill-opacity="0.32" />
                        <rect x="86" y="34" width="46" height="58" rx="6" fill="#fef3c7" fill-opacity="0.22" />
                      </svg>
                      <div class="karta-tile-more karta-tabular">+7</div>
                    </div>
                  </div>
                  <p class="karta-gallery-caption">
                    {{ isZh ? currentPreset.galleryCaptionZh : currentPreset.galleryCaptionEn }}
                  </p>
                </div>

                <!-- Preset 3: Twitter/X Post + Authentic Nested Repost Box -->
                <div v-else-if="currentPreset.id === 'twitter'" class="karta-twitter-body">
                  <p class="karta-card-body-text">{{ currentPreset.body }}</p>
                  <div v-if="currentPreset.repost" class="karta-repost-box">
                    <div class="karta-repost-bar">
                      {{ isZh ? '转发' : 'Repost' }}
                    </div>
                    <div class="karta-repost-content">
                      <div class="karta-repost-meta">
                        <span class="karta-repost-author">{{ currentPreset.repost.author }}</span>
                        <span class="karta-repost-handle">{{ currentPreset.repost.handle }}</span>
                        <span class="karta-repost-time karta-tabular">· {{ currentPreset.repost.time }}</span>
                      </div>
                      <p class="karta-repost-text">{{ currentPreset.repost.text }}</p>
                    </div>
                  </div>
                </div>

                <!-- Preset 4: Douyin Editorial Article with Alpine Sunrise & Tent Silhouette -->
                <div v-else class="karta-editorial-body">
                  <div class="karta-editorial-cover">
                    <svg viewBox="0 0 480 156" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
                      <defs>
                        <linearGradient id="karta-dy-sky" x1="0" y1="0" x2="0.4" y2="1">
                          <stop offset="0%" stop-color="#0d2331" />
                          <stop offset="48%" stop-color="#1d5f6e" />
                          <stop offset="82%" stop-color="#d97d48" />
                          <stop offset="100%" stop-color="#f6bd60" />
                        </linearGradient>
                        <radialGradient id="karta-dy-sun" cx="74%" cy="54%" r="36%">
                          <stop offset="0%" stop-color="#fef08a" stop-opacity="0.95" />
                          <stop offset="42%" stop-color="#fb923c" stop-opacity="0.45" />
                          <stop offset="100%" stop-color="#fb923c" stop-opacity="0" />
                        </radialGradient>
                      </defs>
                      <rect width="480" height="156" fill="url(#karta-dy-sky)" />
                      <rect width="480" height="156" fill="url(#karta-dy-sun)" />

                      <!-- Fading Dawn Stars -->
                      <g fill="#fef9c3" fill-opacity="0.72">
                        <circle cx="54" cy="24" r="1.4" />
                        <circle cx="118" cy="18" r="1.1" />
                        <circle cx="182" cy="30" r="1.3" />
                        <circle cx="242" cy="16" r="1" />
                      </g>

                      <!-- Golden Sunrise Orb -->
                      <circle cx="356" cy="74" r="24" fill="#fef3c7" fill-opacity="0.94" />

                      <!-- Far Alpine Mountain Ridge + Snow Facets -->
                      <path
                        d="M0 112 L76 58 L142 94 L228 38 L314 90 L396 52 L480 96 L480 156 L0 156 Z"
                        fill="#173e4c"
                      />
                      <path d="M228 38 L254 68 L234 62 L212 72 Z" fill="#fef3c7" fill-opacity="0.34" />
                      <path d="M76 58 L98 80 L80 76 L62 84 Z" fill="#fef3c7" fill-opacity="0.22" />
                      <path d="M396 52 L420 74 L402 70 L384 78 Z" fill="#fde68a" fill-opacity="0.38" />

                      <!-- Mid Valley Ridge -->
                      <path
                        d="M0 124 L112 88 L208 118 L318 82 L416 112 L480 98 L480 156 L0 156 Z"
                        fill="#0f2c36"
                      />

                      <!-- Foreground Camp Slope -->
                      <path
                        d="M0 132 Q130 116 250 134 T480 124 L480 156 L0 156 Z"
                        fill="#081a21"
                      />

                      <!-- Glowing Warm Orange Alpine Tent Silhouette -->
                      <g transform="translate(302 104)">
                        <polygon points="0,24 19,2 38,24" fill="#fb923c" />
                        <polygon points="19,2 38,24 26,24" fill="#c2410c" />
                        <polygon points="14,24 19,12 24,24" fill="#fef08a" />
                        <!-- Soft Warm Tent Ground Glow -->
                        <ellipse cx="19" cy="25" rx="22" ry="3.2" fill="#fdba74" fill-opacity="0.28" />
                      </g>

                      <!-- Minimalist Pine Silhouettes -->
                      <g fill="#081a21">
                        <polygon points="274,128 281,110 288,128" />
                        <polygon points="284,129 290,115 296,129" />
                        <polygon points="348,128 355,112 362,128" />
                      </g>
                    </svg>
                    <span class="karta-editorial-tag">
                      {{ isZh ? '长文章 · 口令解析' : 'Long Article · Share Code' }}
                    </span>
                  </div>
                  <p class="karta-card-body-text karta-editorial-excerpt">{{ currentPreset.body }}</p>
                </div>

                <!-- Hairline Divider + Crisp Stroke-Icon Metrics Footer -->
                <footer class="karta-card-footer">
                  <span v-for="metric in currentPreset.metrics" :key="metric.icon" class="karta-metric">
                    <svg v-if="metric.icon === 'play'" width="14" height="14" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                      <circle cx="10" cy="10" r="7.5" stroke="currentColor" stroke-width="1.6" />
                      <path d="M8.5 7.2 13.2 10l-4.7 2.8V7.2Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
                    </svg>
                    <svg v-else-if="metric.icon === 'heart'" width="14" height="14" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                      <path
                        d="M10 16.2s-6-3.7-6-8.1A3.4 3.4 0 0 1 10 5.9a3.4 3.4 0 0 1 6 2.2c0 4.4-6 8.1-6 8.1Z"
                        stroke="currentColor"
                        stroke-width="1.6"
                        stroke-linejoin="round"
                      />
                    </svg>
                    <svg v-else-if="metric.icon === 'coin'" width="14" height="14" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                      <circle cx="10" cy="10" r="7.5" stroke="currentColor" stroke-width="1.6" />
                      <path d="M10 6.5v7M7.5 8.5h5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
                    </svg>
                    <svg v-else-if="metric.icon === 'star'" width="14" height="14" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                      <path
                        d="m10 3.3 2.1 4.2 4.6.7-3.3 3.2.8 4.6-4.2-2.2-4.2 2.2.8-4.6-3.3-3.2 4.6-.7L10 3.3Z"
                        stroke="currentColor"
                        stroke-width="1.5"
                        stroke-linejoin="round"
                      />
                    </svg>
                    <svg v-else-if="metric.icon === 'comment'" width="14" height="14" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                      <path
                        d="M4.2 5.5h11.6a1.5 1.5 0 0 1 1.5 1.5v6.4a1.5 1.5 0 0 1-1.5 1.5H9.2l-3.4 2.4v-2.4H4.2a1.5 1.5 0 0 1-1.5-1.5V7a1.5 1.5 0 0 1 1.5-1.5Z"
                        stroke="currentColor"
                        stroke-width="1.5"
                        stroke-linejoin="round"
                      />
                    </svg>
                    <svg v-else-if="metric.icon === 'repost'" width="14" height="14" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                      <path
                        d="M4.5 7.5h10l-2.4-2.4M15.5 12.5h-10l2.4 2.4"
                        stroke="currentColor"
                        stroke-width="1.6"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                      />
                    </svg>
                    <svg v-else width="14" height="14" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                      <path
                        d="M2.5 10s3-4.8 7.5-4.8S17.5 10 17.5 10s-3 4.8-7.5 4.8S2.5 10 2.5 10Z"
                        stroke="currentColor"
                        stroke-width="1.5"
                      />
                      <circle cx="10" cy="10" r="2.2" stroke="currentColor" stroke-width="1.5" />
                    </svg>
                    <span class="karta-tabular">{{ metric.value }}</span>
                  </span>
                </footer>
              </div>
            </article>
          </div>

          <!-- AUTHENTIC PLAIN TEXT MODE -->
          <div v-else class="karta-text-output-wrap">
            <div class="karta-bot-reply-header">
              <span class="karta-bot-avatar">K</span>
              <span class="karta-bot-name">Karta Bot</span>
              <span class="karta-bot-tag">TextRenderer</span>
            </div>
            <pre class="karta-text-pre">{{ textOutput }}</pre>
            <div class="karta-text-attachments">
              <span
                v-for="att in isZh ? currentPreset.textAttachmentsZh : currentPreset.textAttachmentsEn"
                :key="att"
                class="karta-attach-chip"
              >
                {{ att }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.karta-demo-shell {
  position: relative;
  border-radius: 22px;
  border: 1px solid var(--karta-border);
  background: var(--karta-demo-shell-bg);
  box-shadow: var(--karta-shadow-lg);
  overflow: hidden;
}

/* Top Bar */
.karta-demo-topbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 13px 18px;
  border-bottom: 1px solid var(--vp-c-divider);
  background: var(--karta-surface-soft);
}

.karta-demo-heading {
  display: inline-flex;
  align-items: center;
  gap: 9px;
}

.karta-live-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: var(--vp-c-text-3);
  transition: background-color 0.25s ease, box-shadow 0.25s ease;
}

.karta-live-dot.active {
  background: #f97316;
  box-shadow: 0 0 0 4px rgba(249, 115, 22, 0.22);
}

.karta-demo-eyebrow {
  font-size: 13px;
  font-weight: 600;
  color: var(--vp-c-text-1);
  letter-spacing: -0.01em;
}

/* Unambiguous Preset Tabs: Solid Warm Coral/Amber Active Fill vs Muted Inactive Pills */
.karta-preset-tabs {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.karta-preset-tab {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 6px 13px;
  border-radius: 999px;
  border: 1px solid var(--karta-border);
  background: transparent;
  color: var(--vp-c-text-3);
  font-size: 12.5px;
  font-weight: 550;
  cursor: pointer;
  transition:
    color 0.18s ease,
    border-color 0.18s ease,
    background-color 0.18s ease,
    box-shadow 0.2s ease,
    transform 0.16s ease;
}

.karta-preset-tab:hover:not(.active) {
  border-color: var(--karta-border-hover);
  background: var(--karta-surface);
  color: var(--vp-c-text-1);
}

.karta-preset-tab.active {
  border-color: transparent;
  background: linear-gradient(125deg, #ff5e3a 0%, #f43f5e 52%, #f97316 100%);
  color: #ffffff;
  font-weight: 650;
  box-shadow:
    0 6px 16px -4px rgba(244, 63, 94, 0.48),
    0 1px 0 rgba(255, 255, 255, 0.26) inset;
}

.karta-cycle-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: 999px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  color: var(--vp-c-text-2);
  cursor: pointer;
  transition: all 0.2s ease;
}

.karta-cycle-btn:hover,
.karta-cycle-btn.playing {
  border-color: var(--karta-border-hover);
  color: var(--vp-c-brand-1);
}

/* Main 2-Column Layout */
.karta-demo-grid {
  display: grid;
  grid-template-columns: minmax(0, 0.94fr) minmax(0, 1.16fr);
}

@media (max-width: 900px) {
  .karta-demo-grid {
    grid-template-columns: 1fr;
  }
}

/* LEFT PANEL: Chat + Pipeline + Bot Delivered Reply */
.karta-chat-panel {
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--vp-c-divider);
  background: var(--karta-surface);
}

@media (max-width: 900px) {
  .karta-chat-panel {
    border-right: none;
    border-bottom: 1px solid var(--vp-c-divider);
  }
}

.karta-window-chrome {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  border-bottom: 1px solid var(--vp-c-divider);
  background: var(--karta-surface-soft);
}

.karta-window-dots {
  display: inline-flex;
  gap: 6px;
}

.karta-window-dots span {
  width: 9px;
  height: 9px;
  border-radius: 999px;
  background: var(--karta-border-strong);
}

.karta-window-title {
  font-size: 12px;
  font-weight: 550;
  color: var(--vp-c-text-2);
}

.karta-window-badge {
  font-size: 10.5px;
  font-weight: 700;
  letter-spacing: 0.04em;
  padding: 2px 7px;
  border-radius: 6px;
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}

.karta-chat-body {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 14px;
  padding: 16px 16px 18px;
  flex: 1;
}

.karta-msg-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.karta-msg-avatar {
  width: 32px;
  height: 32px;
  border-radius: 999px;
  background: linear-gradient(135deg, #6366f1 0%, #8b5cf6 100%);
  color: #ffffff;
  font-size: 12.5px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  box-shadow: 0 4px 10px rgba(99, 102, 241, 0.24);
}

.karta-msg-col {
  flex: 1;
  min-width: 0;
}

.karta-msg-meta {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-bottom: 4px;
}

.karta-msg-sender {
  font-size: 12px;
  font-weight: 600;
  color: var(--vp-c-text-1);
}

.karta-msg-time {
  font-size: 10.5px;
  color: var(--vp-c-text-3);
}

.karta-msg-bubble {
  padding: 9px 12px;
  border-radius: 4px 13px 13px 13px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface-soft);
  font-size: 13px;
  line-height: 1.5;
  color: var(--vp-c-text-1);
  word-break: break-word;
}

.karta-msg-link {
  color: var(--vp-c-brand-1);
  font-family: var(--vp-font-family-mono);
  font-size: 12px;
  font-weight: 550;
  background: var(--vp-c-brand-soft);
  padding: 1px 6px;
  border-radius: 5px;
}

.karta-reaction-row {
  margin-top: 6px;
  display: flex;
  align-items: center;
}

.karta-reaction-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 2px 9px;
  border-radius: 999px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface-soft);
  font-size: 11.5px;
  font-weight: 550;
  color: var(--vp-c-text-2);
  transition: all 0.22s ease;
}

.karta-reaction-pill.parsing {
  border-color: rgba(249, 115, 22, 0.42);
  background: rgba(249, 115, 22, 0.12);
  color: var(--vp-c-brand-1);
}

.karta-reaction-pill.done {
  border-color: rgba(16, 185, 129, 0.35);
  background: rgba(16, 185, 129, 0.1);
  color: #10b981;
}

.karta-reaction-emoji {
  font-size: 12.5px;
  line-height: 1;
}

/* Compact 3-Stage Progress Strip */
.karta-stage-box {
  border-radius: 13px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface-soft);
  padding: 10px 12px;
}

.karta-stage-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.karta-stage-title {
  font-size: 11.5px;
  font-weight: 650;
  color: var(--vp-c-text-2);
  letter-spacing: 0.01em;
}

.karta-stage-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.karta-latency-badge {
  font-size: 11px;
  font-weight: 600;
  padding: 1.5px 8px;
  border-radius: 999px;
  background: var(--karta-surface-muted);
  color: var(--vp-c-text-2);
  border: 1px solid var(--karta-border);
}

.karta-latency-badge.ready {
  color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  border-color: rgba(249, 115, 22, 0.28);
}

.karta-replay-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 7px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  color: var(--vp-c-text-2);
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    color 0.18s ease,
    transform 0.2s ease;
}

.karta-replay-btn:hover {
  border-color: var(--karta-border-hover);
  color: var(--vp-c-brand-1);
}

.karta-replay-btn:active svg {
  transform: rotate(-160deg);
}

.karta-replay-btn svg {
  transition: transform 0.25s ease;
}

.karta-stages-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.karta-stage-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border-radius: 8px;
  border: 1px solid transparent;
  background: transparent;
  opacity: 0.48;
  transition: all 0.22s ease;
}

.karta-stage-item.active {
  opacity: 1;
  border-color: rgba(249, 115, 22, 0.35);
  background: var(--vp-c-brand-soft);
}

.karta-stage-item.done {
  opacity: 1;
  background: var(--karta-surface);
  border-color: var(--karta-border);
}

.karta-stage-indicator {
  width: 18px;
  height: 18px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  background: var(--karta-surface-muted);
  color: var(--vp-c-text-3);
}

.karta-stage-item.active .karta-stage-indicator {
  background: rgba(249, 115, 22, 0.2);
  color: #f97316;
}

.karta-stage-item.done .karta-stage-indicator {
  background: rgba(16, 185, 129, 0.16);
  color: #10b981;
}

.karta-stage-pulse {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: currentColor;
  animation: karta-pulse 1.05s ease-in-out infinite;
}

@keyframes karta-pulse {
  0%,
  100% {
    transform: scale(0.85);
    opacity: 0.55;
  }
  50% {
    transform: scale(1.25);
    opacity: 1;
  }
}

.karta-stage-copy {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.karta-stage-name {
  font-size: 12px;
  font-weight: 600;
  color: var(--vp-c-text-1);
}

.karta-stage-sub {
  font-size: 10.5px;
  font-family: var(--vp-font-family-mono);
  color: var(--vp-c-text-3);
  white-space: nowrap;
}

/* Bot Delivered Reply Bubble */
.karta-bot-row {
  opacity: 0.45;
  transform: translateY(4px);
  transition: opacity 0.26s ease, transform 0.26s ease;
}

.karta-bot-row.ready {
  opacity: 1;
  transform: translateY(0);
}

.karta-bot-chat-avatar {
  width: 32px;
  height: 32px;
  border-radius: 10px;
  background: var(--karta-gradient);
  color: #ffffff;
  font-size: 13px;
  font-weight: 800;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  box-shadow: 0 5px 14px -3px rgba(244, 63, 94, 0.42);
}

.karta-bot-role-tag {
  font-size: 9.5px;
  font-weight: 700;
  letter-spacing: 0.05em;
  padding: 1px 5px;
  border-radius: 4px;
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}

.karta-bot-reply-bubble {
  padding: 10px;
  border-radius: 4px 13px 13px 13px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface-soft);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.karta-mini-card-chip {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 9px;
  border-radius: 9px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
}

.karta-mini-thumb {
  width: 38px;
  height: 38px;
  border-radius: 7px;
  padding: 4px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.karta-mini-paper {
  width: 100%;
  height: 100%;
  border-radius: 4px;
  background: #fefaf5;
  padding: 3px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  box-shadow: 0 2px 5px rgba(28, 22, 20, 0.16);
}

.karta-mini-bar-a {
  width: 65%;
  height: 3px;
  border-radius: 2px;
  opacity: 0.85;
}

.karta-mini-bar-b {
  width: 90%;
  height: 2.5px;
  border-radius: 2px;
  background: rgba(25, 28, 32, 0.25);
}

.karta-mini-block {
  flex: 1;
  width: 100%;
  border-radius: 2.5px;
  opacity: 0.28;
}

.karta-mini-card-info {
  flex: 1;
  min-width: 0;
}

.karta-mini-card-title {
  font-size: 12px;
  font-weight: 650;
  color: var(--vp-c-text-1);
  line-height: 1.3;
}

.karta-mini-card-meta {
  font-size: 11px;
  color: var(--vp-c-text-3);
  margin-top: 1px;
}

.karta-mini-delivered {
  width: 20px;
  height: 20px;
  border-radius: 999px;
  background: rgba(16, 185, 129, 0.14);
  color: #10b981;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.karta-delivery-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.karta-delivery-chip {
  display: inline-flex;
  align-items: center;
  padding: 4px 9px;
  border-radius: 7px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  font-size: 11px;
  font-weight: 550;
  color: var(--vp-c-text-2);
  line-height: 1.3;
}

.karta-delivery-chip.primary {
  border-color: rgba(249, 115, 22, 0.3);
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-text-1);
  font-weight: 600;
}

/* RIGHT PANEL: Output Toolbar + Fixed-Height Card Stage */
.karta-output-panel {
  display: flex;
  flex-direction: column;
  background: var(--karta-surface-soft);
}

.karta-output-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 9px 16px;
  border-bottom: 1px solid var(--vp-c-divider);
  background: var(--karta-surface);
}

.karta-seg-group {
  display: inline-flex;
  padding: 3px;
  border-radius: 10px;
  background: var(--karta-surface-muted);
  border: 1px solid var(--karta-border);
}

.karta-seg-btn {
  padding: 4px 11px;
  border-radius: 7px;
  border: none;
  background: transparent;
  color: var(--vp-c-text-2);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
}

.karta-seg-btn.active {
  background: var(--karta-surface);
  color: var(--vp-c-text-1);
  box-shadow: var(--karta-shadow-sm);
}

.karta-style-pills {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.karta-style-pill {
  padding: 4px 9px;
  border-radius: 999px;
  border: 1px solid var(--karta-border);
  background: transparent;
  color: var(--vp-c-text-2);
  font-size: 11.5px;
  font-weight: 550;
  cursor: pointer;
  transition: all 0.2s ease;
}

.karta-style-pill:hover {
  border-color: var(--karta-border-hover);
  color: var(--vp-c-text-1);
}

.karta-style-pill.active {
  background: var(--vp-c-brand-soft);
  border-color: rgba(249, 115, 22, 0.45);
  color: var(--vp-c-brand-1);
}

.karta-text-hint code {
  font-size: 11.5px;
  padding: 3px 8px;
  border-radius: 6px;
  background: var(--karta-surface-muted);
  color: var(--vp-c-text-2);
}

/* Consistent stage min-height prevents vertical page jumps when cycling presets */
.karta-output-stage {
  padding: 18px;
  flex: 1;
  min-height: 454px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: opacity 0.22s ease, transform 0.22s ease;
}

@media (max-width: 900px) {
  .karta-output-stage {
    min-height: 416px;
  }
}

.karta-output-stage.loading {
  opacity: 0.72;
  transform: scale(0.99);
}

/* ============================================================================
 * Authentic Karta Native Card Anatomy (always renders its true warm-paper look)
 * ========================================================================== */

.karta-native-frame {
  width: 100%;
  max-width: 472px;
  padding: 13px 13px 16px;
  border-radius: 22px;
  background: var(--card-frame-bg);
  position: relative;
  overflow: hidden;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.06);
  transition: background 0.35s ease;
}

.karta-native-frame.style-blur_background::before {
  content: '';
  position: absolute;
  inset: -18px;
  background:
    radial-gradient(circle at 22% 18%, var(--card-glow-a), transparent 58%),
    radial-gradient(circle at 82% 76%, var(--card-glow-b), transparent 60%),
    var(--card-frame-bg);
  opacity: 0.42;
  filter: blur(24px);
  pointer-events: none;
}

.karta-native-paper {
  position: relative;
  border-radius: 18px;
  background: #fefaf5;
  color: #191c20;
  overflow: hidden;
  box-shadow:
    0 14px 32px rgba(28, 22, 20, 0.14),
    0 3px 8px rgba(28, 22, 20, 0.07);
  transition: background 0.35s ease;
}

.karta-native-paper.style-blur_background {
  background: linear-gradient(
    165deg,
    rgba(254, 250, 245, 0.84) 0%,
    rgba(254, 250, 245, 0.89) 55%,
    rgba(252, 246, 240, 0.86) 100%
  );
  backdrop-filter: blur(18px) saturate(135%);
  -webkit-backdrop-filter: blur(18px) saturate(135%);
  box-shadow:
    0 16px 34px rgba(28, 22, 20, 0.16),
    0 0 0 1px rgba(255, 255, 255, 0.55) inset;
}

.karta-decor-mesh-glow {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 118px;
  background:
    radial-gradient(ellipse at 80% 6%, var(--card-glow-a) 0%, transparent 56%),
    radial-gradient(ellipse at 36% -4%, var(--card-glow-b) 0%, transparent 58%);
  opacity: 0.32;
  filter: blur(12px);
  pointer-events: none;
}

.karta-decor-blur {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 98px;
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--card-glow-a) 26%, #fefaf5) 0%,
    color-mix(in srgb, var(--card-glow-b) 14%, #fefaf5) 45%,
    transparent 100%
  );
  pointer-events: none;
}

.karta-decor-soft-prism {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 112px;
  -webkit-mask-image: linear-gradient(180deg, #000 0%, rgba(0, 0, 0, 0.55) 60%, transparent 100%);
  mask-image: linear-gradient(180deg, #000 0%, rgba(0, 0, 0, 0.55) 60%, transparent 100%);
  pointer-events: none;
}

.karta-paper-inner {
  position: relative;
  z-index: 1;
  padding: 16px 17px 13px;
}

.karta-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 10px;
}

.karta-card-author {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.karta-card-avatar {
  width: 34px;
  height: 34px;
  border-radius: 999px;
  color: #ffffff;
  font-size: 13.5px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.karta-card-author-meta {
  min-width: 0;
}

.karta-card-author-name {
  font-size: 13.5px;
  font-weight: 700;
  color: #1d2025;
  line-height: 1.25;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.karta-card-handle {
  font-weight: 500;
  font-size: 12px;
  color: #75686f;
  margin-left: 4px;
}

.karta-card-time {
  font-size: 11px;
  color: #75686f;
  margin-top: 2px;
}

.karta-brand-mark {
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
}

.karta-card-title {
  margin: 0 0 10px;
  font-size: 15px;
  font-weight: 750;
  line-height: 1.4;
  color: #191c20;
  text-align: left;
  letter-spacing: -0.015em;
}

/* Bilibili Cover — tuned height so all 4 presets match ~410px total height */
.karta-video-cover {
  position: relative;
  border-radius: 12px;
  overflow: hidden;
  height: 204px;
  background: #111022;
}

.karta-cover-art {
  width: 100%;
  height: 100%;
  display: block;
}

.karta-play-btn {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 48px;
  height: 48px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.24);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  border: 1px solid rgba(255, 255, 255, 0.44);
  box-shadow: 0 8px 22px rgba(0, 0, 0, 0.32);
  display: flex;
  align-items: center;
  justify-content: center;
}

.karta-duration-pill {
  position: absolute;
  right: 10px;
  bottom: 10px;
  padding: 2px 8px;
  border-radius: 6px;
  background: rgba(15, 15, 20, 0.74);
  backdrop-filter: blur(6px);
  color: #ffffff;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.02em;
}

/* Xiaohongshu Compact 3×2 Bespoke Gallery Grid */
.karta-gallery-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px;
}

.karta-gallery-tile {
  position: relative;
  aspect-ratio: 4 / 3;
  border-radius: 9px;
  overflow: hidden;
  box-shadow: inset 0 0 0 1px rgba(28, 22, 20, 0.07);
}

.karta-gallery-tile svg {
  width: 100%;
  height: 100%;
  display: block;
}

.karta-tile-more {
  position: absolute;
  inset: 0;
  background: rgba(16, 13, 16, 0.62);
  backdrop-filter: blur(1.5px);
  color: #ffffff;
  font-size: 21px;
  font-weight: 750;
  letter-spacing: -0.02em;
  display: flex;
  align-items: center;
  justify-content: center;
}

.karta-gallery-caption {
  margin: 7px 0 0;
  font-size: 11.5px;
  color: #75686f;
  text-align: left;
}

/* Twitter/X Nested Repost Box (matches constants.rs #f7f7f7 / #e6e6e6 / #ebebeb) */
.karta-twitter-body {
  min-height: 194px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 10px;
}

.karta-card-body-text {
  margin: 0;
  font-size: 13px;
  line-height: 1.56;
  color: #333333;
}

.karta-repost-box {
  border-radius: 9px;
  border: 1px solid #e6e6e6;
  background: #f7f7f7;
  overflow: hidden;
}

.karta-repost-bar {
  height: 25px;
  background: #ebebeb;
  color: #666666;
  font-size: 11px;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: center;
}

.karta-repost-content {
  padding: 12px 13px 13px;
}

.karta-repost-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 5px;
  margin-bottom: 6px;
}

.karta-repost-author {
  font-size: 12.5px;
  font-weight: 700;
  color: #1d2025;
}

.karta-repost-handle,
.karta-repost-time {
  font-size: 11px;
  color: #75686f;
}

.karta-repost-text {
  margin: 0;
  font-size: 12.5px;
  line-height: 1.55;
  color: #333333;
}

/* Douyin Editorial Cover */
.karta-editorial-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.karta-editorial-cover {
  position: relative;
  border-radius: 11px;
  overflow: hidden;
  height: 134px;
}

.karta-editorial-cover svg {
  width: 100%;
  height: 100%;
  display: block;
}

.karta-editorial-tag {
  position: absolute;
  left: 10px;
  bottom: 9px;
  padding: 2.5px 8.5px;
  border-radius: 6px;
  background: rgba(8, 26, 33, 0.74);
  backdrop-filter: blur(6px);
  color: #f8fafc;
  font-size: 10.5px;
  font-weight: 600;
}

.karta-editorial-excerpt {
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

/* Hairline Divider + Stroke-Icon Metrics Footer */
.karta-card-footer {
  margin-top: 12px;
  padding-top: 9px;
  border-top: 1px solid rgba(117, 104, 111, 0.16);
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 14px;
}

.karta-metric {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 11.5px;
  font-weight: 550;
  color: #6b6166;
}

/* Text Output Mode */
.karta-text-output-wrap {
  width: 100%;
  max-width: 472px;
  border-radius: 16px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  padding: 16px;
  box-shadow: var(--karta-shadow-md);
}

.karta-bot-reply-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.karta-bot-avatar {
  width: 26px;
  height: 26px;
  border-radius: 8px;
  background: var(--karta-gradient);
  color: #ffffff;
  font-size: 12px;
  font-weight: 800;
  display: flex;
  align-items: center;
  justify-content: center;
}

.karta-bot-name {
  font-size: 13px;
  font-weight: 650;
  color: var(--vp-c-text-1);
}

.karta-bot-tag {
  font-size: 10.5px;
  font-family: var(--vp-font-family-mono);
  padding: 2px 7px;
  border-radius: 6px;
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}

.karta-text-pre {
  margin: 0;
  padding: 14px;
  border-radius: 12px;
  background: var(--karta-code-bg);
  border: 1px solid var(--karta-code-border);
  color: #f4f4f5;
  font-family: var(--vp-font-family-mono);
  font-size: 12.5px;
  line-height: 1.65;
  white-space: pre-wrap;
  word-break: break-word;
}

.karta-text-attachments {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  margin-top: 12px;
}

.karta-attach-chip {
  font-size: 11.5px;
  font-weight: 550;
  padding: 4px 10px;
  border-radius: 8px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface-soft);
  color: var(--vp-c-text-2);
}

@media (prefers-reduced-motion: reduce) {
  .karta-stage-pulse {
    animation: none !important;
  }
  .karta-output-stage,
  .karta-native-frame,
  .karta-native-paper,
  .karta-preset-tab,
  .karta-stage-item,
  .karta-bot-row,
  .karta-replay-btn,
  .karta-replay-btn svg {
    transition: none !important;
  }
}
</style>
