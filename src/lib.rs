pub mod cache;
pub mod error;
pub mod handlers;
pub mod models;
pub mod rate_limit;

use utoipa::OpenApi;
use crate::models::{
    HealthResponse, RepoInfo, ReleaseInfo, LatestReleaseInfo, BatchRequest, RepoBatchResult, BatchResponse, BatchResponseMap
};

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::health_check,
        handlers::health,
        handlers::get_repo_info,
        handlers::get_releases,
        handlers::get_latest_release,
        handlers::get_latest_release_pre,
        handlers::batch_get_repos,
        handlers::batch_get_repos_map,
        handlers::download_attachment,
    ),
    components(schemas(
        HealthResponse,
        RepoInfo,
        ReleaseInfo,
        LatestReleaseInfo,
        BatchRequest,
        RepoBatchResult,
        BatchResponse,
        BatchResponseMap,
    )),
    tags(
        (name = "health", description = "健康检查端点"),
        (name = "repos", description = "仓库信息相关端点"),
        (name = "download", description = "文件下载端点"),
    ),
)]
pub struct ApiDoc;

