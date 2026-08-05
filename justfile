# HertaBase Justfile

default:
    @just --list

# 启动前端开发服务器
dev-ui:
    pnpm --filter @hb/admin-ui dev

# 启动后端开发服务器 (需安装 cargo-watch)
dev-server:
    cargo watch -x "run -p herta_server"

# 启动后端内存数据库开发服务器 (用于调试，无需写磁盘)
dev-server-mem:
    cargo watch -x "run -p herta_server -- --db-engine memory"

# 构建前端产物
build-ui:
    pnpm --filter @hb/admin-ui build

# 编译后端二进制 (Release)
build-server:
    cargo build --release

# 全量构建 (先前端后后端)
build: build-ui build-server

# 代码检查 (Clippy + Frontend Lint)
lint:
    pnpm --filter @hb/admin-ui lint
    cargo clippy --workspace -- -D warnings

# 运行测试
test:
    cargo test --workspace

# 清理构建产物
clean:
    cargo clean
    rm -rf node_modules frontend/*/node_modules packages/*/node_modules
