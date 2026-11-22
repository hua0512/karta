<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { useData } from 'vitepress'

type RenderType = 'image' | 'text'
type ImageHeaderStyle = 'blur_background' | 'mesh_glow' | 'blur' | 'soft_prism' | 'auto'
type ImageThemeMode = 'auto' | 'light' | 'dark'

const { lang } = useData()
const isZh = computed(() => lang.value.startsWith('zh'))

const renderType = ref<RenderType>('image')
const imageHeaderStyle = ref<ImageHeaderStyle>('blur_background')
const imageThemeMode = ref<ImageThemeMode>('auto')
const forwardContents = ref(true)
const reactionEnabled = ref(true)
const copied = ref(false)
let copyTimer: ReturnType<typeof setTimeout> | null = null

const headerStyleOptions: { value: ImageHeaderStyle; labelZh: string; labelEn: string }[] = [
  { value: 'blur_background', labelZh: 'blur_background（默认磨砂）', labelEn: 'blur_background (default)' },
  { value: 'mesh_glow', labelZh: 'mesh_glow（弥散氛围光）', labelEn: 'mesh_glow' },
  { value: 'blur', labelZh: 'blur（顶部模糊）', labelEn: 'blur' },
  { value: 'soft_prism', labelZh: 'soft_prism（玻璃棱镜）', labelEn: 'soft_prism' },
  { value: 'auto', labelZh: 'auto（自动）', labelEn: 'auto' },
]

const themeModeOptions: { value: ImageThemeMode; labelZh: string; labelEn: string }[] = [
  { value: 'auto', labelZh: 'auto（按封面明暗自动）', labelEn: 'auto (by media luma)' },
  { value: 'light', labelZh: 'light（浅色暖纸）', labelEn: 'light (warm paper)' },
  { value: 'dark', labelZh: 'dark（暗色磨砂）', labelEn: 'dark (obsidian glass)' },
]

// When style is "auto", Karta picks stably among mesh_glow / blur / soft_prism; preview "blur"
const effectiveStyle = computed(() =>
  imageHeaderStyle.value === 'auto' ? 'blur' : imageHeaderStyle.value
)

const isDarkPreview = computed(() => imageThemeMode.value === 'dark')

const generatedJson = computed(() =>
  JSON.stringify(
    {
      render_type: renderType.value,
      image_header_style: imageHeaderStyle.value,
      image_theme_mode: imageThemeMode.value,
      forward_contents: forwardContents.value,
      reaction_enabled: reactionEnabled.value,
    },
    null,
    2
  )
)

function resetDefaults() {
  renderType.value = 'image'
  imageHeaderStyle.value = 'blur_background'
  imageThemeMode.value = 'auto'
  forwardContents.value = true
  reactionEnabled.value = true
}

