# GitHub 信息收集服务

一个基于 Rust 和 Actix-Web 的高性能 GitHub 仓库信息收集服务，支持批量查询仓库信息、Releases 和最新版本信息。

## 功能特性

- 🚀 **高性能**：基于 Rust 和 Actix-Web 构建，支持高并发请求
- 📦 **批量查询**：支持一次性查询多个仓库的信息
- 💾 **智能缓存**：内置缓存机制，减少对 GitHub API 的请求
- 🔧 **灵活配置**：支持自定义字段选择，按需获取数据
- 🐳 **Docker 支持**：提供 Docker 镜像，便于部署
- 🛡️ **下载限流**：支持下载速度限制、并发限制和频率限制，防止流量盗刷

## API 端点

### 单个仓库查询

#### 1. 获取仓库基本信息

```bash
GET /repos/{owner}/{repo}
```

**示例请求：**
```bash
curl http://localhost:8080/repos/rust-lang/rust
```

**响应示例：**
```json
{
  "repo": "rust-lang/rust",
  "name": "rust",
  "full_name": "rust-lang/rust",
  "html_url": "https://github.com/rust-lang/rust",
  "description": "Empowering everyone to build reliable and efficient software.",
  "stargazers_count": 123456,
  "forks_count": 18000,
  "updated_at": "2024-01-01T00:00:00Z"
}
```

#### 2. 获取所有 Releases

```bash
GET /repos/{owner}/{repo}/releases
```

**示例请求：**
```bash
curl http://localhost:8080/repos/rust-lang/rust/releases
```

**响应示例：**
```json
[
  {
    "tag_name": "1.75.0",
    "name": "1.75.0",
    "changelog": "Release notes...",
    "published_at": "2024-01-01T00:00:00Z",
    "attachments": [
      ["rust-1.75.0-x86_64-unknown-linux-gnu.tar.gz", "https://github.com/.../download/..."]
    ]
  }
]
```

#### 3. 获取最新 Release

```bash
GET /repos/{owner}/{repo}/releases/latest
```

**示例请求：**
```bash
curl http://localhost:8080/repos/rust-lang/rust/releases/latest
```

**响应示例：**
```json
{
  "repo": "rust-lang/rust",
  "latest_version": "1.75.0",
  "changelog": "Release notes...",
  "published_at": "2024-01-01T00:00:00Z",
  "attachments": [
    ["rust-1.75.0-x86_64-unknown-linux-gnu.tar.gz", "https://github.com/.../download/..."]
  ]
}
```

### 批量查询

批量查询支持两种响应格式：
- **数组格式** (`/repos/batch`)：返回结果数组，便于遍历
- **Map 格式** (`/repos/batch/map`)：返回 Map 结构，便于按仓库名查找

#### 批量查询请求格式

```bash
POST /repos/batch
POST /repos/batch/map
```

**请求体：**
```json
{
  "repos": ["owner1/repo1", "owner2/repo2", ...],
  "fields": ["latest_release"]  // 可选，不指定则返回所有字段
}
```

**字段说明：**
- `repos`：仓库列表，格式为 `"owner/repo"`
- `fields`：可选字段列表，支持以下值：
  - `repo_info`：仓库基本信息
  - `releases`：所有 releases
  - `latest_release`：最新 release（包含版本号、附件链接、更新日志）
  - 不指定 `fields` 或为空数组时，返回所有字段

## 批量查询使用场景

### 场景 1：仅获取最新版本号

**请求示例：**
```bash
curl -X POST http://localhost:8080/repos/batch \
  -H "Content-Type: application/json" \
  -d '{
    "repos": [
      "rust-lang/rust",
      "microsoft/vscode",
      "facebook/react"
    ],
    "fields": ["latest_release"]
  }'
```

**响应示例（数组格式）：**
```json
{
  "results": [
    {
      "repo": "rust-lang/rust",
      "success": true,
      "latest_release": {
        "repo": "rust-lang/rust",
        "latest_version": "1.75.0",
        "changelog": null,
        "published_at": "2024-01-01T00:00:00Z",
        "attachments": []
      }
    },
    {
      "repo": "microsoft/vscode",
      "success": true,
      "latest_release": {
        "repo": "microsoft/vscode",
        "latest_version": "1.85.0",
        "changelog": null,
        "published_at": "2024-01-15T00:00:00Z",
        "attachments": []
      }
    },
    {
      "repo": "facebook/react",
      "success": true,
      "latest_release": {
        "repo": "facebook/react",
        "latest_version": "18.2.0",
        "changelog": null,
        "published_at": "2024-01-10T00:00:00Z",
        "attachments": []
      }
    }
  ]
}
```

**使用 Map 格式（便于按仓库名查找）：**
```bash
curl -X POST http://localhost:8080/repos/batch/map \
  -H "Content-Type: application/json" \
  -d '{
    "repos": ["rust-lang/rust", "microsoft/vscode"],
    "fields": ["latest_release"]
  }'
```

