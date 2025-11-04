use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::sleep;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// 限流配置
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// 最大并发下载数
    pub max_concurrent_downloads: usize,
    /// 下载速度限制（字节/秒），0 表示无限制
    pub download_speed_limit: u64,
    /// 每时间窗口内的最大请求数
    pub max_requests_per_window: usize,
    /// 时间窗口大小（秒）
    pub window_duration_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: 10,
            download_speed_limit: 10 * 1024 * 1024, // 10 MB/s
            max_requests_per_window: 100,
            window_duration_secs: 60, // 1 分钟
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

        // 下载速度限制（支持 MB/s 或 KB/s 格式）
        let speed_limit = std::env::var("DOWNLOAD_SPEED_LIMIT")
            .ok()
            .and_then(|v| {
                let v = v.trim().to_lowercase();
                if v.ends_with("mb/s") || v.ends_with("mbs") {
                    v.trim_end_matches("mb/s")
                        .trim_end_matches("mbs")
                        .trim()
                        .parse::<u64>()
                        .ok()
                        .map(|mb| mb * 1024 * 1024)
                } else if v.ends_with("kb/s") || v.ends_with("kbs") {
                    v.trim_end_matches("kb/s")
                        .trim_end_matches("kbs")
                        .trim()
                        .parse::<u64>()
                        .ok()
                        .map(|kb| kb * 1024)
                } else {
                    v.parse::<u64>().ok()
                }
            })
            .unwrap_or(10 * 1024 * 1024);

        let max_requests = std::env::var("MAX_DOWNLOADS_PER_WINDOW")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);

        let window_duration = std::env::var("RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        Self {
            max_concurrent_downloads: max_concurrent,
            download_speed_limit: speed_limit,
            max_requests_per_window: max_requests,
            window_duration_secs: window_duration,
        }
    }
}

/// 请求记录（用于限流）
#[derive(Debug, Clone)]
struct RequestRecord {
    count: usize,
    window_start: Instant,
}

/// 限流管理器
pub struct RateLimitManager {
    config: RateLimitConfig,
    /// 并发下载信号量
    semaphore: Arc<Semaphore>,
    /// 请求限流记录（按 IP 地址）
    request_records: Arc<RwLock<HashMap<String, RequestRecord>>>,
}

impl RateLimitManager {
    pub fn new(config: RateLimitConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_downloads));
        Self {
            config,
            semaphore,
            request_records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检查是否可以开始新的下载（并发限制）
    pub async fn acquire_download_permit(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("Semaphore 不应该被关闭")
    }

    /// 检查请求频率限制（按 IP）
    pub async fn check_rate_limit(&self, ip: &str) -> Result<(), RateLimitError> {
        let mut records = self.request_records.write().await;

        // 清理过期的记录
        let now = Instant::now();
        let window_duration = Duration::from_secs(self.config.window_duration_secs);

        records.retain(|_, record| {
            now.duration_since(record.window_start) < window_duration
        });

        // 检查或创建记录
        let record = records.entry(ip.to_string()).or_insert_with(|| RequestRecord {
            count: 0,
            window_start: now,
        });

        // 如果窗口已过期，重置计数
        if now.duration_since(record.window_start) >= window_duration {
            record.count = 0;
            record.window_start = now;
        }

        // 检查是否超过限制
        if record.count >= self.config.max_requests_per_window {
            return Err(RateLimitError::TooManyRequests {
                limit: self.config.max_requests_per_window,
                window_secs: self.config.window_duration_secs,
            });
        }

        record.count += 1;
        Ok(())
    }

    /// 创建一个限速流包装器
    pub fn limit_speed<S>(&self, stream: S) -> RateLimitedStream<S>
    where
        S: futures::Stream<Item = Result<actix_web::web::Bytes, AppError>> + Unpin + Send + 'static,
    {
        RateLimitedStream {
            stream,
            speed_limit: self.config.download_speed_limit,
            last_send_time: Arc::new(TokioMutex::new(Instant::now())),
            bytes_sent: Arc::new(TokioMutex::new(0)),
        }
    }
}

/// 限速流包装器（使用 Stream 实现）
pub struct RateLimitedStream<S> {
    stream: S,
    speed_limit: u64, // 字节/秒
    last_send_time: Arc<TokioMutex<Instant>>,
    bytes_sent: Arc<TokioMutex<u64>>,
}

impl<S> RateLimitedStream<S>
where
    S: futures::Stream<Item = Result<actix_web::web::Bytes, AppError>> + Unpin + Send + 'static,
{
    /// 将限速流包装为 actix_web 兼容的 Stream
    pub fn into_stream(self) -> impl futures::Stream<Item = Result<actix_web::web::Bytes, AppError>> + Send + 'static {
        use futures::StreamExt;
        let speed_limit = self.speed_limit;
        let last_send_time = self.last_send_time;
        let bytes_sent = self.bytes_sent;

        self.stream.then(move |result| {
            let last_send_time = last_send_time.clone();
            let bytes_sent = bytes_sent.clone();
            let speed_limit = speed_limit;

            async move {
                if speed_limit == 0 {
                    // 无速度限制
                    return result;
                }

                let now = Instant::now();

                // 检查是否需要限速
                let need_throttle = {
                    let mut last_time = last_send_time.lock().await;
                    let mut sent = bytes_sent.lock().await;

                    let elapsed = now.duration_since(*last_time);

                    // 如果超过 1 秒，重置计数器
                    if elapsed.as_secs_f64() >= 1.0 {
                        *sent = 0;
                        *last_time = now;
                    }

                    // 检查是否达到速度限制
                    if *sent >= speed_limit {
                        true
                    } else {
                        false
                    }
                };

                // 如果达到速度限制，等待
                if need_throttle {
                    sleep(Duration::from_secs(1)).await;
                    // 重置计数器
                    let mut last_time = last_send_time.lock().await;
                    let mut sent = bytes_sent.lock().await;
                    *sent = 0;
                    *last_time = Instant::now();
                }

                match result {
                    Ok(bytes) => {
                        let chunk_size = bytes.len() as u64;
                        let mut sent = bytes_sent.lock().await;
                        *sent += chunk_size;
                        Ok(bytes)
                    }
                    Err(e) => Err(e),
                }
            }
        })
    }
}

/// 限流错误
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("请求过于频繁：在 {window_secs} 秒内最多允许 {limit} 次下载")]
    TooManyRequests { limit: usize, window_secs: u64 },
    #[error("并发下载数已达上限")]
    TooManyConcurrent,
}

// 需要导入 AppError，但这里先定义，稍后在 handlers 中处理
use crate::error::AppError;

impl From<RateLimitError> for AppError {
    fn from(err: RateLimitError) -> Self {
        match err {
            RateLimitError::TooManyRequests { limit, window_secs } => {
                AppError::BadRequest(format!(
                    "请求过于频繁：在 {} 秒内最多允许 {} 次下载",
                    window_secs, limit
                ))
            }
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