async function copyConfig() {
  if (typeof window === 'undefined') return
  try {
    if (navigator?.clipboard?.writeText) {
      await navigator.clipboard.writeText(generatedJson.value)
    } else {
      const textarea = document.createElement('textarea')
      textarea.value = generatedJson.value
      textarea.style.position = 'fixed'
      textarea.style.opacity = '0'
      document.body.appendChild(textarea)
      textarea.select()
      document.execCommand('copy')
      document.body.removeChild(textarea)
    }
    copied.value = true
    if (copyTimer) clearTimeout(copyTimer)
    copyTimer = setTimeout(() => {
      copied.value = false
    }, 1800)
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
    class="karta-pg-shell"
    :aria-label="isZh ? 'Karta 配置交互演练场' : 'Karta interactive configuration playground'"
  >
    <div class="karta-pg-header">
      <div>
        <span class="karta-pg-badge">
          {{ isZh ? '交互式配置生成器' : 'INTERACTIVE CONFIG BUILDER' }}
        </span>
        <h3 class="karta-pg-title">
          {{ isZh ? '实时预览渲染风格与输出行为' : 'Preview rendering styles & output behavior live' }}
        </h3>
      </div>
      <button type="button" class="karta-pg-reset karta-focusable" @click="resetDefaults">
        {{ isZh ? '恢复推荐配置' : 'Reset' }}
      </button>
    </div>

    <div class="karta-pg-grid">
      <!-- Controls Column -->
      <div class="karta-pg-controls">
        <!-- render_type -->
        <div class="karta-ctrl-block">
          <div class="karta-ctrl-label-row">
            <code class="karta-ctrl-key">render_type</code>
            <span class="karta-ctrl-desc">
              {{ isZh ? '卡片渲染方式' : 'Response style' }}
            </span>
          </div>
          <div class="karta-choice-row" role="radiogroup" aria-label="render_type">
            <button
              type="button"
              role="radio"
              class="karta-choice-btn karta-focusable"
              :class="{ active: renderType === 'image' }"
              :aria-checked="renderType === 'image'"
              @click="renderType = 'image'"
            >
              <span>"image"</span>
              <small>{{ isZh ? '原生图片卡片' : 'Native image card' }}</small>
            </button>
            <button
              type="button"
              role="radio"
              class="karta-choice-btn karta-focusable"
              :class="{ active: renderType === 'text' }"
              :aria-checked="renderType === 'text'"
              @click="renderType = 'text'"
            >
              <span>"text"</span>
              <small>{{ isZh ? '简洁纯文本' : 'Structured text' }}</small>
            </button>
          </div>
        </div>

        <!-- image_header_style -->
        <div class="karta-ctrl-block" :class="{ dimmed: renderType === 'text' }">
          <div class="karta-ctrl-label-row">
            <code class="karta-ctrl-key">image_header_style</code>
            <span class="karta-ctrl-desc">
              {{
                renderType === 'text'
                  ? isZh
                    ? '仅 image 模式生效'
                    : 'Applies in image mode'
                  : isZh
                    ? '原生卡片背景与页眉风格'
                    : 'Card backdrop & header style'
              }}
            </span>
          </div>
          <div class="karta-style-grid" role="radiogroup" aria-label="image_header_style">
            <button
              v-for="opt in headerStyleOptions"
              :key="opt.value"
              type="button"
              role="radio"
              class="karta-style-opt karta-focusable"
              :class="{ active: imageHeaderStyle === opt.value }"
              :aria-checked="imageHeaderStyle === opt.value"
              @click="imageHeaderStyle = opt.value"
            >
              {{ isZh ? opt.labelZh : opt.labelEn }}
            </button>
          </div>
        </div>

        <!-- image_theme_mode -->
        <div class="karta-ctrl-block" :class="{ dimmed: renderType === 'text' }">
          <div class="karta-ctrl-label-row">
            <code class="karta-ctrl-key">image_theme_mode</code>
            <span class="karta-ctrl-desc">
              {{
                renderType === 'text'
                  ? isZh
                    ? '仅 image 模式生效'
                    : 'Applies in image mode'
                  : isZh
                    ? '明暗极性（暗调封面自动切暗色磨砂）'
                    : 'Color polarity (auto dark glass for moody covers)'
              }}
            </span>
          </div>
          <div class="karta-style-grid" role="radiogroup" aria-label="image_theme_mode">
            <button
              v-for="opt in themeModeOptions"
              :key="opt.value"
              type="button"
              role="radio"
              class="karta-style-opt karta-focusable"
              :class="{ active: imageThemeMode === opt.value }"
              :aria-checked="imageThemeMode === opt.value"
              @click="imageThemeMode = opt.value"
            >
              {{ isZh ? opt.labelZh : opt.labelEn }}
            </button>
          </div>
        </div>

        <!-- Boolean Toggles -->
        <div class="karta-toggle-list">
          <label class="karta-toggle-row">
            <div class="karta-toggle-copy">
              <code class="karta-ctrl-key">forward_contents</code>
              <span class="karta-ctrl-desc">
                {{
                  isZh
                    ? '多图图集自动打包为 QQ 合并转发消息'
                    : 'Bundle multi-image galleries into a merged-forward message'
                }}
              </span>
            </div>
            <button
              type="button"
              role="switch"
              class="karta-switch karta-focusable"
              :class="{ on: forwardContents }"
              :aria-checked="forwardContents"
              @click="forwardContents = !forwardContents"
            >
              <span class="karta-switch-thumb" />
            </button>
          </label>

          <label class="karta-toggle-row">
            <div class="karta-toggle-copy">
              <code class="karta-ctrl-key">reaction_enabled</code>
              <span class="karta-ctrl-desc">
                {{
                  isZh
                    ? '解析中贴 👀 表情回应，完成后自动切换 ✨ / ❌'
                    : 'React with 👀 while parsing, then ✨ or ❌ on completion'
                }}
              </span>
            </div>
            <button
              type="button"
              role="switch"
              class="karta-switch karta-focusable"
              :class="{ on: reactionEnabled }"
              :aria-checked="reactionEnabled"
              @click="reactionEnabled = !reactionEnabled"
            >
              <span class="karta-switch-thumb" />
            </button>
          </label>
        </div>
      </div>

      <!-- Live Visual Preview + Generated JSON Column -->
      <div class="karta-pg-preview-col">
        <!-- Mini Visual Preview -->
        <div class="karta-mini-stage">
          <div class="karta-mini-user-msg">
            <span class="karta-mini-link">https://xhslink.com/a/9kP2xM</span>
            <span v-if="reactionEnabled" class="karta-mini-reaction">
              👀 → ✨
            </span>
          </div>

          <!-- Native Mini Card -->
          <div
            v-if="renderType === 'image'"
            class="karta-mini-frame"
            :class="[`style-${effectiveStyle}`, { 'is-dark': isDarkPreview }]"
          >
            <div class="karta-mini-paper" :class="[`style-${effectiveStyle}`, { 'is-dark': isDarkPreview }]">
              <div v-if="effectiveStyle === 'mesh_glow'" class="karta-mini-mesh" aria-hidden="true" />
              <div v-else-if="effectiveStyle === 'blur'" class="karta-mini-blur" aria-hidden="true" />
              <svg
                v-else-if="effectiveStyle === 'soft_prism'"
                class="karta-mini-prism"
                viewBox="0 0 260 66"
                preserveAspectRatio="none"
                aria-hidden="true"
              >
                <ellipse cx="240" cy="-28" rx="202" ry="86" fill="#fbbf24" fill-opacity="0.22" stroke="#ffffff" stroke-opacity="0.45" stroke-width="1.2" vector-effect="non-scaling-stroke" />
                <ellipse cx="265" cy="-20" rx="146" ry="74" fill="#ff3b5c" fill-opacity="0.24" stroke="#ffffff" stroke-opacity="0.55" stroke-width="1.2" vector-effect="non-scaling-stroke" />
              </svg>

              <div class="karta-mini-paper-inner">
                <div class="karta-mini-card-head">
                  <span class="karta-mini-ava" />
                  <div class="karta-mini-author-lines">
                    <span class="karta-mini-line strong" />
                    <span class="karta-mini-line short" />
                  </div>
                  <span class="karta-mini-mark">小红书</span>
                </div>
                <div class="karta-mini-title">
                  {{ isZh ? '京都鸭川边 6 家暖木风手冲咖啡馆' : '6 warm-wood pour-over cafes in Kyoto' }}
                </div>
                <div class="karta-mini-thumbs" aria-hidden="true">
                  <span />
                  <span />
                  <span />
                </div>
              </div>
            </div>
          </div>

          <!-- Plain Text Mini Reply -->
          <div v-else class="karta-mini-text-card">
            <div>[xiaohongshu] @胶片漫游日记 | 京都鸭川边 6 家暖木风手冲咖啡馆</div>
            <div class="karta-mini-text-extra">点赞: 2.4万 | 收藏: 1.9万 | 评论: 648</div>
          </div>

          <!-- Gallery Delivery Indicator -->
          <div class="karta-mini-delivery">
            <span class="karta-delivery-dot" :class="{ bundled: forwardContents }" />
            <span v-if="forwardContents">
              {{
                isZh
                  ? '图集交付：1 条合并转发消息（内含 6 张图片）'
                  : 'Gallery delivery: 1 merged-forward bundle (6 images)'
              }}
            </span>
            <span v-else>
              {{
                isZh
                  ? '图集交付：逐条发送 6 条独立图片消息'
                  : 'Gallery delivery: 6 separate image messages'
              }}
            </span>
          </div>
        </div>

        <!-- Generated data/karta/config.json Snippet -->
        <div class="karta-json-box">
          <div class="karta-json-bar">
            <span class="karta-json-path">data/karta/config.json</span>
            <button
              type="button"
              class="karta-copy-btn karta-focusable"
              :class="{ copied }"
              @click="copyConfig"
            >
              <template v-if="copied">
                {{ isZh ? '已复制' : 'Copied!' }}
              </template>
              <template v-else>
                {{ isZh ? '复制 JSON' : 'Copy JSON' }}
              </template>
            </button>
          </div>
          <pre class="karta-json-pre"><code>{{ generatedJson }}</code></pre>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.karta-pg-shell {
  margin: 24px 0;
  border-radius: 18px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  box-shadow: var(--karta-shadow-md);
  overflow: hidden;
}

.karta-pg-header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 20px;
  border-bottom: 1px solid var(--vp-c-divider);
  background: var(--karta-surface-soft);
}

.karta-pg-badge {
  display: inline-block;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
  color: var(--vp-c-brand-1);
  margin-bottom: 3px;
}

.karta-pg-title {
  margin: 0 !important;
  font-size: 16.5px !important;
  font-weight: 650;
  color: var(--vp-c-text-1);
}

.karta-pg-reset {
  padding: 5px 11px;
  border-radius: 8px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  color: var(--vp-c-text-2);
  font-size: 12px;
  font-weight: 550;
  cursor: pointer;
  transition: all 0.18s ease;
}

.karta-pg-reset:hover {
  border-color: var(--karta-border-hover);
  color: var(--vp-c-text-1);
}

.karta-pg-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.05fr) minmax(0, 0.95fr);
}

