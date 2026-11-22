<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { useData, withBase } from 'vitepress'

type AuthLevel = 'none' | 'optional' | 'recommended' | 'partial' | 'required'
type FilterKey = 'all' | 'nocookie' | 'video' | 'gallery'

interface PlatformItem {
  slug: string
  nameEn: string
  nameZh: string
  shortZh: string
  auth: AuthLevel
  categories: ('video' | 'gallery')[]
  accent: string
  neutralTint?: boolean
  examplesEn: string
  examplesZh: string
}

const props = withDefaults(
  defineProps<{
    compact?: boolean
  }>(),
  {
    compact: false,
  }
)

const { lang } = useData()
const isZh = computed(() => lang.value.startsWith('zh'))

const platforms: PlatformItem[] = [
  {
    slug: 'bilibili',
    nameEn: 'Bilibili',
    nameZh: 'Bilibili 哔哩哔哩',
    shortZh: 'Bilibili',
    auth: 'optional',
    categories: ['video', 'gallery'],
    accent: '#fb7299',
    examplesEn: 'b23.tv/…, BV/av, opus, live, bangumi',
    examplesZh: 'b23.tv/…、BV/av 号、动态、直播、番剧',
  },
  {
    slug: 'douyin',
    nameEn: 'Douyin',
    nameZh: '抖音',
    shortZh: '抖音',
    auth: 'optional',
    categories: ['video', 'gallery'],
    accent: '#1e979e',
    examplesEn: 'v.douyin.com/…, notes, 口令 articles, live',
    examplesZh: 'v.douyin.com/…、图文、长文口令、直播',
  },
  {
    slug: 'twitter',
    nameEn: 'Twitter/X',
    nameZh: 'Twitter/X',
    shortZh: 'Twitter / X',
    auth: 'none',
    categories: ['video', 'gallery'],
    accent: '#52525b',
    neutralTint: true,
    examplesEn: 'x.com/<user>/status/…, reposts',
    examplesZh: 'x.com/…/status/…、推文与嵌套转发',
  },
  {
    slug: 'youtube',
    nameEn: 'YouTube',
    nameZh: 'YouTube',
    shortZh: 'YouTube',
    auth: 'optional',
    categories: ['video'],
    accent: '#ef4444',
    examplesEn: 'youtu.be/…, youtube.com/watch?v=…',
    examplesZh: 'youtu.be/…、youtube.com/watch?v=…',
  },
  {
    slug: 'tiktok',
    nameEn: 'TikTok',
    nameZh: 'TikTok',
    shortZh: 'TikTok',
    auth: 'none',
    categories: ['video'],
    accent: '#14b8a6',
    examplesEn: 'tiktok.com/@user/video/…',
    examplesZh: 'tiktok.com/@user/video/…',
  },
  {
    slug: 'weibo',
    nameEn: 'Weibo',
    nameZh: '微博',
    shortZh: '微博',
    auth: 'none',
    categories: ['video', 'gallery'],
    accent: '#f59e0b',
    examplesEn: 'weibo.com/…, m.weibo.cn, video & articles',
    examplesZh: 'weibo.com/…、微博视频与头条文章',
  },
  {
    slug: 'xiaohongshu',
    nameEn: 'Xiaohongshu',
    nameZh: '小红书',
    shortZh: '小红书',
    auth: 'recommended',
    categories: ['video', 'gallery'],
    accent: '#ff2442',
    examplesEn: 'xhslink.com/…, explore notes & galleries',
    examplesZh: 'xhslink.com/…、图文笔记与视频',
  },
  {
    slug: 'kuaishou',
    nameEn: 'Kuaishou',
    nameZh: '快手',
    shortZh: '快手',
    auth: 'none',
    categories: ['video'],
    accent: '#ff6600',
    examplesEn: 'v.kuaishou.com/…, short-video/…',
    examplesZh: 'v.kuaishou.com/…、短视频链接',
  },
  {
    slug: 'hongguo',
    nameEn: 'Hongguo',
    nameZh: '红果短剧',
    shortZh: '红果短剧',
    auth: 'optional',
    categories: ['video', 'gallery'],
    accent: '#f97316',
    examplesEn: 'hongguoduanju.com/player/…, motion comics',
    examplesZh: '短剧播放页、剧集详情与漫剧图文',
  },
  {
    slug: 'acfun',
    nameEn: 'AcFun',
    nameZh: 'AcFun',
    shortZh: 'AcFun',
    auth: 'none',
    categories: ['video'],
    accent: '#fd4c5d',
    examplesEn: 'acfun.cn/v/ac…',
    examplesZh: 'acfun.cn/v/ac…',
  },
  {
    slug: 'nga',
    nameEn: 'NGA',
    nameZh: 'NGA 玩家社区',
    shortZh: 'NGA',
    auth: 'none',
    categories: ['gallery'],
    accent: '#ca8a04',
    examplesEn: 'nga.178.com/read.php?tid=…, ngabbs.com',
    examplesZh: 'nga.178.com、ngabbs.com、bbs.nga.cn',
  },
  {
    slug: 'xiaoheihe',
    nameEn: 'Xiaoheihe',
    nameZh: '小黑盒',
    shortZh: '小黑盒',
    auth: 'optional',
    categories: ['gallery'],
    accent: '#64748b',
    neutralTint: true,
    examplesEn: 'xiaoheihe.cn/bbs/link/…, game pages',
    examplesZh: 'xiaoheihe.cn 社区帖子与游戏详情页',
  },
  {
    slug: 'instagram',
    nameEn: 'Instagram',
    nameZh: 'Instagram',
    shortZh: 'Instagram',
    auth: 'partial',
    categories: ['video', 'gallery'],
    accent: '#e1306c',
    examplesEn: 'instagram.com/p/…, reels, stories',
    examplesZh: '帖子、Reels 与快拍（Stories）',
  },
  {
    slug: 'v2ex',
    nameEn: 'V2EX',
    nameZh: 'V2EX',
    shortZh: 'V2EX',
    auth: 'none',
    categories: ['gallery'],
    accent: '#71717a',
    neutralTint: true,
    examplesEn: 'v2ex.com/t/<id>',
    examplesZh: 'v2ex.com/t/<id> 主题帖',
  },
  {
    slug: 'wechat',
    nameEn: 'WeChat Articles',
    nameZh: '微信公众号文章',
    shortZh: '微信公众号',
    auth: 'none',
    categories: ['gallery'],
    accent: '#10b981',
    examplesEn: 'mp.weixin.qq.com/s/…',
    examplesZh: 'mp.weixin.qq.com/s/… 图文文章',
  },
  {
    slug: 'wechatchannel',
    nameEn: 'WeChat Channels',
    nameZh: '微信视频号',
    shortZh: '微信视频号',
    auth: 'required',
    categories: ['video'],
    accent: '#f59e0b',
    examplesEn: 'weixin.qq.com/sph/…',
    examplesZh: 'weixin.qq.com/sph/…（需元宝 Cookie）',
  },
]

