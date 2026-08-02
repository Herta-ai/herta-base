# 🚀 vue3-ess-starter

一个现代化的 Vue 3 核心启动模板（Starter Template），基于最新的 Vite 8 和 Vue 3.6 构建。
本项目集成了当前前端生态中最优秀的工具链与依赖，旨在为中后台系统或企业级 Web 应用提供一个开箱即用、高规范、强类型的前端工程化底座。

## ✨ 技术栈亮点

### ⚡️ 核心框架

- **[Vue](https://vuejs.org/)** - 渐进式 JavaScript 框架
- **[Vite](https://vitejs.dev/)** - 极速的前端构建工具
- **[TypeScript](https://www.typescriptlang.org/)** - 静态类型检查

### 🗺 路由与状态管理

- **[Vue Router](https://router.vuejs.org/)** - Vue 官方路由 (可以按需安装 `unplugin-vue-router` 插件)
- **[Pinia](https://pinia.vuejs.org/)** - 新一代 Vue 状态管理
- **[pinia-plugin-persistedstate](https://prazdevs.github.io/pinia-plugin-persistedstate/)** - Pinia 状态持久化插件

### 🎨 UI 与样式设计

- **[Naive UI](https://www.naiveui.com/)** - 尤雨溪推荐的 Vue 3 组件库
- **[UnoCSS](https://unocss.dev/)** - 高性能的即时原子化 CSS 引擎
- **SCSS** (`sass-embedded`) - 更快速的 Sass 编译支持
- **图标库** - 基于 `unplugin-icons` 和 `@iconify/utils` 实现纯静态按需加载的图标组件

### 📡 请求与通信

- **[Alova](https://alova.js.org/)** - 轻量级的下一代请求策略库，提供缓存、防抖、自动刷新等高级特性

### 🛠 实用工具库

- **[@vueuse/core](https://vueuse.org/)** - 强大的 Vue 组合式 API 工具集合
- **[Vue I18n](https://vue-i18n.intlify.dev/)** - 国际化支持
- **[Day.js](https://day.js.org/)** - 轻量级日期时间处理库
- **[Defu](https://github.com/unjs/defu)** - 配置深度合并工具
- **[Colord](https://colord.omgovich.com/)** - 极致轻量的高性能颜色操作与转换库
- **[Crypto-js](https://github.com/brix/crypto-js)** - 常用加密算法工具（MD5, AES 等）
- **NProgress** - 顶部加载进度条

### ⚙️ 工程化与代码规范

- **ESLint** - 采用 [@antfu/eslint-config](https://github.com/antfu/eslint-config) (Flat Config) 实现统一的代码风格格式化
- **Git Hooks** - 基于 `simple-git-hooks` 实现轻量级 Git 钩子
- **Commitlint** - 遵循 Conventional Commits（约定式提交）规范
- **自动导入** - 配合 `unplugin-vue-components` 实现组件和 API 的自动按需引入

---

## 📦 快速开始

### 1. 环境准备

- 推荐 Bun 版本 >= **1.3**
- 本项目默认推荐使用 [bun](https://bun.sh/) 作为包管理器 (在 Git Hooks 中已固定使用 bun)。

### 2. 安装依赖

```bash
# 推荐使用 bun 安装
bun install
```

### 3. 初始化 Git Hooks

项目使用了 `simple-git-hooks`，在安装依赖后通常会自动触发 `prepare` 脚本。如果没有，请手动执行：

```bash
bun prepare
```

### 4. 启动开发服务器

```bash
bun dev
```

---

## 📜 脚本命令说明

| 命令            | 描述                                        |
| --------------- | ------------------------------------------- |
| `bun dev`       | 启动 Vite 开发服务器                        |
| `bun build`     | 生产环境构建（包含 `vue-tsc` 严格类型检查） |
| `bun preview`   | 在本地预览生产环境的构建产物                |
| `bun typecheck` | 仅执行 TypeScript 类型检查，不生成编译文件  |
| `bun lint`      | 运行 ESLint 检查并自动修复代码格式问题      |
| `bun prepare`   | 安装 Git 钩子配置（通常无需手动运行）       |

---

## 🏗 Git 提交规范

本项目已配置严格的 Git 工作流：

1. **Pre-commit**: 提交代码前会自动触发 `bun typecheck` 和 `bun lint`，并检查是否有未修复的变更（`git diff --exit-code`）。只有类型与格式检查全通过，才能成功 commit。
2. **Commit-msg**: 采用 Angular 规范（Conventional Commits），提交信息必须符合如下格式：

```text
<type>(<scope>): <subject>
```

**常用的 Type 类型：**

- `feat`: 新增功能
- `fix`: 修复 Bug
- `docs`: 文档变更
- `style`: 代码格式（不影响代码运行的变动，空格、格式化等）
- `refactor`: 重构（既不是新增功能，也不是修复 bug 的代码变动）
- `perf`: 性能优化
- `test`: 增加测试
- `chore`: 构建过程或辅助工具的变动

**示例：** `feat: 增加用户登录模块`

---

## 🤝 贡献指南

1. Fork 本仓库
2. 创建你的特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交你的更改 (`git commit -m 'feat: add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 发起一个 Pull Request