@media (max-width: 820px) {
  .karta-pg-grid {
    grid-template-columns: 1fr;
  }
}

/* Controls */
.karta-pg-controls {
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 18px;
  border-right: 1px solid var(--vp-c-divider);
}

@media (max-width: 820px) {
  .karta-pg-controls {
    border-right: none;
    border-bottom: 1px solid var(--vp-c-divider);
  }
}

.karta-ctrl-block {
  transition: opacity 0.2s ease;
}

.karta-ctrl-block.dimmed {
  opacity: 0.52;
}

.karta-ctrl-label-row {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 8px;
}

.karta-ctrl-key {
  font-family: var(--vp-font-family-mono);
  font-size: 12.5px;
  font-weight: 600;
  color: var(--vp-c-text-1);
  background: var(--karta-surface-soft);
  border: 1px solid var(--karta-border);
  padding: 1px 7px;
  border-radius: 6px;
}

.karta-ctrl-desc {
  font-size: 12.5px;
  color: var(--vp-c-text-2);
}

.karta-choice-row {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.karta-choice-btn {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 10px 12px;
  border-radius: 11px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface-soft);
  color: var(--vp-c-text-1);
  font-family: var(--vp-font-family-mono);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.18s ease;
}

.karta-choice-btn small {
  font-family: var(--vp-font-family-base);
  font-size: 11.5px;
  font-weight: 450;
  color: var(--vp-c-text-2);
}