const marqueeRow1 = platforms.slice(0, 8)
const marqueeRow2 = platforms.slice(8, 16)

const activeFilter = ref<FilterKey>('all')
const copiedSlug = ref<string | null>(null)
let copyTimer: ReturnType<typeof setTimeout> | null = null

const filters = computed<{ key: FilterKey; label: string; count: number }[]>(() => [
  {
    key: 'all',
    label: isZh.value ? '全部' : 'All',
    count: platforms.length,
  },
  {
    key: 'nocookie',
    label: isZh.value ? '免 Cookie' : 'No Cookie',
    count: platforms.filter((p) => p.auth === 'none').length,
  },
  {
    key: 'video',
    label: isZh.value ? '视频与直播' : 'Video & Live',
    count: platforms.filter((p) => p.categories.includes('video')).length,
  },
  {
    key: 'gallery',
    label: isZh.value ? '图文与文章' : 'Galleries & Articles',
    count: platforms.filter((p) => p.categories.includes('gallery')).length,
  },
])

const visiblePlatforms = computed(() => {
  switch (activeFilter.value) {
    case 'nocookie':
      return platforms.filter((p) => p.auth === 'none')
    case 'video':
      return platforms.filter((p) => p.categories.includes('video'))
    case 'gallery':
      return platforms.filter((p) => p.categories.includes('gallery'))
    default:
      return platforms
  }
})

function authLabel(auth: AuthLevel): string {
  if (isZh.value) {
    switch (auth) {
      case 'none':
        return '免 Cookie'
      case 'optional':
        return 'Cookie 可选'
      case 'recommended':
        return '推荐 Cookie'
      case 'partial':
        return '部分需 Cookie'
      case 'required':
        return '需 Cookie'
    }
  }
  switch (auth) {
    case 'none':
      return 'No Cookie'
    case 'optional':
      return 'Optional'
    case 'recommended':
      return 'Recommended'
    case 'partial':
      return 'Partial'
    case 'required':
      return 'Required'
  }
}

function platformHref(slug: string): string {
  return withBase(isZh.value ? `/zh/platforms#${slug}` : `/platforms#${slug}`)
}

const allPlatformsHref = computed(() =>
  withBase(isZh.value ? '/zh/platforms' : '/platforms')
)

async function copySlug(slug: string) {
  if (typeof window === 'undefined') return
  try {
    if (navigator?.clipboard?.writeText) {
      await navigator.clipboard.writeText(slug)
    } else {
      const textarea = document.createElement('textarea')
      textarea.value = slug
      textarea.style.position = 'fixed'
      textarea.style.opacity = '0'
      document.body.appendChild(textarea)
      textarea.select()
      document.execCommand('copy')
      document.body.removeChild(textarea)
    }
    copiedSlug.value = slug
    if (copyTimer) clearTimeout(copyTimer)
    copyTimer = setTimeout(() => {
      copiedSlug.value = null
    }, 1600)
  } catch {
    // Ignore clipboard failure in restricted contexts
  }
}

onBeforeUnmount(() => {
  if (copyTimer) clearTimeout(copyTimer)
})
</script>

