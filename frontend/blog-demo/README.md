# HertaBlog (`blog-demo`)

基于 **Vue 3.5 + Vite 8 + TypeScript + UnoCSS** 构建的现代高颜值 WordPress 风格中文技术博客前端项目，无缝集成 **HertaBase** 官方 TypeScript SDK (`@hb/sdk`)。

---

## 🌟 核心特性

- 🎨 **WordPress 风格视觉美学**：
  - 杂志风 Hero 轮播头条与特色精选文章卡片
  - 沉浸式中文排版（`@unocss/preset-typography` + 中文字体栈优化）
  - 自动提取 H2/H3 大纲目录（TOC），随滚动动态高亮
  - 语法高亮代码块（Highlight.js GitHub 暗黑主题）并自带一键复制按钮
  - 人性化中文时间格式（“3 分钟前”、“昨天 18:20”）与阅读时长/字数估算
- 🗂️ **Taxonomy 专栏分类与标签云体系**：
  - 专栏分类统计与聚合流（架构设计、人工智能、Rust 实战、前端工程、思考随笔）
  - 权重自适应标签云
  - 年/月双级时间轴历史归档（Archives）
- 🔍 **全局极速搜索（Command Palette）**：
  - 快捷键 `Ctrl + K` / `Cmd + K` 快速唤起
  - 即时模糊检索标题、摘要、正文与标签
- 💬 **互动式评论系统**：
  - 读者讨论区，支持游客即时留言与已登录创作者标牌识别
  - 评论删除与管理能力
- ✍️ **Gutenberg 风格写作工作室**：
  - 双栏即时预览 Markdown 编辑器，配齐常用排版快捷工具栏
  - 文章属性侧边检查器（Inspector）：URL Slug、专栏选择、标签管理、随机高清封面、公开发布/私密草稿切换
  - 发布成功时触发五彩纸屑（Confetti）庆祝动画
- 📊 **文章管理工作台**：
  - WordPress 级后台管理表格，支持按全部/已发布/草稿箱筛选、快速编辑与上下架
- 🛠️ **前后端一体化与开箱即用**：
  - API `baseUrl` 默认为 `/`，专为 HertaBase Web 静态部署设计
  - 内置高质量中文技术演示文章，离线/未连接数据库时亦可完整体验
  - 一键 HertaBase 数据底座连接检测与数据初始化向导

---

## 🚀 快速开始

### 本地开发

从仓库根目录运行：

```bash
# 启动博客前端开发服务器 (默认端口 5174，自动代理 /api 到 8080)
pnpm dev:blog
```

或直接进入 `frontend/blog-demo` 目录：

```bash
cd frontend/blog-demo
pnpm dev
```

浏览器打开 `http://localhost:5174` 即可体验。

### 生产构建与部署到 HertaBase

```bash
# 构建生产包 (产物输出到 frontend/blog-demo/dist)
pnpm build:blog
```

构建生成的 `frontend/blog-demo/dist` 目录可打包为 `.zip` / `.tar.gz`，通过 HertaBase 的 Web 静态托管功能或 API 直接部署：

```bash
# 示例：通过 HertaBase 管理接口部署到 /web/blog-demo 或根别名
curl -X POST http://127.0.0.1:8080/_/web/deploy \
  -H "Authorization: Bearer <ADMIN_TOKEN>" \
  -F "archive=@dist.zip" \
  -F "alias=/web/blog" \
  -F "spaFallback=true"
```

---

## 📁 目录结构

```
frontend/blog-demo/
├── src/
│   ├── components/
│   │   ├── layout/       # 导航栏、页脚、Hero 轮播、侧边栏
│   │   ├── post/         # 文章卡片、列表流、TOC 目录、作者名片、推荐
│   │   ├── comments/     # 评论列表、表单、评论项
│   │   ├── editor/       # Markdown 写作器、Gutenberg 属性面板
│   │   └── ui/           # 全局搜索弹窗、初始化向导、Toast 消息、模态框
│   ├── lib/
│   │   ├── hb.ts         # @hb/sdk 客户端单例 (baseUrl: '/')
│   │   ├── markdown.ts   # Markdown 解析器与代码高亮
│   │   ├── utils.ts      # 中文阅读时长、日期格式化与字数统计
│   │   └── seed-data.ts  # 精选中文技术演示文章与分类
│   ├── stores/           # Pinia 状态库 (auth, blog, theme)
│   ├── types/            # 完整 TypeScript 类型定义
│   └── views/            # 首页、阅读页、分类页、标签页、归档页、写作页、后台等
├── uno.config.ts         # UnoCSS 原子化配置
├── vite.config.ts        # Vite 8 配置与代理规则
└── package.json
```
