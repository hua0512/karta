import { defineConfig } from 'vitepress'

const SITE_URL = 'https://hua0512.github.io/karta/'
const REPO = 'https://github.com/hua0512/karta'

export default defineConfig({
  title: 'Karta',
  base: '/karta/',

  lastUpdated: true,
  cleanUrls: true,
  sitemap: { hostname: SITE_URL },

  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/karta/hero.svg' }],
    ['meta', { name: 'theme-color', content: '#f97316' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'Karta' }],
    ['meta', { property: 'og:url', content: SITE_URL }],
    [
      'meta',
      {
        property: 'og:description',
        content:
          'Multi-platform social-media link parser & native card renderer (16 platforms, QQ OneBot adapter, CLI).',
      },
    ],
  ],

  locales: {
    root: {
      label: 'English',
      lang: 'en-US',
      description:
        'A multi-platform social-media link parser and native card renderer — shipping with a Kovi QQ bot adapter and standalone CLI.',
      themeConfig: {
        nav: [
          {
            text: 'Guide',
            link: '/getting-started',
            activeMatch: '^/(getting-started|deployment)',
          },
          {
            text: 'Reference',
            activeMatch: '^/(configuration|platforms|commands)',
            items: [
              { text: 'Configuration', link: '/configuration' },
              { text: 'Platforms', link: '/platforms' },
              { text: 'Admin Commands', link: '/commands' },
            ],
          },
          {
            text: 'Development',
            link: '/development',
            activeMatch: '^/development',
          },
          { text: 'Releases', link: `${REPO}/releases` },
        ],
        sidebar: [
          {
            text: 'Guide',
            items: [
              { text: 'Getting Started', link: '/getting-started' },
              { text: 'Deployment', link: '/deployment' },
            ],
          },
          {
            text: 'Reference',
            items: [
              { text: 'Configuration', link: '/configuration' },
              { text: 'Platforms', link: '/platforms' },
              { text: 'Admin Commands', link: '/commands' },
            ],
          },
          {
            text: 'Development',
            items: [{ text: 'Contributing & Internals', link: '/development' }],
          },
        ],
        editLink: {
          pattern: `${REPO}/edit/main/docs/:path`,
          text: 'Edit this page on GitHub',
        },
        outline: { level: [2, 3] },
        footer: {
          message: 'Released under the MPL-2.0 License.',
          copyright: 'Copyright © 2026 hua0512',
        },
      },
    },

    zh: {
      label: '简体中文',
      lang: 'zh-CN',
      link: '/zh/',
      description:
        '多平台社交媒体链接解析与原生卡片渲染器，内置 Kovi QQ 机器人适配器与独立命令行工具。',
      themeConfig: {
        nav: [
          {
            text: '指南',
            link: '/zh/getting-started',
            activeMatch: '^/zh/(getting-started|deployment)',
          },
          {
            text: '参考',
            activeMatch: '^/zh/(configuration|platforms|commands)',
            items: [
              { text: '配置', link: '/zh/configuration' },
              { text: '平台', link: '/zh/platforms' },
              { text: '管理命令', link: '/zh/commands' },
            ],
          },
          {
            text: '开发',
            link: '/zh/development',
            activeMatch: '^/zh/development',
          },
          { text: '发布', link: `${REPO}/releases` },
        ],
        sidebar: [
          {
            text: '指南',
            items: [
              { text: '快速开始', link: '/zh/getting-started' },
              { text: '部署', link: '/zh/deployment' },
            ],
          },
          {
            text: '参考',
            items: [
              { text: '配置', link: '/zh/configuration' },
              { text: '平台', link: '/zh/platforms' },
              { text: '管理命令', link: '/zh/commands' },
            ],
          },
          {
            text: '开发',
            items: [{ text: '贡献与内部机制', link: '/zh/development' }],
          },
        ],
        editLink: {
          pattern: `${REPO}/edit/main/docs/:path`,
          text: '在 GitHub 上编辑此页',
        },
        outline: { level: [2, 3], label: '本页目录' },
        footer: {
          message: '基于 MPL-2.0 许可发布。',
          copyright: 'Copyright © 2026 hua0512',
        },
        docFooter: { prev: '上一页', next: '下一页' },
        lastUpdated: { text: '最后更新于' },
        darkModeSwitchLabel: '主题',
        sidebarMenuLabel: '菜单',
        returnToTopLabel: '回到顶部',
        langMenuLabel: '切换语言',
      },
    },
  },

  themeConfig: {
    logo: '/hero.svg',
    socialLinks: [{ icon: 'github', link: REPO }],

    search: {
      provider: 'local',
      options: {
        locales: {
          zh: {
            translations: {
              button: { buttonText: '搜索文档', buttonAriaLabel: '搜索文档' },
              modal: {
                noResultsText: '无法找到相关结果',
                resetButtonTitle: '清除查询条件',
                displayDetails: '显示详细列表',
                footer: {
                  selectText: '选择',
                  navigateText: '切换',
                  closeText: '关闭',
                },
              },
            },
          },
        },
      },
    },
  },
})