<template>
  <section
    class="karta-platform-section"
    :class="{ compact: props.compact }"
    :aria-label="isZh ? '支持的 16 个平台' : 'Supported 16 platforms'"
  >
    <!-- ================================================================== -->
    <!-- COMPACT MODE (Landing Page): Sleek Dual-Row Infinite Marquee       -->
    <!-- ================================================================== -->
    <template v-if="props.compact">
      <div class="karta-marquee-header">
        <div>
          <span class="karta-pgrid-kicker">
            {{ isZh ? '平台生态' : 'PLATFORM MATRIX' }}
          </span>
          <h2 class="karta-pgrid-title">
            {{ isZh ? '开箱支持 16 个主流内容平台' : '16 supported platforms out of the box' }}
          </h2>
        </div>

        <a :href="allPlatformsHref" class="karta-explore-btn karta-focusable">
          <span>
            {{ isZh ? '查看全部 16 个平台与配置说明' : 'Explore all 16 platforms' }}
          </span>
          <span class="karta-explore-arrow" aria-hidden="true">→</span>
        </a>
      </div>

      <div class="karta-marquee-stage">
        <!-- Row 1: Scrolls Left -->
        <div class="karta-marquee-row">
          <div class="karta-marquee-track dir-left">
            <div v-for="copy in 2" :key="`r1-${copy}`" class="karta-marquee-group" :aria-hidden="copy === 2">
              <a
                v-for="p in marqueeRow1"
                :key="`r1-${copy}-${p.slug}`"
                :href="platformHref(p.slug)"
                :tabindex="copy === 2 ? -1 : undefined"
                class="karta-marquee-pill karta-focusable"
              >
                <span
                  class="karta-icon-tile"
                  :class="{ neutral: p.neutralTint }"
                  :style="{ '--p-accent': p.accent }"
                  aria-hidden="true"
                >
                  <!-- Bilibili -->
                  <svg v-if="p.slug === 'bilibili'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <path d="M8 4.2 10.6 7M16 4.2 13.4 7" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" />
                    <rect x="3.5" y="7" width="17" height="12.5" rx="3.4" stroke="currentColor" stroke-width="1.9" />
                    <path d="M9 12v2.4M15 12v2.4" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" />
                  </svg>
                  <!-- Douyin -->
                  <svg v-else-if="p.slug === 'douyin'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <path
                      d="M14.4 4.2c.6 2.5 2.4 4.1 4.6 4.5v3a7.2 7.2 0 0 1-4.5-1.5v5.6a5.1 5.1 0 1 1-4.5-5v3a2.2 2.2 0 1 0 1.6 2.1V4.2h2.8Z"
                      fill="currentColor"
                    />
                  </svg>
                  <!-- Twitter / X -->
                  <svg v-else-if="p.slug === 'twitter'" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                    <path
                      d="M18.24 2.5h3.22l-7.04 8.05 8.28 10.95h-6.48l-5.08-6.64-5.81 6.64H2.1l7.53-8.6L1.7 2.5h6.64l4.6 6.07L18.24 2.5Zm-1.13 17.07h1.79L7.35 4.33H5.43l11.68 15.24Z"
                    />
                  </svg>
                  <!-- YouTube -->
                  <svg v-else-if="p.slug === 'youtube'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <rect x="2.8" y="5.5" width="18.4" height="13" rx="3.8" stroke="currentColor" stroke-width="1.9" />
                    <path d="M10.2 9.2 15.2 12l-5 2.8V9.2Z" fill="currentColor" />
                  </svg>
                  <!-- TikTok -->
                  <svg v-else-if="p.slug === 'tiktok'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <path
                      d="M15.4 4v9.8a4.2 4.2 0 1 1-3.2-4.1"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <path
                      d="M15.4 4c.8 2.4 2.5 3.9 4.6 4.2"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                    />
                  </svg>
                  <!-- Weibo -->
                  <svg v-else-if="p.slug === 'weibo'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <path
                      d="M10.5 18.8c-4.2 0-7.3-2-7.3-4.7 0-1.9 1.6-4.2 3.9-5.8 1.5-1 2.9-.8 2.6.5-.2.7 0 .9.8.6 1.7-.7 3.7-.4 4.1.8.2.5.1.9.7 1 1.6.4 2.6 1.5 2.6 2.9 0 2.7-3.2 4.7-7.4 4.7Z"
                      stroke="currentColor"
                      stroke-width="1.8"
                      stroke-linejoin="round"
                    />
                    <circle cx="9.8" cy="14.2" r="1.5" fill="currentColor" />
                    <path d="M15.6 5.6a5.4 5.4 0 0 1 4.8 4.8M15.6 8.6a2.6 2.6 0 0 1 2 2" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" />
                  </svg>
                  <!-- Xiaohongshu -->
                  <svg v-else-if="p.slug === 'xiaohongshu'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <rect x="4" y="3.8" width="16" height="16.4" rx="3.4" stroke="currentColor" stroke-width="1.9" />
                    <path d="M8.5 3.8v16.4M12 9.2h4.6M12 13h4.6M12 16.2h2.8" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
                  </svg>
                  <!-- Kuaishou -->
                  <svg v-else width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <circle cx="8" cy="7.4" r="2.6" stroke="currentColor" stroke-width="1.8" />
                    <circle cx="14.2" cy="7.4" r="2.6" stroke="currentColor" stroke-width="1.8" />
                    <rect x="4.5" y="10.4" width="12.2" height="8.6" rx="2.4" stroke="currentColor" stroke-width="1.8" />
                    <path d="m16.7 13 3.4-1.8v7l-3.4-1.8" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />
                  </svg>
                </span>

                <span class="karta-marquee-name">{{ isZh ? p.shortZh : p.nameEn }}</span>
                <span class="karta-auth-badge" :class="`auth-${p.auth}`">
                  {{ authLabel(p.auth) }}
                </span>
              </a>
            </div>
          </div>
        </div>

        <!-- Row 2: Scrolls Right -->
        <div class="karta-marquee-row">
          <div class="karta-marquee-track dir-right">
            <div v-for="copy in 2" :key="`r2-${copy}`" class="karta-marquee-group" :aria-hidden="copy === 2">
              <a
                v-for="p in marqueeRow2"
                :key="`r2-${copy}-${p.slug}`"
                :href="platformHref(p.slug)"
                :tabindex="copy === 2 ? -1 : undefined"
                class="karta-marquee-pill karta-focusable"
              >
                <span
                  class="karta-icon-tile"
                  :class="{ neutral: p.neutralTint }"
                  :style="{ '--p-accent': p.accent }"
                  aria-hidden="true"
                >
                  <!-- Hongguo -->
                  <svg v-if="p.slug === 'hongguo'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <path d="M12 6.4c0-1.8 1.2-3 2.8-3.2" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
                    <path d="M9.4 4.8c1 .2 2 .8 2.6 1.6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
                    <circle cx="12" cy="13.4" r="6.8" stroke="currentColor" stroke-width="1.9" />
                    <path d="M10.5 11.2 14.8 13.4l-4.3 2.2v-4.4Z" fill="currentColor" />
                  </svg>
                  <!-- AcFun -->
                  <svg v-else-if="p.slug === 'acfun'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <path d="M4.2 17.2 8.2 6.8l3.8 10.4M5.7 13.6h4.9" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" />
                    <path d="M19.8 9.2a4.6 4.6 0 1 0 0 5.6" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" />
                  </svg>
                  <!-- NGA -->
                  <svg v-else-if="p.slug === 'nga'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <path
                      d="M12 3.2 19.2 6.2v5.4c0 4.4-3 7.6-7.2 9.2-4.2-1.6-7.2-4.8-7.2-9.2V6.2L12 3.2Z"
                      stroke="currentColor"
                      stroke-width="1.85"
                      stroke-linejoin="round"
                    />
                    <path d="M9.2 14.8V9.2l5.6 5.6V9.2" stroke="currentColor" stroke-width="1.85" stroke-linecap="round" stroke-linejoin="round" />
                  </svg>
                  <!-- Xiaoheihe (3D Cube) -->
                  <svg v-else-if="p.slug === 'xiaoheihe'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <path
                      d="M12 3.2 19.6 7.5v9L12 20.8 4.4 16.5v-9L12 3.2Z"
                      stroke="currentColor"
                      stroke-width="1.85"
                      stroke-linejoin="round"
                    />
                    <path d="M4.4 7.5 12 11.8l7.6-4.3M12 11.8v9" stroke="currentColor" stroke-width="1.85" stroke-linejoin="round" />
                  </svg>
                  <!-- Instagram -->
                  <svg v-else-if="p.slug === 'instagram'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <rect x="3.5" y="3.5" width="17" height="17" rx="4.8" stroke="currentColor" stroke-width="1.9" />
                    <circle cx="12" cy="12" r="3.8" stroke="currentColor" stroke-width="1.9" />
                    <circle cx="17" cy="7" r="1.15" fill="currentColor" />
                  </svg>
                  <!-- V2EX -->
                  <svg v-else-if="p.slug === 'v2ex'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <path d="M4 7.2 8 16.8l4-9.6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
                    <path
                      d="M14.2 9.4c0-1.4 1.2-2.2 2.7-2.2s2.7.8 2.7 2.2c0 2-3.4 3.4-5.4 7.2h5.6"
                      stroke="currentColor"
                      stroke-width="1.9"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                  </svg>
                  <!-- WeChat Articles -->
                  <svg v-else-if="p.slug === 'wechat'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <path
                      d="M9.6 15.6c-3.4 0-6.1-2.2-6.1-5 0-2.8 2.7-5 6.1-5s6.1 2.2 6.1 5c0 .7-.2 1.4-.5 2"
                      stroke="currentColor"
                      stroke-width="1.8"
                      stroke-linecap="round"
                    />
                    <path
                      d="M15.4 11c2.8 0 5.1 1.8 5.1 4.1 0 1.2-.6 2.3-1.6 3l.4 1.7-1.9-.9c-.6.2-1.3.3-2 .3-2.8 0-5.1-1.8-5.1-4.1S12.6 11 15.4 11Z"
                      stroke="currentColor"
                      stroke-width="1.8"
                      stroke-linejoin="round"
                    />
                    <circle cx="7.8" cy="9.6" r="1" fill="currentColor" />
                    <circle cx="11.4" cy="9.6" r="1" fill="currentColor" />
                  </svg>
                  <!-- WeChat Channels -->
                  <svg v-else width="18" height="18" viewBox="0 0 24 24" fill="none">
                    <path
                      d="M14.2 4.6c-3.8 0-6.2 2.4-6.2 5.4 0 2.1 1.4 3.6 3.5 3.6 1.7 0 2.8-1 2.8-2.3 0-1.1-.8-1.9-1.9-1.9"
                      stroke="currentColor"
                      stroke-width="1.9"
                      stroke-linecap="round"
                    />
                    <path
                      d="M9.8 19.4c3.8 0 6.2-2.4 6.2-5.4 0-2.1-1.4-3.6-3.5-3.6-1.7 0-2.8 1-2.8 2.3 0 1.1.8 1.9 1.9 1.9"
                      stroke="currentColor"
                      stroke-width="1.9"
                      stroke-linecap="round"
                    />
                  </svg>
                </span>

                <span class="karta-marquee-name">{{ isZh ? p.shortZh : p.nameEn }}</span>
                <span class="karta-auth-badge" :class="`auth-${p.auth}`">
                  {{ authLabel(p.auth) }}
                </span>
              </a>
            </div>
          </div>
        </div>
      </div>
    </template>

    <!-- ================================================================== -->
    <!-- FULL MODE (platforms.md / zh/platforms.md): Filterable 16-Card Grid -->
    <!-- ================================================================== -->
    <template v-else>
      <div class="karta-pgrid-header">
        <div>
          <p class="karta-pgrid-sub">
            {{
              isZh
                ? '点击 slug 标签复制平台标识符（可用于 disabled_platforms 或 /disable）；点击名称查看详情。'
                : 'Click any slug chip to copy it for disabled_platforms or /disable; click a title for platform details.'
            }}
          </p>
        </div>

        <div class="karta-filter-bar" role="group" :aria-label="isZh ? '平台筛选' : 'Filter platforms'">
          <button
            v-for="f in filters"
            :key="f.key"
            type="button"
            class="karta-filter-chip karta-focusable"
            :class="{ active: activeFilter === f.key }"
            :aria-pressed="activeFilter === f.key"
            @click="activeFilter = f.key"
          >
            <span>{{ f.label }}</span>
            <span class="karta-filter-count karta-tabular">{{ f.count }}</span>
          </button>
        </div>
      </div>

      <div class="karta-platform-grid">
        <article v-for="p in visiblePlatforms" :key="p.slug" class="karta-platform-card">
          <div class="karta-pcard-top">
            <a :href="platformHref(p.slug)" class="karta-pcard-title karta-focusable">
              <span
                class="karta-icon-tile"
                :class="{ neutral: p.neutralTint }"
                :style="{ '--p-accent': p.accent }"
                aria-hidden="true"
              >
                <!-- Bilibili -->
                <svg v-if="p.slug === 'bilibili'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path d="M8 4.2 10.6 7M16 4.2 13.4 7" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" />
                  <rect x="3.5" y="7" width="17" height="12.5" rx="3.4" stroke="currentColor" stroke-width="1.9" />
                  <path d="M9 12v2.4M15 12v2.4" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" />
                </svg>
                <!-- Douyin -->
                <svg v-else-if="p.slug === 'douyin'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path
                    d="M14.4 4.2c.6 2.5 2.4 4.1 4.6 4.5v3a7.2 7.2 0 0 1-4.5-1.5v5.6a5.1 5.1 0 1 1-4.5-5v3a2.2 2.2 0 1 0 1.6 2.1V4.2h2.8Z"
                    fill="currentColor"
                  />
                </svg>
                <!-- Twitter / X -->
                <svg v-else-if="p.slug === 'twitter'" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                  <path
                    d="M18.24 2.5h3.22l-7.04 8.05 8.28 10.95h-6.48l-5.08-6.64-5.81 6.64H2.1l7.53-8.6L1.7 2.5h6.64l4.6 6.07L18.24 2.5Zm-1.13 17.07h1.79L7.35 4.33H5.43l11.68 15.24Z"
                  />
                </svg>
                <!-- YouTube -->
                <svg v-else-if="p.slug === 'youtube'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <rect x="2.8" y="5.5" width="18.4" height="13" rx="3.8" stroke="currentColor" stroke-width="1.9" />
                  <path d="M10.2 9.2 15.2 12l-5 2.8V9.2Z" fill="currentColor" />
                </svg>
                <!-- TikTok -->
                <svg v-else-if="p.slug === 'tiktok'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path
                    d="M15.4 4v9.8a4.2 4.2 0 1 1-3.2-4.1"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                  <path
                    d="M15.4 4c.8 2.4 2.5 3.9 4.6 4.2"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                  />
                </svg>
                <!-- Weibo -->
                <svg v-else-if="p.slug === 'weibo'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path
                    d="M10.5 18.8c-4.2 0-7.3-2-7.3-4.7 0-1.9 1.6-4.2 3.9-5.8 1.5-1 2.9-.8 2.6.5-.2.7 0 .9.8.6 1.7-.7 3.7-.4 4.1.8.2.5.1.9.7 1 1.6.4 2.6 1.5 2.6 2.9 0 2.7-3.2 4.7-7.4 4.7Z"
                    stroke="currentColor"
                    stroke-width="1.8"
                    stroke-linejoin="round"
                  />
                  <circle cx="9.8" cy="14.2" r="1.5" fill="currentColor" />
                  <path d="M15.6 5.6a5.4 5.4 0 0 1 4.8 4.8M15.6 8.6a2.6 2.6 0 0 1 2 2" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" />
                </svg>
                <!-- Xiaohongshu -->
                <svg v-else-if="p.slug === 'xiaohongshu'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <rect x="4" y="3.8" width="16" height="16.4" rx="3.4" stroke="currentColor" stroke-width="1.9" />
                  <path d="M8.5 3.8v16.4M12 9.2h4.6M12 13h4.6M12 16.2h2.8" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
                </svg>
                <!-- Kuaishou -->
                <svg v-else-if="p.slug === 'kuaishou'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <circle cx="8" cy="7.4" r="2.6" stroke="currentColor" stroke-width="1.8" />
                  <circle cx="14.2" cy="7.4" r="2.6" stroke="currentColor" stroke-width="1.8" />
                  <rect x="4.5" y="10.4" width="12.2" height="8.6" rx="2.4" stroke="currentColor" stroke-width="1.8" />
                  <path d="m16.7 13 3.4-1.8v7l-3.4-1.8" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />
                </svg>
                <!-- Hongguo -->
                <svg v-else-if="p.slug === 'hongguo'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path d="M12 6.4c0-1.8 1.2-3 2.8-3.2" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
                  <path d="M9.4 4.8c1 .2 2 .8 2.6 1.6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
                  <circle cx="12" cy="13.4" r="6.8" stroke="currentColor" stroke-width="1.9" />
                  <path d="M10.5 11.2 14.8 13.4l-4.3 2.2v-4.4Z" fill="currentColor" />
                </svg>
                <!-- AcFun -->
                <svg v-else-if="p.slug === 'acfun'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path d="M4.2 17.2 8.2 6.8l3.8 10.4M5.7 13.6h4.9" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" />
                  <path d="M19.8 9.2a4.6 4.6 0 1 0 0 5.6" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" />
                </svg>
                <!-- NGA -->
                <svg v-else-if="p.slug === 'nga'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path
                    d="M12 3.2 19.2 6.2v5.4c0 4.4-3 7.6-7.2 9.2-4.2-1.6-7.2-4.8-7.2-9.2V6.2L12 3.2Z"
                    stroke="currentColor"
                    stroke-width="1.85"
                    stroke-linejoin="round"
                  />
                  <path d="M9.2 14.8V9.2l5.6 5.6V9.2" stroke="currentColor" stroke-width="1.85" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
                <!-- Xiaoheihe -->
                <svg v-else-if="p.slug === 'xiaoheihe'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path
                    d="M12 3.2 19.6 7.5v9L12 20.8 4.4 16.5v-9L12 3.2Z"
                    stroke="currentColor"
                    stroke-width="1.85"
                    stroke-linejoin="round"
                  />
                  <path d="M4.4 7.5 12 11.8l7.6-4.3M12 11.8v9" stroke="currentColor" stroke-width="1.85" stroke-linejoin="round" />
                </svg>
                <!-- Instagram -->
                <svg v-else-if="p.slug === 'instagram'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <rect x="3.5" y="3.5" width="17" height="17" rx="4.8" stroke="currentColor" stroke-width="1.9" />
                  <circle cx="12" cy="12" r="3.8" stroke="currentColor" stroke-width="1.9" />
                  <circle cx="17" cy="7" r="1.15" fill="currentColor" />
                </svg>
                <!-- V2EX -->
                <svg v-else-if="p.slug === 'v2ex'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path d="M4 7.2 8 16.8l4-9.6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
                  <path
                    d="M14.2 9.4c0-1.4 1.2-2.2 2.7-2.2s2.7.8 2.7 2.2c0 2-3.4 3.4-5.4 7.2h5.6"
                    stroke="currentColor"
                    stroke-width="1.9"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
                <!-- WeChat Articles -->
                <svg v-else-if="p.slug === 'wechat'" width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path
                    d="M9.6 15.6c-3.4 0-6.1-2.2-6.1-5 0-2.8 2.7-5 6.1-5s6.1 2.2 6.1 5c0 .7-.2 1.4-.5 2"
                    stroke="currentColor"
                    stroke-width="1.8"
                    stroke-linecap="round"
                  />
                  <path
                    d="M15.4 11c2.8 0 5.1 1.8 5.1 4.1 0 1.2-.6 2.3-1.6 3l.4 1.7-1.9-.9c-.6.2-1.3.3-2 .3-2.8 0-5.1-1.8-5.1-4.1S12.6 11 15.4 11Z"
                    stroke="currentColor"
                    stroke-width="1.8"
                    stroke-linejoin="round"
                  />
                  <circle cx="7.8" cy="9.6" r="1" fill="currentColor" />
                  <circle cx="11.4" cy="9.6" r="1" fill="currentColor" />
                </svg>
                <!-- WeChat Channels -->
                <svg v-else width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path
                    d="M14.2 4.6c-3.8 0-6.2 2.4-6.2 5.4 0 2.1 1.4 3.6 3.5 3.6 1.7 0 2.8-1 2.8-2.3 0-1.1-.8-1.9-1.9-1.9"
                    stroke="currentColor"
                    stroke-width="1.9"
                    stroke-linecap="round"
                  />
                  <path
                    d="M9.8 19.4c3.8 0 6.2-2.4 6.2-5.4 0-2.1-1.4-3.6-3.5-3.6-1.7 0-2.8 1-2.8 2.3 0 1.1.8 1.9 1.9 1.9"
                    stroke="currentColor"
                    stroke-width="1.9"
                    stroke-linecap="round"
                  />
                </svg>
              </span>
              <span class="karta-pcard-name">{{ isZh ? p.nameZh : p.nameEn }}</span>
            </a>

            <span class="karta-auth-badge" :class="`auth-${p.auth}`">
              {{ authLabel(p.auth) }}
            </span>
          </div>

          <p class="karta-pcard-examples">
            {{ isZh ? p.examplesZh : p.examplesEn }}
          </p>

          <div class="karta-pcard-bottom">
            <button
              type="button"
              class="karta-slug-chip karta-focusable"
              :class="{ copied: copiedSlug === p.slug }"
              :title="isZh ? `复制 slug: ${p.slug}` : `Copy slug: ${p.slug}`"
              @click="copySlug(p.slug)"
            >
              <code class="karta-slug-code">{{ p.slug }}</code>
              <span class="karta-slug-status">
                <template v-if="copiedSlug === p.slug">
                  {{ isZh ? '已复制' : 'Copied!' }}
                </template>
                <svg v-else width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <rect x="5.2" y="5.2" width="8.3" height="8.3" rx="1.6" stroke="currentColor" stroke-width="1.4" />
                  <path
                    d="M10.8 5.2V3.8A1.3 1.3 0 0 0 9.5 2.5H3.8A1.3 1.3 0 0 0 2.5 3.8v5.7a1.3 1.3 0 0 0 1.3 1.3h1.4"
                    stroke="currentColor"
                    stroke-width="1.4"
                    stroke-linecap="round"
                  />
                </svg>
              </span>
            </button>

            <a
              :href="platformHref(p.slug)"
              class="karta-pcard-arrow karta-focusable"
              :aria-label="isZh ? `查看 ${p.nameZh}` : `View ${p.nameEn}`"
            >
              →
            </a>
          </div>
        </article>
      </div>
    </template>
  </section>
