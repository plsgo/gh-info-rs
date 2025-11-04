use crate::cache::get_cache_manager;
use crate::error::AppError;
use crate::models::{
    BatchRequest, BatchResponse, BatchResponseMap, GithubRelease, GithubRepo,
    LatestReleaseInfo, ReleaseInfo, RepoBatchResult, RepoInfo,
};
use crate::rate_limit::get_rate_limit_manager;
use actix_web::{get, post, web, HttpResponse, Responder, HttpRequest};
use futures::future::join_all;
use futures::join;
use futures::StreamExt;
use log;
use reqwest::Client;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use sha2::{Sha256, Digest};
use tokio::fs;
use tokio::io::AsyncWriteExt;

// 获取 GitHub token（可选，如果设置了环境变量则使用）
fn get_github_token() -> Option<String> {
    dotenv::dotenv().ok();
    env::var("GITHUB_TOKEN").ok()
}

// 创建 GitHub API 请求客户端
fn create_client() -> Client {
    Client::new()
}

// 获取仓库基本信息
pub async fn fetch_repo_info(owner: &str, repo: &str) -> Result<RepoInfo, AppError> {
    let cache = get_cache_manager().await;

    // 先尝试从缓存获取
    if let Some(cached_info) = cache.get_repo_info(owner, repo).await {
        log::debug!("从缓存获取仓库信息: {}/{}", owner, repo);
        return Ok(cached_info);
    }

    // 缓存未命中，从 API 获取
    log::debug!("从 GitHub API 获取仓库信息: {}/{}", owner, repo);
    let client = create_client();
    let api_url = format!("https://api.github.com/repos/{}/{}", owner, repo);

    let mut request = client
        .get(&api_url)
        .header("User-Agent", "gh-info-rs")
        .header("Accept", "application/vnd.github.v3+json");

    // 如果设置了 token，则添加认证头
    if let Some(token) = get_github_token() {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        if response.status().as_u16() == 404 {
            return Err(AppError::NotFound);
        }
        return Err(AppError::ApiError(format!(
            "GitHub API 返回状态码: {}",
            response.status()
        )));
    }

    let github_repo: GithubRepo = response.json().await?;

    let repo_info = RepoInfo {
        repo: format!("{}/{}", owner, repo),
        name: github_repo.name,
        full_name: github_repo.full_name,
        html_url: github_repo.html_url,
        description: github_repo.description,
        stargazers_count: github_repo.stargazers_count,
        forks_count: github_repo.forks_count,
        updated_at: github_repo.updated_at,
    };

    // 存入缓存
    cache.set_repo_info(owner, repo, repo_info.clone()).await;
    log::debug!("成功获取并缓存仓库信息: {}/{}", owner, repo);

    Ok(repo_info)
}

// 获取所有 releases
pub async fn fetch_releases(owner: &str, repo: &str) -> Result<Vec<ReleaseInfo>, AppError> {
    let cache = get_cache_manager().await;

    // 先尝试从缓存获取
    if let Some(cached_releases) = cache.get_releases(owner, repo).await {
        log::debug!("从缓存获取 releases: {}/{} (共 {} 个)", owner, repo, cached_releases.len());
        return Ok(cached_releases);
    }

    // 缓存未命中，从 API 获取
    log::debug!("从 GitHub API 获取 releases: {}/{}", owner, repo);
    let client = create_client();
    let api_url = format!("https://api.github.com/repos/{}/{}/releases", owner, repo);

    let mut request = client
        .get(&api_url)
        .header("User-Agent", "gh-info-rs")
        .header("Accept", "application/vnd.github.v3+json");

    if let Some(token) = get_github_token() {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        if response.status().as_u16() == 404 {
            return Err(AppError::NotFound);
        }
        return Err(AppError::ApiError(format!(
            "GitHub API 返回状态码: {}",
            response.status()
        )));
    }

    let releases: Vec<GithubRelease> = response.json().await?;

    let release_infos: Vec<ReleaseInfo> = releases
        .into_iter()
        .map(|r| ReleaseInfo {
            tag_name: r.tag_name,
            name: r.name,
            changelog: r.body,
            published_at: r.published_at,
            attachments: r
                .assets
                .into_iter()
                .map(|a| (a.name, a.download_url))
                .collect(),
        })
        .collect();

    // 存入缓存
    cache.set_releases(owner, repo, release_infos.clone()).await;
    log::debug!("成功获取并缓存 releases: {}/{} (共 {} 个)", owner, repo, release_infos.len());

    Ok(release_infos)
}

