# HertaBase Admin UI

React、Vite 和 TanStack Router 实现的 HertaBase 管理后台。生产构建使用 `/webui/` 作为
资源和客户端路由基路径，并由 `herta_server` 通过 `rust-embed` 编入单一二进制。

## Development

从仓库根目录启动前端开发服务器：

```bash
pnpm dev:ui
```

浏览器访问 `http://localhost:5173/webui/`。Vite 会将 `/api/*` 和 `/_/*` 请求代理到
`http://127.0.0.1:8080`，因此需要另行启动开发 server。

## Build

```bash
pnpm build:ui
```

产物写入 `frontend/admin-ui/dist`。构建完整单一二进制应从仓库根目录运行 `pnpm build`
或 `just build`，以确保 UI 先于 Rust server 构建。