</template>

<style scoped>
.karta-platform-section {
  margin: 16px 0;
}

/* Shared Icon Tile (32×32 rounded badge) */
.karta-icon-tile {
  width: 32px;
  height: 32px;
  border-radius: 9px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: var(--p-accent);
  background: color-mix(in srgb, var(--p-accent) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--p-accent) 24%, transparent);
  transition: transform 0.2s ease, border-color 0.2s ease, background-color 0.2s ease;
}

.karta-icon-tile.neutral {
  color: var(--vp-c-text-1);
  background: var(--karta-surface-muted);
  border-color: var(--karta-border-strong);
}

/* Shared Auth Badges */
.karta-auth-badge {
  font-size: 10.5px;
  font-weight: 600;
  padding: 2px 7.5px;
  border-radius: 999px;
  white-space: nowrap;
  flex-shrink: 0;
}

.karta-auth-badge.auth-none {
  background: rgba(16, 185, 129, 0.14);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.24);
}

.karta-auth-badge.auth-optional {
  background: rgba(59, 130, 246, 0.12);
  color: #3b82f6;
}

.karta-auth-badge.auth-partial {
  background: rgba(168, 85, 247, 0.13);
  color: #a855f7;
}

.karta-auth-badge.auth-recommended {
  background: rgba(249, 115, 22, 0.14);
  color: #f97316;
}