// 获取最新 release
pub async fn fetch_latest_release(owner: &str, repo: &str) -> Result<LatestReleaseInfo, AppError> {
    let cache = get_cache_manager().await;

    // 先尝试从缓存获取
    if let Some(cached_release) = cache.get_latest_release(owner, repo).await {
        log::debug!("从缓存获取最新 release: {}/{} (版本: {})", owner, repo, cached_release.latest_version);
        return Ok(cached_release);
    }

    // 缓存未命中，从 API 获取
    log::debug!("从 GitHub API 获取最新 release: {}/{}", owner, repo);
    let client = create_client();
    let api_url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        owner, repo
    );

    let mut request = client
        .get(&api_url)
        .header("User-Agent", "gh-info-rs")
        .header("Accept", "application/vnd.github.v3+json");

    if let Some(token) = get_github_token() {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        if response.status().as_u16() == 404 {
            return Err(AppError::NotFound);
        }
        return Err(AppError::ApiError(format!(
            "GitHub API 返回状态码: {}",
            response.status()
        )));
    }

    let release: GithubRelease = response.json().await?;

    let latest_release = LatestReleaseInfo {
        repo: format!("{}/{}", owner, repo),
        latest_version: release.tag_name,
        changelog: release.body,
        published_at: release.published_at,
        attachments: release
            .assets
            .into_iter()
            .map(|a| (a.name, a.download_url))
            .collect(),
    };

    // 存入缓存
    cache
        .set_latest_release(owner, repo, latest_release.clone())
        .await;
    log::debug!("成功获取并缓存最新 release: {}/{} (版本: {})", owner, repo, latest_release.latest_version);

    Ok(latest_release)
}

// API 端点：GET / - 健康检查和基本信息
#[utoipa::path(
    get,
    path = "/",
    tag = "health",
    responses(
        (status = 200, description = "服务健康", body = HealthResponse)
    )
)]
#[get("/")]
pub async fn health_check() -> impl Responder {
    use crate::models::HealthResponse;
    HttpResponse::Ok().json(HealthResponse {
        status: "ok".to_string(),
        service: "GitHub API 信息收集服务".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

// API 端点：GET /health - 健康检查端点
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "服务健康", body = HealthResponse)
    )
)]
#[get("/health")]
pub async fn health() -> impl Responder {
    use crate::models::HealthResponse;
    HttpResponse::Ok().json(HealthResponse {
        status: "ok".to_string(),
        service: "GitHub API 信息收集服务".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

// API 端点：GET /repos/{owner}/{repo}
#[utoipa::path(
    get,
    path = "/repos/{owner}/{repo}",
    tag = "repos",
    params(
        ("owner" = String, Path, description = "仓库所有者"),
        ("repo" = String, Path, description = "仓库名称")
    ),
    responses(
        (status = 200, description = "成功获取仓库信息", body = RepoInfo),
        (status = 404, description = "仓库不存在")
    )
)]
#[get("/repos/{owner}/{repo}")]
pub async fn get_repo_info(path: web::Path<(String, String)>) -> Result<impl Responder, AppError> {
    let (owner, repo) = path.into_inner();
    log::info!("请求: GET /repos/{}/{}", owner, repo);
    let repo_info = fetch_repo_info(&owner, &repo).await?;
    Ok(HttpResponse::Ok().json(repo_info))
}

// API 端点：GET /repos/{owner}/{repo}/releases
#[utoipa::path(
    get,
    path = "/repos/{owner}/{repo}/releases",
    tag = "repos",
    params(
        ("owner" = String, Path, description = "仓库所有者"),
        ("repo" = String, Path, description = "仓库名称")
    ),
    responses(
        (status = 200, description = "成功获取所有 releases", body = Vec<ReleaseInfo>),
        (status = 404, description = "仓库不存在")
    )
)]
#[get("/repos/{owner}/{repo}/releases")]
pub async fn get_releases(path: web::Path<(String, String)>) -> Result<impl Responder, AppError> {
    let (owner, repo) = path.into_inner();
    log::info!("请求: GET /repos/{}/{}/releases", owner, repo);
    let releases = fetch_releases(&owner, &repo).await?;
    Ok(HttpResponse::Ok().json(releases))
}

