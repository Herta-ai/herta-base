# 部署指南

本文档介绍 HertaBase 的部署与运维操作，涵盖从单文件执行到集群部署的多种方案。

## 1. 部署方案概览

HertaBase 设计为零依赖的单体可执行文件（在单体架构模式下）。我们提供以下主要部署方式：

- **单二进制文件部署**：最简单直接，适合大多数中小规模应用。
- **Docker 部署**：适合容器化环境。
- **系统服务（Systemd）**：适合裸金属或虚拟机上的后台驻留服务。

## 2. 单二进制文件部署

### 下载版本

可以从 GitHub Releases 页面下载针对不同操作系统的预编译 `hertabase` 二进制文件。
[GitHub 仓库: github.com/Herta-ai/hertabase]

### 基础启动

直接运行 `hertabase serve` 即可在默认端口启动服务。

### 配置管理

系统支持通过环境变量或 `hertabase.toml` 配置文件进行配置：

- 环境变量如 `HB_PORT=8080`
- 在相同目录下存放 `hertabase.toml` 以持久化配置。

从源码构建单文件发行版时，使用根目录 `pnpm build` 或 `just build`。这两个命令会先生成
`frontend/admin-ui/dist`，再编译包含管理后台资源的 release 二进制。若直接运行
`cargo build -p herta_server`，必须先执行 `pnpm build:ui`。

服务启动后，内置管理后台位于 `/webui/`；`/webui` 会以 `308` 重定向到规范路径。

## 3. Docker 部署

### 官方 Docker 镜像

官方镜像通过 GitHub Container Registry (GHCR) 进行发布：

```bash
docker pull ghcr.io/herta-ai/hertabase:latest
```

### Dockerfile 示例（多阶段构建）

```dockerfile
FROM node:24-bookworm-slim AS ui-builder
WORKDIR /usr/src/hertabase
RUN corepack enable
COPY . .
RUN pnpm install --frozen-lockfile && pnpm build:ui

FROM rust:1.97 AS builder
WORKDIR /usr/src/hertabase
COPY . .
COPY --from=ui-builder /usr/src/hertabase/frontend/admin-ui/dist ./frontend/admin-ui/dist
RUN cargo build --release --bin hertabase

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /usr/src/hertabase/target/release/hertabase /app/
EXPOSE 8080
CMD ["./hertabase", "serve"]
```

### docker-compose.yml 示例

```yaml
version: '3.8'
services:
  hertabase:
    image: ghcr.io/herta-ai/hertabase:latest
    ports:
      - "8080:8080"
    volumes:
      - ./hb_data:/app/hb_data
      - ./hb_hooks:/app/hb_hooks
    environment:
      - HB_PORT=8080
      - HB_DATA_DIR=/app/hb_data
      - HB_HOOKS_DIR=/app/hb_hooks
    restart: unless-stopped
```

## 4. Systemd 服务部署

### Unit 文件示例

创建 `/etc/systemd/system/hertabase.service`：

```ini
[Unit]
Description=HertaBase Service
After=network.target

[Service]
User=hertabase
Group=hertabase
ExecStart=/opt/hertabase/hertabase serve --dir /opt/hertabase/hb_data
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### 日志管理

配置后，可使用 `journalctl -u hertabase.service -f` 跟踪管理日志。

## 5. 网页项目部署

HertaBase 计划支持管理员通过 Admin API 上传 ZIP、`tar.gz` 或 7z 前端构建产物，并将每个
顶层目录作为一个独立网页项目托管。项目默认解压到 `./hb_data/web/{project}`，通过
`/web/{project}/` 访问，并默认启用适合 History API SPA 的 `try_files` fallback。
重复上传同名项目会先备份当前版本，再部署新版本。

压缩包结构、更新与回滚、路由别名、安全解压和静态文件行为见
[网页部署与静态托管](web-deployment.md)。

## 6. 反向代理

### Nginx 配置示例（支持 WebSocket/SSE）

```nginx
server {
    listen 80;
    server_name api.example.com;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        
        # SSE & WebSocket 支持
        proxy_http_version 1.1;
        proxy_set_header Connection '';
        proxy_buffering off;
        proxy_cache off;
        chunked_transfer_encoding off;
    }
}
```

### Caddy 配置示例

```caddy
api.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

## 7. TLS/HTTPS 配置

推荐通过反向代理处理 TLS。使用 Caddy 可自动申请和续期 Let's Encrypt 证书。

## 8. 生产环境检查清单

部署至生产环境前，系统管理员需确认以下配置：

- 设置强 JWT 密钥（通过环境变量 `HB_JWT_SECRET`）
- 正确配置跨域（CORS）策略
- 启用结构化日志（tracing）以便于监控
- 配置定期备份机制
- 监控磁盘空间，特别是在文件存储模块使用 LocalFS 时
- 在反向代理层配置限流（Rate Limiting）以防止恶意攻击
- 备份并监控 `HB_DATA_DIR/web` 中托管的网页项目

## 9. 扩展与高可用

- **垂直扩展**：优化单实例资源分配，HertaBase 极低的内存占用使其在小机型上亦有良好表现。
- **水平扩展**：在企业级应用场景中，HertaBase 支持基于 TiKV 的集群模式（Phase 7）。

## 10. 数据备份与恢复

备份数据目录即可实现全量备份。恢复时替换对应的 `hb_data` 目录即可。