.karta-auth-badge.auth-required {
  background: rgba(244, 63, 94, 0.14);
  color: #f43f5e;
}

/* ==========================================================================
 * COMPACT MODE: Dual-Row Infinite Horizontal Marquee (Landing Page)
 * ========================================================================== */

.karta-marquee-header {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: 14px;
  margin-bottom: 22px;
}

.karta-pgrid-kicker {
  display: inline-block;
  font-size: 11.5px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--vp-c-brand-1);
  margin-bottom: 6px;
}

.karta-pgrid-title {
  margin: 0 !important;
  padding: 0 !important;
  border: none !important;
  font-size: 24px !important;
  font-weight: 700;
  letter-spacing: -0.02em;
  line-height: 1.28;
  color: var(--vp-c-text-1);
}

.karta-explore-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 15px;
  border-radius: 999px;
  border: 1px solid var(--karta-border-strong);
  background: var(--karta-surface);
  color: var(--vp-c-text-1);
  font-size: 13px;
  font-weight: 600;
  text-decoration: none !important;
  box-shadow: var(--karta-shadow-sm);
  transition:
    border-color 0.2s ease,
    color 0.2s ease,
    transform 0.2s ease,
    box-shadow 0.2s ease;
}

.karta-explore-btn:hover {
  border-color: var(--karta-border-hover);
  color: var(--vp-c-brand-1);
  transform: translateY(-1px);
  box-shadow: var(--karta-shadow-md);
}