// API 端点：GET /repos/{owner}/{repo}/releases/latest
#[utoipa::path(
    get,
    path = "/repos/{owner}/{repo}/releases/latest",
    tag = "repos",
    params(
        ("owner" = String, Path, description = "仓库所有者"),
        ("repo" = String, Path, description = "仓库名称")
    ),
    responses(
        (status = 200, description = "成功获取最新 release", body = LatestReleaseInfo),
        (status = 404, description = "仓库不存在或没有 releases")
    )
)]
#[get("/repos/{owner}/{repo}/releases/latest")]
pub async fn get_latest_release(
    path: web::Path<(String, String)>,
) -> Result<impl Responder, AppError> {
    let (owner, repo) = path.into_inner();
    log::info!("请求: GET /repos/{}/{}/releases/latest", owner, repo);
    let release = fetch_latest_release(&owner, &repo).await?;
    Ok(HttpResponse::Ok().json(release))
}

// 解析仓库字符串 "owner/repo" 为 (owner, repo)
fn parse_repo(repo_str: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = repo_str.split('/').collect();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repo_valid() {
        assert_eq!(
            parse_repo("owner/repo"),
            Some(("owner".to_string(), "repo".to_string()))
        );
        assert_eq!(
            parse_repo("octocat/Hello-World"),
            Some(("octocat".to_string(), "Hello-World".to_string()))
        );
    }

    #[test]
    fn test_parse_repo_invalid() {
        assert_eq!(parse_repo("invalid"), None);
        assert_eq!(parse_repo("owner/repo/extra"), None);
        assert_eq!(parse_repo(""), None);
        assert_eq!(parse_repo("owner/"), None);
        assert_eq!(parse_repo("/repo"), None);
    }
}

// 处理单个仓库的批量请求
async fn process_single_repo(repo_str: &str, fields: &[String]) -> RepoBatchResult {
    let (owner, repo) = match parse_repo(repo_str) {
        Some(parsed) => parsed,
        None => {
            return RepoBatchResult {
                repo: repo_str.to_string(),
                success: false,
                error: Some("仓库格式错误，应为 'owner/repo'".to_string()),
                repo_info: None,
                releases: None,
                latest_release: None,
            };
        }
    };

    // 如果没有指定fields，默认获取全部
    let should_get_repo_info = fields.is_empty() || fields.contains(&"repo_info".to_string());
    let should_get_releases = fields.is_empty() || fields.contains(&"releases".to_string());
    let should_get_latest_release =
        fields.is_empty() || fields.contains(&"latest_release".to_string());

    // 并发获取所有请求的数据
    let repo_info_future = if should_get_repo_info {
        Some(fetch_repo_info(&owner, &repo))
    } else {
        None
    };

    let releases_future = if should_get_releases {
        Some(fetch_releases(&owner, &repo))
    } else {
        None
    };

    let latest_release_future = if should_get_latest_release {
        Some(fetch_latest_release(&owner, &repo))
    } else {
        None
    };

    // 并发执行所有请求
    let (repo_info_result, releases_result, latest_release_result) = join!(
        async {
            match repo_info_future {
                Some(f) => f.await.ok(),
                None => None,
            }
        },
        async {
            match releases_future {
                Some(f) => f.await.ok(),
                None => None,
            }
        },
        async {
            match latest_release_future {
                Some(f) => f.await.ok(),
                None => None,
            }
        }
    );

    // 检查是否有任何错误并生成错误消息
    let mut error_parts = Vec::new();

    if should_get_repo_info && repo_info_result.is_none() {
        error_parts.push("仓库信息获取失败");
    }
    if should_get_releases && releases_result.is_none() {
        error_parts.push("releases 获取失败");
    }
    if should_get_latest_release && latest_release_result.is_none() {
        error_parts.push("最新 release 获取失败");
    }

    let has_error = !error_parts.is_empty();
    let error_message = if has_error {
        Some(error_parts.join("; "))
    } else {
        None
    };

    RepoBatchResult {
        repo: repo_str.to_string(),
        success: !has_error,
        error: error_message,
        repo_info: repo_info_result,
        releases: releases_result,
        latest_release: latest_release_result,
    }
}

// API 端点：POST /repos/batch - 批量获取多个仓库的信息（返回数组格式）
#[utoipa::path(
    post,
    path = "/repos/batch",
    tag = "repos",
    request_body = BatchRequest,
    responses(
        (status = 200, description = "批量获取成功", body = BatchResponse),
        (status = 400, description = "请求参数错误")
    )
)]
#[post("/repos/batch")]
pub async fn batch_get_repos(body: web::Json<BatchRequest>) -> Result<impl Responder, AppError> {
    let repos = &body.repos;
    let fields = &body.fields;

    if repos.is_empty() {
        return Err(AppError::BadRequest("repos 列表不能为空".to_string()));
    }

    log::info!("请求: POST /repos/batch (共 {} 个仓库)", repos.len());

    // 并发处理所有仓库
    let futures: Vec<_> = repos
        .iter()
        .map(|repo| process_single_repo(repo, fields))
        .collect();

    let results = join_all(futures).await;

    let success_count = results.iter().filter(|r| r.success).count();
    log::info!("批量请求完成: 成功 {}/{}", success_count, repos.len());

    Ok(HttpResponse::Ok().json(BatchResponse { results }))
}

