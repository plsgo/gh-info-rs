use std::sync::Arc;
use tokio::sync::Semaphore;

/// 限流配置
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// 最大并发下载数
    pub max_concurrent_downloads: usize,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: 10,
        }
    }
}

impl RateLimitConfig {
    /// 从环境变量加载配置
    pub fn from_env() -> Self {
        let max_concurrent = std::env::var("MAX_CONCURRENT_DOWNLOADS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        Self {
            max_concurrent_downloads: max_concurrent,
        }
    }
}

/// 限流管理器
pub struct RateLimitManager {
    #[allow(dead_code)]
    config: RateLimitConfig,
    /// 并发下载信号量
    semaphore: Arc<Semaphore>,
}

impl RateLimitManager {
    pub fn new(config: RateLimitConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_downloads));
        Self {
            config,
            semaphore,
        }
    }

    /// 获取并发下载许可（这会在下载完成后自动释放）
    pub async fn acquire_download_permit(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("Semaphore 不应该被关闭")
    }

    /// 获取当前配置的最大并发数（用于测试）
    #[cfg(test)]
    pub fn max_concurrent_downloads(&self) -> usize {
        self.config.max_concurrent_downloads
    }
}

/// 限流错误
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("并发下载数已达上限")]
    TooManyConcurrent,
}

// 需要导入 AppError
use crate::error::AppError;

impl From<RateLimitError> for AppError {
    fn from(err: RateLimitError) -> Self {
        match err {
            RateLimitError::TooManyConcurrent => {
                AppError::BadRequest("并发下载数已达上限，请稍后再试".to_string())
            }
        }
    }
}

// 全局限流管理器（使用 OnceCell）
use tokio::sync::OnceCell as AsyncOnceCell;

static RATE_LIMIT_MANAGER: AsyncOnceCell<Arc<RateLimitManager>> = AsyncOnceCell::const_new();

pub async fn get_rate_limit_manager() -> &'static Arc<RateLimitManager> {
    RATE_LIMIT_MANAGER
        .get_or_init(|| async {
            let config = RateLimitConfig::from_env();
            Arc::new(RateLimitManager::new(config))
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_concurrent_downloads, 10);
    }

    #[tokio::test]
    async fn test_rate_limit_config_from_env() {
        std::env::set_var("MAX_CONCURRENT_DOWNLOADS", "5");
        let config = RateLimitConfig::from_env();
        assert_eq!(config.max_concurrent_downloads, 5);
        std::env::remove_var("MAX_CONCURRENT_DOWNLOADS");
    }

    #[tokio::test]
    async fn test_rate_limit_manager_concurrent_limit() {
        let config = RateLimitConfig {
            max_concurrent_downloads: 2,
        };
        let manager = RateLimitManager::new(config);

        // 获取两个许可
        let permit1 = manager.acquire_download_permit().await;
        let permit2 = manager.acquire_download_permit().await;

        // 第三个许可应该被阻塞（但我们可以设置超时来测试）
        let permit3_future = manager.acquire_download_permit();
        
        // 使用 tokio::time::timeout 来测试超时
        let result = tokio::time::timeout(Duration::from_millis(100), permit3_future).await;
        
        // 应该超时，因为前两个许可还未释放
        assert!(result.is_err(), "第三个许可应该被阻塞");

        // 释放前两个许可
        drop(permit1);
        drop(permit2);

        // 现在应该可以获取第三个许可
        let permit3 = manager.acquire_download_permit().await;
        drop(permit3);
    }

    #[tokio::test]
    async fn test_rate_limit_manager_multiple_permits() {
        let config = RateLimitConfig {
            max_concurrent_downloads: 3,
        };
        let manager = RateLimitManager::new(config);

        // 同时获取多个许可
        let permits: Vec<_> = (0..3)
            .map(|_| manager.acquire_download_permit())
            .collect();

        let permits = futures::future::join_all(permits).await;

        // 验证所有许可都已获取
        assert_eq!(permits.len(), 3);

        // 释放所有许可
        drop(permits);
    }
}