.karta-choice-btn:hover {
  border-color: var(--karta-border-hover);
}

.karta-choice-btn.active {
  border-color: rgba(249, 115, 22, 0.55);
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}

.karta-style-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
}

.karta-style-opt {
  padding: 6px 10px;
  border-radius: 8px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface-soft);
  color: var(--vp-c-text-2);
  font-size: 12px;
  font-weight: 550;
  cursor: pointer;
  transition: all 0.18s ease;
}

.karta-style-opt:hover {
  border-color: var(--karta-border-hover);
  color: var(--vp-c-text-1);
}

.karta-style-opt.active {
  border-color: rgba(249, 115, 22, 0.52);
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}

/* Switches */
.karta-toggle-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.karta-toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 12px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface-soft);
}

.karta-toggle-copy {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
}

.karta-switch {
  position: relative;
  width: 42px;
  height: 24px;
  border-radius: 999px;
  border: 1px solid var(--karta-border-strong);
  background: var(--karta-surface-muted);
  cursor: pointer;
  flex-shrink: 0;
  transition: background-color 0.2s ease, border-color 0.2s ease;
}

.karta-switch.on {
  background: linear-gradient(120deg, #ff5e3a, #f97316);
  border-color: transparent;
}

.karta-switch-thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 18px;
  height: 18px;
  border-radius: 999px;
  background: #ffffff;
  box-shadow: 0 2px 5px rgba(0, 0, 0, 0.25);
  transition: transform 0.2s ease;
}