// API 端点：POST /repos/batch/map - 批量获取多个仓库的信息（返回 Map 格式，方便客户端处理）
#[utoipa::path(
    post,
    path = "/repos/batch/map",
    tag = "repos",
    request_body = BatchRequest,
    responses(
        (status = 200, description = "批量获取成功", body = BatchResponseMap),
        (status = 400, description = "请求参数错误")
    )
)]
#[post("/repos/batch/map")]
pub async fn batch_get_repos_map(
    body: web::Json<BatchRequest>,
) -> Result<impl Responder, AppError> {
    let repos = &body.repos;
    let fields = &body.fields;

    if repos.is_empty() {
        return Err(AppError::BadRequest("repos 列表不能为空".to_string()));
    }

    log::info!("请求: POST /repos/batch/map (共 {} 个仓库)", repos.len());

    // 并发处理所有仓库
    let futures: Vec<_> = repos
        .iter()
        .map(|repo| process_single_repo(repo, fields))
        .collect();

    let results = join_all(futures).await;

    // 将结果转换为 HashMap，使用 repo 作为 key
    let results_map: HashMap<String, RepoBatchResult> = results
        .into_iter()
        .map(|result| (result.repo.clone(), result))
        .collect();

    let success_count = results_map.values().filter(|r| r.success).count();
    log::info!("批量请求完成: 成功 {}/{}", success_count, repos.len());

    Ok(HttpResponse::Ok().json(BatchResponseMap { results_map }))
}