.karta-explore-arrow {
  transition: transform 0.2s ease;
}

.karta-explore-btn:hover .karta-explore-arrow {
  transform: translateX(2.5px);
}

.karta-marquee-stage {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 6px 0;
  overflow: hidden;
  -webkit-mask-image: linear-gradient(
    to right,
    transparent 0%,
    #000000 7%,
    #000000 93%,
    transparent 100%
  );
  mask-image: linear-gradient(
    to right,
    transparent 0%,
    #000000 7%,
    #000000 93%,
    transparent 100%
  );
}

.karta-marquee-row {
  display: flex;
  width: 100%;
  overflow: hidden;
}

.karta-marquee-track {
  display: flex;
  width: max-content;
  will-change: transform;
}

.karta-marquee-track.dir-left {
  animation: karta-marquee-left 40s linear infinite;
}

.karta-marquee-track.dir-right {
  animation: karta-marquee-right 44s linear infinite;
}

.karta-marquee-row:hover .karta-marquee-track,
.karta-marquee-row:focus-within .karta-marquee-track {
  animation-play-state: paused;
}

@keyframes karta-marquee-left {
  0% {
    transform: translateX(0);
  }
  100% {
    transform: translateX(-50%);
  }
}

@keyframes karta-marquee-right {
  0% {
    transform: translateX(-50%);
  }
  100% {
    transform: translateX(0);
  }
}

