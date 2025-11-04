use crate::cache::get_cache_manager;
use crate::error::AppError;
use crate::models::{
    BatchRequest, BatchResponse, BatchResponseMap, GithubRelease, GithubRepo, LatestReleaseInfo,
    ReleaseInfo, RepoBatchResult, RepoInfo,
};
use actix_web::{get, post, web, HttpResponse, Responder};
use futures::future::join_all;
use futures::join;
use log;
use reqwest::Client;
use std::collections::HashMap;
use std::env;

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

// API 端点：GET /repos/{owner}/{repo}
#[get("/repos/{owner}/{repo}")]
pub async fn get_repo_info(path: web::Path<(String, String)>) -> Result<impl Responder, AppError> {
    let (owner, repo) = path.into_inner();
    log::info!("请求: GET /repos/{}/{}", owner, repo);
    let repo_info = fetch_repo_info(&owner, &repo).await?;
    Ok(HttpResponse::Ok().json(repo_info))
}

// API 端点：GET /repos/{owner}/{repo}/releases
#[get("/repos/{owner}/{repo}/releases")]
pub async fn get_releases(path: web::Path<(String, String)>) -> Result<impl Responder, AppError> {
    let (owner, repo) = path.into_inner();
    log::info!("请求: GET /repos/{}/{}/releases", owner, repo);
    let releases = fetch_releases(&owner, &repo).await?;
    Ok(HttpResponse::Ok().json(releases))
}

// API 端点：GET /repos/{owner}/{repo}/releases/latest
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