// 下载附件文件（支持缓存）
#[utoipa::path(
    get,
    path = "/download",
    tag = "download",
    params(
        ("url" = String, Query, description = "要下载的文件 URL")
    ),
    responses(
        (status = 200, description = "文件下载成功", content_type = "application/octet-stream"),
        (status = 400, description = "缺少 url 参数")
    )
)]
#[get("/download")]
pub async fn download_attachment(
    req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
) -> Result<impl Responder, AppError> {
    let url = query.get("url").ok_or_else(|| {
        AppError::BadRequest("缺少 url 参数".to_string())
    })?;

    // 获取客户端 IP 地址（用于限流）
    let client_ip = req
        .connection_info()
        .peer_addr()
        .map(|s| s.to_string())
        .or_else(|| {
            // 尝试从 X-Forwarded-For 或 X-Real-IP 获取（如果使用反向代理）
            req.headers()
                .get("X-Forwarded-For")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(|s| s.trim().to_string())
                .or_else(|| {
                    req.headers()
                        .get("X-Real-IP")
                        .and_then(|h| h.to_str().ok())
                        .map(|s| s.to_string())
                })
        })
        .unwrap_or_else(|| "unknown".to_string());

    log::info!("请求下载文件: {} (IP: {})", url, client_ip);

    // 获取限流管理器并获取并发下载许可
    let rate_limit_manager = get_rate_limit_manager().await;

    // 获取并发下载许可（这会在下载完成后自动释放）
    let permit = rate_limit_manager.acquire_download_permit().await;

    let cache = get_cache_manager().await;

    // 先检查缓存
    if let Some(metadata) = cache.get_file_cache(url).await {
        log::debug!("从缓存获取文件: {}", url);

        let content_type = metadata.content_type
            .as_ref()
            .and_then(|ct| ct.parse::<mime::Mime>().ok())
            .unwrap_or_else(|| mime::APPLICATION_OCTET_STREAM);

        let filename = metadata.original_filename.clone();
        let file_path = metadata.file_path.clone();

        // 使用流式读取缓存文件（避免一次性加载大文件到内存）
        use actix_web::web::Bytes;
        use futures::stream::TryStreamExt;

        let file = fs::File::open(&file_path).await
            .map_err(|e| AppError::ApiError(format!("打开缓存文件失败: {}", e)))?;

        let stream = tokio_util::io::ReaderStream::new(file);
        let bytes_stream = stream.map_ok(|b| Bytes::from(b))
            .map(|r| r.map_err(|e| AppError::ApiError(format!("读取文件错误: {}", e))));

        // 将 permit 绑定到流上，确保在整个流完成之前都不会释放
        // 使用 map 将 permit 移动到闭包中，permit 会在流完成时自动释放
        // 注意：permit 需要在整个流期间保持，所以将其移动到闭包的捕获中
        let permit_for_stream = permit;
        let stream_with_permit = bytes_stream.map(move |result| {
            // permit_for_stream 在闭包中保持，直到流完成
            let _keep_permit = &permit_for_stream;
            result
        });

        return Ok(HttpResponse::Ok()
            .content_type(content_type.clone())
            .append_header((
                "Content-Disposition",
                format!("attachment; filename=\"{}\"", filename)
            ))
            .streaming(stream_with_permit));
    }

    // 缓存未命中，从 GitHub 流式下载
    log::debug!("从 GitHub 流式下载文件: {}", url);
    let client = create_client();

    let mut request = client
        .get(url)
        .header("User-Agent", "gh-info-rs")
        .header("Accept", "*/*");

    // 如果设置了 token，则添加认证头
    if let Some(token) = get_github_token() {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        return Err(AppError::ApiError(format!(
            "GitHub 返回状态码: {}",
            response.status()
        )));
    }

    // 先获取 Content-Type（在移动 response 之前）
    let content_type = response.headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .and_then(|ct| ct.parse::<mime::Mime>().ok())
        .unwrap_or_else(|| mime::APPLICATION_OCTET_STREAM);

    // 从 URL 提取文件名
    let filename = url
        .split('/')
        .last()
        .unwrap_or("file")
        .split('?')
        .next()
        .unwrap_or("file")
        .to_string();

    // 生成缓存文件名（基于 URL 的 hash）
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let file_hash = hex::encode(hasher.finalize());

    // 尝试从文件名获取扩展名
    let filename_path = PathBuf::from(&filename);
    let extension = filename_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("bin");

    let cache_filename = format!("{}.{}", file_hash, extension);
    let cache_file_path = cache.get_file_cache_dir().join(&cache_filename);
    let filename_clone = filename.clone();
    let url_clone = url.to_string();
    let content_type_str = content_type.to_string();

    // 创建缓存文件（用于写入）
    let cache_file = fs::File::create(&cache_file_path).await
        .map_err(|e| AppError::ApiError(format!("创建缓存文件失败: {}", e)))?;

    // 获取响应流并转换为字节流
    let bytes_stream = response.bytes_stream();

    // 创建一个流，同时写入缓存和发送给客户端
    // 使用 channel 来分离写入任务，避免阻塞流
    use tokio::sync::mpsc;
    use actix_web::web::Bytes;

    let (tx, mut rx) = mpsc::channel::<Bytes>(100);
    let tx_for_stream = tx.clone(); // mpsc::Sender 实现了 Clone
    let cache_file_path_clone = cache_file_path.clone();
    let url_for_cache = url_clone.clone();
    let filename_for_cache = filename_clone.clone();
    let content_type_for_cache = content_type_str.clone();

    // 启动后台任务写入缓存文件
    tokio::spawn(async move {
        let mut file = cache_file;
        while let Some(bytes) = rx.recv().await {
            if let Err(e) = file.write_all(&bytes).await {
                log::warn!("写入缓存文件失败: {}", e);
                break;
            }
        }

        // 文件写入完成，刷新并更新缓存元数据
        if let Err(e) = file.flush().await {
            log::warn!("刷新缓存文件失败: {}", e);
        }

        let cache = get_cache_manager().await;
        cache.set_file_cache(
            &url_for_cache,
            cache_file_path_clone,
            filename_for_cache,
            Some(content_type_for_cache),
        ).await;
        log::info!("文件已流式下载并缓存: {}", url_for_cache);
    });

    // 创建一个流，将数据同时发送给客户端和缓存写入任务
    // 将 permit 绑定到流上，确保在整个流完成之前都不会释放
    // 注意：permit 需要在整个流期间保持，所以将其移动到闭包的捕获中
    let permit_for_stream = permit;
    let stream = bytes_stream.map(move |result| {
        // permit_for_stream 在闭包中保持，直到流完成
        let _keep_permit = &permit_for_stream;
        match result {
            Ok(bytes) => {
                // 发送到缓存写入任务（非阻塞，如果 channel 满了就丢弃）
                let _ = tx_for_stream.try_send(bytes.clone());
                Ok(bytes)
            }
            Err(e) => Err(AppError::ApiError(format!("流式下载错误: {}", e))),
        }
    });

    Ok(HttpResponse::Ok()
        .content_type(content_type.clone())
        .append_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", filename)
        ))
        .streaming(stream))
}
