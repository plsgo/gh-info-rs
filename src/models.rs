use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// 健康检查响应结构
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

// GitHub API 返回的仓库基本信息
#[derive(Debug, Deserialize, Serialize)]
pub struct GithubRepo {
    pub name: String,
    #[serde(rename = "full_name")]
    pub full_name: String,
    #[serde(rename = "html_url")]
    pub html_url: String,
    pub description: Option<String>,
    #[serde(rename = "stargazers_count")]
    pub stargazers_count: u32,
    #[serde(rename = "forks_count")]
    pub forks_count: u32,
    #[serde(rename = "updated_at")]
    pub updated_at: String,
}

// GitHub API 返回的 Release Asset
#[derive(Debug, Deserialize, Serialize)]
pub struct GithubAsset {
    pub name: String,
    #[serde(rename = "browser_download_url")]
    pub download_url: String,
}

// GitHub API 返回的 Release 数据
#[derive(Debug, Deserialize, Serialize)]
pub struct GithubRelease {
    #[serde(rename = "tag_name")]
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    #[serde(rename = "published_at")]
    pub published_at: String,
    pub assets: Vec<GithubAsset>,
}

// 整理后的仓库信息（用于 API 响应）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RepoInfo {
    pub repo: String,
    pub name: String,
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub stargazers_count: u32,
    pub forks_count: u32,
    pub updated_at: String,
}

// 整理后的 Release 信息（用于 API 响应）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: Option<String>,
    pub changelog: Option<String>,
    pub published_at: String,
    pub attachments: Vec<(String, String)>, // (名称, 下载链接)
}

// 整理后的最新版本信息（用于 API 响应）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LatestReleaseInfo {
    pub repo: String,
    pub latest_version: String,
    pub changelog: Option<String>,
    pub published_at: String,
    pub attachments: Vec<(String, String)>,
}

// 批量请求的数据结构
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct BatchRequest {
    pub repos: Vec<String>, // 格式: "owner/repo" 或 ["owner1/repo1", "owner2/repo2"]
    #[serde(default)]
    pub fields: Vec<String>, // 可选字段: "repo_info", "releases", "latest_release"，默认全部
}

// 单个仓库的批量响应结果
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RepoBatchResult {
    pub repo: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_info: Option<RepoInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub releases: Option<Vec<ReleaseInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_release: Option<LatestReleaseInfo>,
}

// 批量响应数据结构（数组格式）
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchResponse {
    pub results: Vec<RepoBatchResult>,
}

// 批量响应数据结构（Map 格式，方便客户端按 repo 查找）
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchResponseMap {
    #[serde(rename = "results_map")]
    pub results_map: std::collections::HashMap<String, RepoBatchResult>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_github_repo_deserialize() {
        let json = r#"{
            "name": "test",
            "full_name": "owner/test",
            "html_url": "https://github.com/owner/test",
            "description": "Test repo",
            "stargazers_count": 100,
            "forks_count": 50,
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;

        let repo: GithubRepo = serde_json::from_str(json).unwrap();
        assert_eq!(repo.name, "test");
        assert_eq!(repo.full_name, "owner/test");
        assert_eq!(repo.stargazers_count, 100);
        assert_eq!(repo.forks_count, 50);
    }

    #[test]
    fn test_github_release_deserialize() {
        let json = r#"{
            "tag_name": "v1.0.0",
            "name": "Release 1.0.0",
            "body": "Changelog",
            "published_at": "2024-01-01T00:00:00Z",
            "assets": [
                {
                    "name": "file.zip",
                    "browser_download_url": "https://example.com/file.zip"
                }
            ]
        }"#;

        let release: GithubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v1.0.0");
        assert_eq!(release.name, Some("Release 1.0.0".to_string()));
        assert_eq!(release.assets.len(), 1);
        assert_eq!(release.assets[0].name, "file.zip");
    }

    #[test]
    fn test_repo_info_serialize() {
        let repo_info = RepoInfo {
            repo: "owner/test".to_string(),
            name: "test".to_string(),
            full_name: "owner/test".to_string(),
            html_url: "https://github.com/owner/test".to_string(),
            description: Some("Test repo".to_string()),
            stargazers_count: 100,
            forks_count: 50,
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&repo_info).unwrap();
        assert!(json.contains("owner/test"));
        assert!(json.contains("stargazers_count"));
    }

    #[test]
    fn test_batch_request_deserialize() {
        let json = r#"{
            "repos": ["owner1/repo1", "owner2/repo2"],
            "fields": ["repo_info", "latest_release"]
        }"#;

        let request: BatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.repos.len(), 2);
        assert_eq!(request.fields.len(), 2);
    }

    #[test]
    fn test_batch_request_deserialize_empty_fields() {
        let json = r#"{
            "repos": ["owner/repo"]
        }"#;

        let request: BatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.repos.len(), 1);
        assert!(request.fields.is_empty());
    }

    #[test]
    fn test_repo_batch_result_serialize() {
        let result = RepoBatchResult {
            repo: "owner/test".to_string(),
            success: true,
            error: None,
            repo_info: Some(RepoInfo {
                repo: "owner/test".to_string(),
                name: "test".to_string(),
                full_name: "owner/test".to_string(),
                html_url: "https://github.com/owner/test".to_string(),
                description: None,
                stargazers_count: 0,
                forks_count: 0,
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            }),
            releases: None,
            latest_release: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("owner/test"));
        assert!(json.contains("success"));
        assert!(!json.contains("error")); // skip_serializing_if = "Option::is_none"
    }

    #[test]
    fn test_repo_batch_result_with_error() {
        let result = RepoBatchResult {
            repo: "owner/test".to_string(),
            success: false,
            error: Some("Not found".to_string()),
            repo_info: None,
            releases: None,
            latest_release: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("Not found"));
    }
}

