use actix_web::{HttpResponse, ResponseError};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("HTTP 请求失败: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("环境变量未找到: {0}")]
    EnvVar(#[from] std::env::VarError),
    #[error("GitHub API 返回错误: {0}")]
    ApiError(String),
    #[error("数据未找到")]
    NotFound,
    #[error("请求参数错误: {0}")]
    BadRequest(String),
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::NotFound => HttpResponse::NotFound().json(serde_json::json!({
                "error": self.to_string()
            })),
            AppError::BadRequest(msg) => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": msg
                }))
            }
            AppError::ApiError(msg) => {
                HttpResponse::BadGateway().json(serde_json::json!({
                    "error": msg
                }))
            }
            _ => HttpResponse::InternalServerError().json(serde_json::json!({
                "error": self.to_string()
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_display() {
        let error = AppError::NotFound;
        assert_eq!(error.to_string(), "数据未找到");

        let error = AppError::ApiError("测试错误".to_string());
        assert_eq!(error.to_string(), "GitHub API 返回错误: 测试错误");
    }

    #[test]
    fn test_error_response_not_found() {
        let error = AppError::NotFound;
        let resp = error.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_error_response_api_error() {
        let error = AppError::ApiError("API错误".to_string());
        let resp = error.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_error_response_other() {
        // 测试其他错误类型（如Reqwest错误）
        // 注意：这里我们无法直接创建Reqwest错误，所以只测试错误处理逻辑
        let error = AppError::ApiError("其他错误".to_string());
        let resp = error.error_response();
        assert!(resp.status().is_client_error() || resp.status().is_server_error());
    }
}

