# 构建阶段 - 使用 Alpine（更小的镜像）
FROM rust:1.91.0-alpine3.22 AS builder

WORKDIR /app

# 安装必要的构建依赖（Alpine 使用 musl libc）
# 注意：使用 rustls 后不需要 OpenSSL 开发库
RUN apk add --no-cache \
    musl-dev \
    ca-certificates

# 复制依赖文件并构建（利用 Docker 缓存层）
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# 复制源代码并构建
COPY src ./src
RUN cargo build --release && \
    strip target/release/gh-info-rs

# 运行阶段 - 使用 Alpine 最小镜像（仅 ~5MB）
FROM alpine:latest

WORKDIR /app

# 安装运行时依赖（仅 ca-certificates 用于 HTTPS 证书验证）
# 使用 rustls 后不需要 OpenSSL 运行时库
RUN apk add --no-cache ca-certificates

# 从构建阶段复制二进制文件
COPY --from=builder /app/target/release/gh-info-rs /app/gh-info-rs

# 暴露端口
EXPOSE 8080

# 设置环境变量（默认值，可通过运行时环境变量覆盖）
ENV LOG_LEVEL=info

# 运行应用
CMD ["./gh-info-rs"]