**响应示例（Map 格式）：**
```json
{
  "results_map": {
    "rust-lang/rust": {
      "repo": "rust-lang/rust",
      "success": true,
      "latest_release": {
        "repo": "rust-lang/rust",
        "latest_version": "1.75.0",
        "changelog": null,
        "published_at": "2024-01-01T00:00:00Z",
        "attachments": []
      }
    },
    "microsoft/vscode": {
      "repo": "microsoft/vscode",
      "success": true,
      "latest_release": {
        "repo": "microsoft/vscode",
        "latest_version": "1.85.0",
        "changelog": null,
        "published_at": "2024-01-15T00:00:00Z",
        "attachments": []
      }
    }
  }
}
```

### 场景 2：获取最新版本号 + 附件链接

**请求示例：**
```bash
curl -X POST http://localhost:8080/repos/batch \
  -H "Content-Type: application/json" \
  -d '{
    "repos": [
      "rust-lang/rust",
      "microsoft/vscode",
      "facebook/react"
    ],
    "fields": ["latest_release"]
  }'
```

**响应示例：**
```json
{
  "results": [
    {
      "repo": "rust-lang/rust",
      "success": true,
      "latest_release": {
        "repo": "rust-lang/rust",
        "latest_version": "1.75.0",
        "changelog": null,
        "published_at": "2024-01-01T00:00:00Z",
        "attachments": [
          ["rust-1.75.0-x86_64-unknown-linux-gnu.tar.gz", "https://github.com/rust-lang/rust/releases/download/1.75.0/rust-1.75.0-x86_64-unknown-linux-gnu.tar.gz"],
          ["rust-1.75.0-x86_64-pc-windows-msvc.msi", "https://github.com/rust-lang/rust/releases/download/1.75.0/rust-1.75.0-x86_64-pc-windows-msvc.msi"]
        ]
      }
    },
    {
      "repo": "microsoft/vscode",
      "success": true,
      "latest_release": {
        "repo": "microsoft/vscode",
        "latest_version": "1.85.0",
        "changelog": null,
        "published_at": "2024-01-15T00:00:00Z",
        "attachments": [
          ["VSCode-darwin-x64.zip", "https://github.com/microsoft/vscode/releases/download/1.85.0/VSCode-darwin-x64.zip"],
          ["VSCodeUserSetup-x64-1.85.0.exe", "https://github.com/microsoft/vscode/releases/download/1.85.0/VSCodeUserSetup-x64-1.85.0.exe"]
        ]
      }
    }
  ]
}
```

**说明：** `attachments` 字段是一个数组，每个元素是 `[文件名, 下载链接]` 的元组。

### 场景 3：获取最新版本号 + 附件链接 + 更新日志

**请求示例：**
```bash
curl -X POST http://localhost:8080/repos/batch \
  -H "Content-Type: application/json" \
  -d '{
    "repos": [
      "rust-lang/rust",
      "microsoft/vscode",
      "facebook/react"
    ],
    "fields": ["latest_release"]
  }'
```

**响应示例：**
```json
{
  "results": [
    {
      "repo": "rust-lang/rust",
      "success": true,
      "latest_release": {
        "repo": "rust-lang/rust",
        "latest_version": "1.75.0",
        "changelog": "## Version 1.75.0\n\n### Added\n- New features...\n\n### Fixed\n- Bug fixes...",
        "published_at": "2024-01-01T00:00:00Z",
        "attachments": [
          ["rust-1.75.0-x86_64-unknown-linux-gnu.tar.gz", "https://github.com/rust-lang/rust/releases/download/1.75.0/rust-1.75.0-x86_64-unknown-linux-gnu.tar.gz"],
          ["rust-1.75.0-x86_64-pc-windows-msvc.msi", "https://github.com/rust-lang/rust/releases/download/1.75.0/rust-1.75.0-x86_64-pc-windows-msvc.msi"]
        ]
      }
    },
    {
      "repo": "microsoft/vscode",
      "success": true,
      "latest_release": {
        "repo": "microsoft/vscode",
        "latest_version": "1.85.0",
        "changelog": "## 1.85.0 Release Notes\n\n### New Features\n- Feature 1\n- Feature 2",
        "published_at": "2024-01-15T00:00:00Z",
        "attachments": [
          ["VSCode-darwin-x64.zip", "https://github.com/microsoft/vscode/releases/download/1.85.0/VSCode-darwin-x64.zip"],
          ["VSCodeUserSetup-x64-1.85.0.exe", "https://github.com/microsoft/vscode/releases/download/1.85.0/VSCodeUserSetup-x64-1.85.0.exe"]
        ]
      }
    }
  ]
}
```

**说明：** `changelog` 字段包含完整的更新日志（Markdown 格式）。

## 错误处理