.karta-marquee-group {
  display: flex;
  align-items: center;
  gap: 12px;
  padding-right: 12px;
  flex-shrink: 0;
}

.karta-marquee-pill {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  padding: 8px 14px 8px 9px;
  border-radius: 14px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  color: var(--vp-c-text-1);
  text-decoration: none !important;
  box-shadow: var(--karta-shadow-sm);
  white-space: nowrap;
  transition:
    border-color 0.2s ease,
    transform 0.2s ease,
    box-shadow 0.2s ease;
}

.karta-marquee-pill:hover {
  border-color: var(--karta-border-hover);
  transform: translateY(-2px);
  box-shadow: var(--karta-shadow-md);
}

.karta-marquee-pill:hover .karta-icon-tile {
  transform: scale(1.05);
}

.karta-marquee-name {
  font-size: 13.5px;
  font-weight: 650;
  color: var(--vp-c-text-1);
  letter-spacing: -0.01em;
}

/* ==========================================================================
 * FULL MODE: Filterable 16-Card Grid (platforms.md & zh/platforms.md)
 * ========================================================================== */

.karta-pgrid-header {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: 14px;
  margin-bottom: 18px;
}

.karta-pgrid-sub {
  margin: 0;
  font-size: 13.5px;
  color: var(--vp-c-text-2);
}