.karta-switch.on .karta-switch-thumb {
  transform: translateX(18px);
}

/* Right Column: Preview + JSON */
.karta-pg-preview-col {
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  background: var(--karta-surface-soft);
}

.karta-mini-stage {
  border-radius: 14px;
  border: 1px solid var(--karta-border);
  background: var(--karta-surface);
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.karta-mini-user-msg {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 7px 10px;
  border-radius: 10px;
  background: var(--karta-surface-soft);
  border: 1px solid var(--karta-border);
}

.karta-mini-link {
  font-family: var(--vp-font-family-mono);
  font-size: 11.5px;
  color: var(--vp-c-brand-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.karta-mini-reaction {
  font-size: 11px;
  padding: 2px 7px;
  border-radius: 999px;
  background: rgba(16, 185, 129, 0.12);
  color: #10b981;
  white-space: nowrap;
}

/* Mini Card Frame */
.karta-mini-frame {
  padding: 9px 9px 11px;
  border-radius: 14px;
  background: linear-gradient(145deg, #fbd8dc 0%, #fce6d8 55%, #f9dfd2 100%);
  position: relative;
  overflow: hidden;
}

.karta-mini-frame.style-blur_background::before {
  content: '';
  position: absolute;
  inset: -12px;
  background:
    radial-gradient(circle at 20% 20%, #ff3b5c, transparent 58%),
    radial-gradient(circle at 85% 80%, #fbbf24, transparent 58%);
  opacity: 0.35;
  filter: blur(16px);
}

.karta-mini-paper {
  position: relative;
  border-radius: 11px;
  background: #fefaf5;
  color: #191c20;
  overflow: hidden;
  box-shadow: 0 8px 18px rgba(28, 22, 20, 0.14);
}

.karta-mini-paper.style-blur_background {
  background: rgba(254, 250, 245, 0.84);
  backdrop-filter: blur(12px);
}

.karta-mini-frame.is-dark {
  background: linear-gradient(145deg, #1b1724 0%, #151722 55%, #1d1620 100%);
}

.karta-mini-paper.is-dark {
  background: #171923;
  color: #f4f1ee;
  border: 1px solid rgba(255, 255, 255, 0.14);
  box-shadow: 0 10px 22px rgba(4, 5, 10, 0.45);
}

.karta-mini-paper.is-dark.style-blur_background {
  background: rgba(22, 24, 33, 0.84);
}

.karta-mini-paper.is-dark .karta-mini-title {
  color: #fcfaf8;
}

.karta-mini-paper.is-dark .karta-mini-line {
  background: rgba(244, 241, 238, 0.22);
}

.karta-mini-paper.is-dark .karta-mini-line.strong {
  background: rgba(244, 241, 238, 0.52);
}

.karta-mini-mesh {
  position: absolute;
  inset: 0 0 auto 0;
  height: 64px;
  background:
    radial-gradient(ellipse at 80% 8%, rgba(255, 59, 92, 0.34) 0%, transparent 62%),
    radial-gradient(ellipse at 36% -4%, rgba(251, 191, 36, 0.30) 0%, transparent 60%);
  filter: blur(8px);
}

.karta-mini-blur {
  position: absolute;
  inset: 0 0 auto 0;
  height: 54px;
  background: linear-gradient(180deg, rgba(255, 59, 92, 0.22), transparent);
}

.karta-mini-prism {
  position: absolute;
  inset: 0 0 auto 0;
  width: 100%;
  height: 60px;
  -webkit-mask-image: linear-gradient(180deg, #000 0%, rgba(0, 0, 0, 0.55) 60%, transparent 100%);
  mask-image: linear-gradient(180deg, #000 0%, rgba(0, 0, 0, 0.55) 60%, transparent 100%);
}

.karta-mini-paper.is-dark .karta-mini-mesh,
.karta-mini-paper.is-dark .karta-mini-prism {
  opacity: 0.72;
}

.karta-mini-paper-inner {
  position: relative;
  z-index: 1;
  padding: 11px 12px;
}

.karta-mini-card-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.karta-mini-ava {
  width: 22px;
  height: 22px;
  border-radius: 999px;
  background: linear-gradient(135deg, #ff2442, #fb7185);
  flex-shrink: 0;
}

.karta-mini-author-lines {
  display: flex;
  flex-direction: column;
  gap: 3px;
  flex: 1;
}

.karta-mini-line {
  height: 6px;
  border-radius: 4px;
  background: rgba(29, 32, 37, 0.16);
}

.karta-mini-line.strong {
  width: 68px;
  background: rgba(29, 32, 37, 0.35);
}

.karta-mini-line.short {
  width: 46px;
}

.karta-mini-mark {
  font-size: 11px;
  font-weight: 800;
  color: #ff2442;
}

.karta-mini-title {
  font-size: 12.5px;
  font-weight: 700;
  color: #191c20;
  margin-bottom: 8px;
  line-height: 1.35;
}

.karta-mini-thumbs {
  display: grid;
  grid-template-columns: 1.45fr 1fr;
  grid-template-rows: 22px 22px;
  gap: 4px;
}

.karta-mini-thumbs span {
  border-radius: 6px;
  background: linear-gradient(135deg, #fb7185, #fbbf24);
}

.karta-mini-thumbs span:first-child {
  grid-row: 1 / 3;
}

.karta-mini-thumbs span:nth-child(2) {
  background: linear-gradient(135deg, #f97316, #fcd34d);
}

.karta-mini-thumbs span:nth-child(3) {
  background: linear-gradient(135deg, #e11d48, #fb923c);
}

.karta-mini-text-card {
  padding: 11px 12px;
  border-radius: 10px;
  background: var(--karta-code-bg);
  border: 1px solid var(--karta-code-border);
  color: #f4f4f5;
  font-family: var(--vp-font-family-mono);
  font-size: 11.5px;
  line-height: 1.55;
}

.karta-mini-text-extra {
  color: #a1a1aa;
  margin-top: 4px;
}

.karta-mini-delivery {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--vp-c-text-2);
}

.karta-delivery-dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: #94a3b8;
  flex-shrink: 0;
}

.karta-delivery-dot.bundled {
  background: #f97316;
}

/* JSON Snippet Box */
.karta-json-box {
  border-radius: 14px;
  border: 1px solid var(--karta-code-border);
  background: var(--karta-code-bg);
  overflow: hidden;
}

.karta-json-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.karta-json-path {
  font-family: var(--vp-font-family-mono);
  font-size: 11.5px;
  color: #a1a1aa;
}

.karta-copy-btn {
  padding: 3px 10px;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  background: rgba(255, 255, 255, 0.06);
  color: #f4f4f5;
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.18s ease;
}

.karta-copy-btn:hover {
  border-color: rgba(251, 146, 60, 0.5);
  color: #fb923c;
}

.karta-copy-btn.copied {
  border-color: rgba(16, 185, 129, 0.5);
  color: #34d399;
}

.karta-json-pre {
  margin: 0 !important;
  padding: 13px 14px !important;
  background: transparent !important;
  color: #fde68a;
  font-family: var(--vp-font-family-mono);
  font-size: 12.5px;
  line-height: 1.6;
  overflow-x: auto;
}

.karta-json-pre code {
  background: transparent !important;
  color: inherit !important;
  padding: 0 !important;
}

@media (prefers-reduced-motion: reduce) {
  .karta-switch,
  .karta-switch-thumb,
  .karta-choice-btn,
  .karta-style-opt {
    transition: none !important;
  }
}
</style>
