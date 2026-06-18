# ============================================
# Stage 1: Rust 构建环境
# ============================================
FROM rust:1.75-alpine3.19 AS builder

WORKDIR /app

# 安装 musl 工具链和编译依赖
RUN apk add --no-cache \
    musl-dev \
    perl \
    make \
    gcc \
    g++ \
    pkgconfig \
    openssl-dev \
    ca-certificates \
    curl

# 添加 x86_64-unknown-linux-musl 目标
RUN rustup target add x86_64-unknown-linux-musl

# 先构建依赖层（利用 Docker 缓存）
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs

# 预构建依赖
RUN cargo build --release --target x86_64-unknown-linux-musl 2>/dev/null || true

# 拷贝源代码
COPY src/ src/
COPY config/ config/

# 替换掉 placeholder main.rs
RUN touch src/main.rs

# 静态编译
RUN RUSTFLAGS="-C target-feature=+crt-static" \
    cargo build --release --target x86_64-unknown-linux-musl

# strip 符号表减小体积
RUN strip /app/target/x86_64-unknown-linux-musl/release/hunyi-backend

# ============================================
# Stage 2: 运行时最小镜像（基于 alpine，~10MB）
# ============================================
FROM alpine:3.19 AS runtime

WORKDIR /app

# 安装运行时依赖（ca-certificates 用于 HTTPS/ClickHouse）
RUN apk add --no-cache ca-certificates tzdata

# 设置时区
ENV TZ=Asia/Shanghai
RUN ln -sf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime

# 拷贝编译好的二进制
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/hunyi-backend /usr/local/bin/hunyi-backend

# 拷贝配置文件
COPY --from=builder /app/config/ /app/config/

# 创建非 root 用户
RUN addgroup -S app && adduser -S app -G app
RUN chown -R app:app /app
USER app

# 健康检查
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

EXPOSE 8080 8081

CMD ["hunyi-backend"]
