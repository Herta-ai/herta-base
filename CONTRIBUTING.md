# 参与贡献 HertaBase

欢迎来到 HertaBase 项目！我们非常感谢您花时间为本项目做出贡献。本文档旨在帮助您了解参与贡献的流程和规范。

## 行为准则

参与本项目即表示您同意遵守我们的[行为准则](CODE_OF_CONDUCT.md)（暂未发布，请秉持友好、包容和专业的态度参与讨论）。

## 开发环境搭建

### 依赖要求

- **Rust**: 1.75 或更高版本
- **Node.js**: 20 或更高版本
- **pnpm**: 9 或更高版本

### 克隆与构建

```bash
git clone https://github.com/Herta-ai/hertabase.git
cd hertabase

# 安装前端依赖
pnpm install

# 构建后端核心模块
cargo build
```

### IDE 推荐

我们推荐使用 **Visual Studio Code** 进行开发，并安装以下插件：
- **rust-analyzer**: Rust 语言支持
- **ES7+ React/Redux/React-Native snippets**: React 开发支持
- **ESLint**: 代码检查
- **Prettier**: 代码格式化
- **Even Better TOML**: TOML 文件支持

## 项目结构概览

有关架构设计的详细信息，请参阅[项目架构文档](docs/project_arch.md)。以下是项目的核心目录结构：

```
hertabase/
├── crates/                  # Rust 后端微内核 (Cargo Workspace)
│   ├── herta_api/           # Salvo 路由、控制器、中间件
│   ├── herta_core/          # App 上下文、配置管理、全局错误处理
│   ├── herta_db/            # SurrealDB 客户端封装、Schema 动态转换
│   ├── herta_jsvm/          # rquickjs 运行时、沙盒、Rust↔JS FFI
│   ├── herta_storage/       # 文件存储抽象适配器 (FS/S3)
│   └── herta_server/        # CLI 入口、前端静态资源嵌入 (rust-embed)
│
├── frontend/                # 前端项目 (PNPM Workspace)
│   └── admin-ui/            # React/Vite 管理后台 SPA
│
├── packages/                # JS/TS 包 (PNPM Workspace)
│   ├── @hb/sdk/             # (可选) 面向最终用户的 JS SDK
│   └── @hb/types/           # Hook 编写者的 .d.ts 类型定义
│
├── examples/                # 示例与参考
│   └── hooks/               # Hook 脚本示例
│
└── docs/                    # 项目文档
```

## 开发工作流

根据您的开发重点，可以选择不同的启动方式：

- **仅后端开发**: 
  ```bash
  cargo watch -x 'run -p herta_server'
  ```
- **仅前端开发**: 
  ```bash
  pnpm --filter @hb/admin-ui dev
  ```
- **全栈开发**：在两个终端分别运行
  ```bash
  pnpm dev:server:mem
  pnpm dev:ui
  ```

## 代码风格与规范

### Rust
- 遵循 `rustfmt` 的默认格式化规则。
- 提交代码前，请确保运行 `cargo clippy` 且未产生任何警告：
  ```bash
  cargo clippy -- -D warnings
  ```

### TypeScript / React
- 使用项目现有格式配置进行代码格式化。
- 使用 **Oxlint** 进行代码质量检查。
- 请在提交前运行 `pnpm lint`。

### 提交信息规范 (Commit Messages)

本项目遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范。提交信息的格式应如下：
```
<type>[optional scope]: <description>
```
常用的 `<type>` 包含：
- `feat`: 新功能
- `fix`: 修复 Bug
- `docs`: 文档修改
- `refactor`: 重构（既非新增功能也非修改 bug 的代码变动）
- `test`: 增加或修改测试用例
- `chore`: 构建过程或辅助工具的变动

## 分支策略

- `main`: 稳定分支，随时准备发布。
- `dev`: 开发分支，用于集成各项新功能。
- `feature/*`: 功能开发分支，从 `dev` 切出，完成后合并回 `dev`。
- `fix/*`: 问题修复分支，从相应的目标分支切出，完成后合并回原分支。

## Pull Request 流程

1. 从 `feature/*` 或 `fix/*` 分支向 `dev` 分支发起 PR。
2. 确保所有的 CI 检查通过（包括 `cargo test`, `cargo clippy`, `pnpm lint` 等）。
3. 至少需要一位维护者的 Code Review 并同意。
4. 我们倾向于使用 **Squash and Merge** 将您的更改合并入主分支，以保持提交历史整洁。

## Issue 规范

### Bug 报告
请提供以下信息：
- 明确的复现步骤。
- 期望结果与实际结果的对比。
- 相关的环境信息（操作系统、Rust 版本、Node.js 版本等）。

### 功能请求
请说明：
- 该功能的应用场景或试图解决的问题。
- 建议的解决方案或初步思路。

## 测试规范

提交代码前，请确保现有测试均能通过，并为新功能添加适当的测试覆盖。

- **Rust 后端**:
  ```bash
  cargo test --workspace
  ```
- **前端模块**:
  ```bash
  pnpm --filter @hb/admin-ui test
  ```
- **集成测试**: (计划中，后续将补充相关规范)

## 发布流程概述

1. 确认 `dev` 分支的各项功能与测试均已完成。
2. 整理 Changelog 并提升版本号。
3. 将 `dev` 分支合并至 `main`。
4. 在 `main` 分支上打上对应版本号的 Tag。
5. GitHub Actions 将自动触发打包和发布流程。

## 联系与社区

如有任何疑问或需要帮助，欢迎通过 GitHub Issues 或 Discussions 与我们交流。期待您的参与！