批量查询时，即使部分仓库查询失败，也会返回所有结果。失败的仓库会在响应中标记 `success: false` 并包含错误信息。

**响应示例（包含错误）：**
```json
{
  "results": [
    {
      "repo": "rust-lang/rust",
      "success": true,
      "latest_release": { ... }
    },
    {
      "repo": "invalid/repo",
      "success": false,
      "error": "仓库格式错误，应为 'owner/repo'"
    },
    {
      "repo": "notfound/repo",
      "success": false,
      "error": "仓库信息获取失败; 最新 release 获取失败"
    }
  ]
}
```

## 启动服务

### 使用 Cargo 运行

```bash
# 克隆项目
git clone <repository-url>
cd gh-info-rs

# 运行服务
cargo run

# 或指定绑定地址
BIND_ADDRESS=0.0.0.0:8080 cargo run
```

### 使用 Docker 运行

```bash
# 构建镜像
docker build -t gh-info-rs .

# 运行容器
docker run -p 8080:8080 gh-info-rs

# 或使用 docker-compose
docker-compose up
```

## 环境变量配置

### 基础配置

| 变量名 | 说明 | 默认值 |
|--------|------|--------|
| `BIND_ADDRESS` | 服务绑定地址 | `0.0.0.0:8080` |
| `GITHUB_TOKEN` | GitHub API Token（可选，用于提高 API 速率限制） | 无 |
| `LOG_LEVEL` | 日志级别（debug, info, warn, error） | `info` |
| `RUST_LOG` | 日志级别（兼容旧版本配置） | `info` |

### 缓存配置

| 变量名 | 说明 | 默认值 |
|--------|------|--------|
| `CACHE_ENABLED` | 是否启用缓存 | `true` |
| `CACHE_TTL_SECONDS` | 缓存过期时间（秒） | `3600` |
| `CACHE_FILE` | 缓存文件路径 | `cache.json` |
| `FILE_CACHE_DIR` | 文件缓存目录（可选，默认根据 CACHE_FILE 自动设置） | 自动 |

### 下载限流配置

为了防止流量盗刷，下载功能支持以下限流策略：

| 变量名 | 说明 | 默认值 | 示例 |
|--------|------|--------|------|
| `MAX_CONCURRENT_DOWNLOADS` | 最大并发下载数 | `10` | `20` |
| `DOWNLOAD_SPEED_LIMIT` | 下载速度限制（字节/秒），支持 `MB/s` 或 `KB/s` 单位 | `10MB/s` | `5MB/s` 或 `10240000` |
| `MAX_DOWNLOADS_PER_WINDOW` | 每个时间窗口内的最大下载次数 | `100` | `200` |
| `RATE_LIMIT_WINDOW_SECS` | 限流时间窗口大小（秒） | `60` | `300` |

**限流说明：**
- **并发限制**：限制同时进行的下载数量，超过限制的请求会等待或返回错误
- **速度限制**：限制每个下载的传输速度，防止单次下载占用过多带宽
- **频率限制**：按 IP 地址限制每个时间窗口内的下载次数，防止恶意刷流量

**配置示例：**
```bash
# 允许最多 20 个并发下载
MAX_CONCURRENT_DOWNLOADS=20

# 限制下载速度为 5MB/s
DOWNLOAD_SPEED_LIMIT=5mb/s

# 或者使用字节数（5MB = 5 * 1024 * 1024）
DOWNLOAD_SPEED_LIMIT=5242880

# 限制每分钟最多 50 次下载
MAX_DOWNLOADS_PER_WINDOW=50
RATE_LIMIT_WINDOW_SECS=60

# 限制每小时最多 200 次下载
MAX_DOWNLOADS_PER_WINDOW=200
RATE_LIMIT_WINDOW_SECS=3600
```

**限流错误响应：**
当触发限流时，API 会返回 `429 Too Many Requests` 状态码：
```json
{
  "error": "请求过于频繁：在 60 秒内最多允许 100 次下载",
  "retry_after": 60
}
```

**示例：**
```bash
export GITHUB_TOKEN=your_github_token_here
export BIND_ADDRESS=0.0.0.0:8080
export LOG_LEVEL=debug
cargo run
```

## 测试

### 运行单元测试

```bash
cargo test
```

### 运行集成测试

```bash
cargo test --test integration_test
```

### 使用测试脚本

```bash
# 确保服务正在运行
cargo run

# 在另一个终端运行测试脚本
cd tests
./test_batch.sh

# 或指定服务器地址
SERVER_URL=http://localhost:8080 ./test_batch.sh
```

## 性能特性

- **并发处理**：批量查询时，所有仓库的请求会并发执行
- **智能缓存**：使用内存缓存减少对 GitHub API 的请求
- **错误隔离**：单个仓库查询失败不影响其他仓库的结果

## 许可证

[添加许可证信息]

## 贡献

欢迎提交 Issue 和 Pull Request！

