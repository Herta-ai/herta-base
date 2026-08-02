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

## 3. Docker 部署

### 官方 Docker 镜像

官方镜像通过 GitHub Container Registry (GHCR) 进行发布：

```bash
docker pull ghcr.io/herta-ai/hertabase:latest
```

### Dockerfile 示例（多阶段构建）

```dockerfile
FROM rust:1.80 AS builder
WORKDIR /usr/src/hertabase
COPY . .
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

## 5. 反向代理

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

## 6. TLS/HTTPS 配置

推荐通过反向代理处理 TLS。使用 Caddy 可自动申请和续期 Let's Encrypt 证书。

## 7. 生产环境检查清单

部署至生产环境前，系统管理员需确认以下配置：

- 设置强 JWT 密钥（通过环境变量 `HB_JWT_SECRET`）
- 正确配置跨域（CORS）策略
- 启用结构化日志（tracing）以便于监控
- 配置定期备份机制
- 监控磁盘空间，特别是在文件存储模块使用 LocalFS 时
- 在反向代理层配置限流（Rate Limiting）以防止恶意攻击

## 8. 扩展与高可用

- **垂直扩展**：优化单实例资源分配，HertaBase 极低的内存占用使其在小机型上亦有良好表现。
- **水平扩展**：在企业级应用场景中，HertaBase 支持基于 TiKV 的集群模式（Phase 7）。

## 9. 数据备份与恢复

备份数据目录即可实现全量备份。恢复时替换对应的 `hb_data` 目录即可。

## 项目阶段 (Roadmap)

- Phase 1: 基础架构与动态 ORM — Salvo + SurrealDB 互通，动态 Collection CRUD，数据类型转换层，OpenAPI 自动生成
- Phase 2: 鉴权与权限引擎 — 用户系统（_users/_admins 表），JWT 签发与中间件，API Rules 动态规则引擎
- Phase 3: JS 扩展运行时 — rquickjs AsyncRuntime 集成，Rust→JS FFI 映射，生命周期 Hook 挂载与执行
- Phase 4: 实时订阅引擎 — 基于 SurrealDB LIVE SELECT + Salvo SSE 的数据变更推送
- Phase 5: 文件存储模块 — 抽象 Storage trait，LocalFS + S3 兼容云存储适配器
- Phase 6: 管理后台与单体打包 — SvelteKit Admin UI 开发，rust-embed 静态嵌入，Salvo 静态文件服务
- Phase 7: 生产加固与分布式 — CLI 工具 (clap)，结构化日志 (tracing)，RocksDB→TiKV 集群支持