.karta-filter-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
}

.karta-filter-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: 999px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  color: var(--vp-c-text-2);
  font-size: 12.5px;
  font-weight: 550;
  cursor: pointer;
  transition: all 0.2s ease;
}

.karta-filter-chip:hover {
  border-color: var(--karta-border-hover);
  color: var(--vp-c-text-1);
}

.karta-filter-chip.active {
  border-color: rgba(249, 115, 22, 0.5);
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}

.karta-filter-count {
  font-size: 11px;
  padding: 0 6px;
  border-radius: 999px;
  background: var(--karta-surface-muted);
  color: var(--vp-c-text-2);
}

.karta-filter-chip.active .karta-filter-count {
  background: rgba(249, 115, 22, 0.18);
  color: var(--vp-c-brand-1);
}

.karta-platform-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(238px, 1fr));
  gap: 12px;
}

.karta-platform-card {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 11px;
  padding: 14px;
  border-radius: 15px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  box-shadow: var(--karta-shadow-sm);
  transition:
    transform 0.2s ease,
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.karta-platform-card:hover {
  transform: translateY(-2px);
  border-color: var(--karta-border-hover);
  box-shadow: var(--karta-shadow-md);
}

.karta-pcard-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.karta-pcard-title {
  display: inline-flex;
  align-items: center;
  gap: 9px;
  min-width: 0;
  font-size: 14px;
  font-weight: 650;
  color: var(--vp-c-text-1);
  text-decoration: none !important;
  line-height: 1.3;
}

.karta-pcard-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.karta-pcard-title:hover {
  color: var(--vp-c-brand-1);
}

.karta-pcard-examples {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--vp-c-text-2);
}

.karta-pcard-bottom {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.karta-slug-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px 8px;
  border-radius: 7px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface-soft);
  color: var(--vp-c-text-2);
  cursor: pointer;
  transition: all 0.18s ease;
}

.karta-slug-chip:hover {
  border-color: var(--karta-border-hover);
  color: var(--vp-c-text-1);
}

.karta-slug-chip.copied {
  border-color: rgba(16, 185, 129, 0.45);
  background: rgba(16, 185, 129, 0.12);
  color: #10b981;
}

.karta-slug-code {
  font-family: var(--vp-font-family-mono);
  font-size: 11.5px;
  background: transparent !important;
  padding: 0 !important;
  color: inherit !important;
}

.karta-slug-status {
  display: inline-flex;
  align-items: center;
  font-size: 11px;
  font-weight: 600;
}

.karta-pcard-arrow {
  font-size: 13px;
  color: var(--vp-c-text-3);
  text-decoration: none !important;
  padding: 2px 5px;
  border-radius: 6px;
  transition: color 0.18s ease, transform 0.18s ease;
}

.karta-platform-card:hover .karta-pcard-arrow {
  color: var(--vp-c-brand-1);
  transform: translateX(2px);
}

/* ==========================================================================
 * Reduced Motion: Marquee falls back to clean wrapped flex pills
 * ========================================================================== */
@media (prefers-reduced-motion: reduce) {
  .karta-marquee-stage {
    -webkit-mask-image: none;
    mask-image: none;
    overflow: visible;
  }
  .karta-marquee-row {
    overflow: visible;
  }
  .karta-marquee-track {
    animation: none !important;
    width: 100%;
    flex-wrap: wrap;
  }
  .karta-marquee-group {
    flex-wrap: wrap;
    padding-right: 0;
  }
  .karta-marquee-group[aria-hidden='true'] {
    display: none;
  }
  .karta-platform-card,
  .karta-pcard-arrow,
  .karta-marquee-pill,
  .karta-icon-tile,
  .karta-explore-btn,
  .karta-explore-arrow {
    transition: none !important;
  }
  .karta-platform-card:hover,
  .karta-platform-card:hover .karta-pcard-arrow,
  .karta-marquee-pill:hover,
  .karta-marquee-pill:hover .karta-icon-tile,
  .karta-explore-btn:hover,
  .karta-explore-btn:hover .karta-explore-arrow {
    transform: none !important;
  }
}
</style>
