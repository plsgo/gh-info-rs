use crate::models::{LatestReleaseInfo, ReleaseInfo, RepoInfo};
use log;
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::interval;
use sha2::{Sha256, Digest};

// 缓存键类型
type CacheKey = String;

// 持久化缓存条目（带过期时间）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedEntry<T> {
    value: T,
    expires_at: u64, // Unix 时间戳（秒）
}

// 持久化缓存数据结构
#[derive(Debug, Serialize, Deserialize)]
struct PersistentCache {
    repo_info: HashMap<String, CachedEntry<RepoInfo>>,
    releases: HashMap<String, CachedEntry<Vec<ReleaseInfo>>>,
    latest_release: HashMap<String, CachedEntry<LatestReleaseInfo>>,
}

// 缓存配置
#[derive(Clone)]
pub struct CacheConfig {
    pub enabled: bool,
    pub ttl_seconds: u64,
}

impl CacheConfig {
    pub fn from_env() -> Self {
        dotenv::dotenv().ok();

        let enabled = env::var("CACHE_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);

        let ttl_seconds = env::var("CACHE_TTL_SECONDS")
            .unwrap_or_else(|_| "3600".to_string()) // 默认 1 小时
            .parse::<u64>()
            .unwrap_or(3600);

        CacheConfig {
            enabled,
            ttl_seconds,
        }
    }
}

// 文件缓存元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCacheMetadata {
    pub url: String,
    pub file_path: PathBuf,
    pub original_filename: String,
    pub content_type: Option<String>,
    pub expires_at: u64,
    pub last_accessed_at: u64, // 最后访问时间（Unix 时间戳，秒）
}

// 缓存管理器
pub struct CacheManager {
    config: CacheConfig,
    repo_info_cache: Cache<CacheKey, RepoInfo>,
    releases_cache: Cache<CacheKey, Vec<ReleaseInfo>>,
    latest_release_cache: Cache<CacheKey, LatestReleaseInfo>,
    file_cache: Cache<CacheKey, FileCacheMetadata>,
    // 持久化存储（用于保存和加载）
    persistent_store: Arc<RwLock<PersistentCache>>,
    cache_file_path: PathBuf,
    file_cache_dir: PathBuf,
    // 文件路径到缓存键的映射（用于清理时查找）
    file_path_to_key: Arc<RwLock<HashMap<PathBuf, CacheKey>>>,
}

impl CacheManager {
    pub async fn new(config: CacheConfig) -> Self {
        let ttl = Duration::from_secs(config.ttl_seconds);

        // 确定缓存文件路径（使用环境变量 CACHE_FILE，默认当前目录下的 cache.json）
        let cache_file_path = env::var("CACHE_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("cache.json"));

        // 确定文件缓存目录（使用环境变量 FILE_CACHE_DIR）
        // 如果未设置，则根据 CACHE_FILE 的父目录智能推断
        let file_cache_dir = env::var("FILE_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                // 如果 CACHE_FILE 在 /app/data/ 目录下，则使用 /app/data/cache_files
                // 否则使用 cache_files（与 cache.json 同级）
                if let Some(parent) = cache_file_path.parent() {
                    if parent == PathBuf::from("/app/data") {
                        PathBuf::from("/app/data/cache_files")
                    } else {
                        parent.join("cache_files")
                    }
                } else {
                    PathBuf::from("cache_files")
                }
            });

        // 确保缓存目录存在
        if let Some(parent) = cache_file_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                log::warn!("无法创建缓存目录: {:?}, 错误: {}", parent, e);
            }
        }

        // 确保文件缓存目录存在
        if let Err(e) = std::fs::create_dir_all(&file_cache_dir) {
            log::warn!("无法创建文件缓存目录: {:?}, 错误: {}", file_cache_dir, e);
        }

        // 创建持久化存储
        let persistent_store = Arc::new(RwLock::new(PersistentCache {
            repo_info: HashMap::new(),
            releases: HashMap::new(),
            latest_release: HashMap::new(),
        }));

        // 创建缓存管理器
        let manager = CacheManager {
            config: config.clone(),
            repo_info_cache: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(ttl)
                .build(),
            releases_cache: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(ttl)
                .build(),
            latest_release_cache: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(ttl)
                .build(),
            file_cache: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(ttl)
                .build(),
            persistent_store: persistent_store.clone(),
            cache_file_path: cache_file_path.clone(),
            file_cache_dir: file_cache_dir.clone(),
            file_path_to_key: Arc::new(RwLock::new(HashMap::new())),
        };

        if config.enabled {
            log::info!("缓存已启用，TTL: {} 秒", config.ttl_seconds);
            
            // 从磁盘加载缓存
            manager.load_from_disk().await;
            
            // 启动后台保存任务（每30秒保存一次）
            let manager_clone = manager.clone_for_background();
            tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    manager_clone.save_to_disk().await;
                }
            });
        } else {
            log::info!("缓存已禁用");
        }

        manager
    }

    // 克隆用于后台任务
    fn clone_for_background(&self) -> BackgroundCacheManager {
        BackgroundCacheManager {
            persistent_store: self.persistent_store.clone(),
            cache_file_path: self.cache_file_path.clone(),
            config: self.config.clone(),
        }
    }

    // 从磁盘加载缓存
    async fn load_from_disk(&self) {
        if !self.config.enabled {
            return;
        }

        match std::fs::read_to_string(&self.cache_file_path) {
            Ok(content) => {
                match serde_json::from_str::<PersistentCache>(&content) {
                    Ok(persistent_cache) => {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        
                        let mut loaded_count = 0;
                        let mut store = self.persistent_store.write().await;

                        // 加载 repo_info 缓存
                        for (key, entry) in persistent_cache.repo_info.iter() {
                            if entry.expires_at > now {
                                // 计算剩余 TTL
                                let remaining_ttl = entry.expires_at - now;
                                if remaining_ttl > 0 {
                                    self.repo_info_cache
                                        .insert(key.clone(), entry.value.clone())
                                        .await;
                                    store.repo_info.insert(key.clone(), entry.clone());
                                    loaded_count += 1;
                                }
                            }
                        }

                        // 加载 releases 缓存
                        for (key, entry) in persistent_cache.releases.iter() {
                            if entry.expires_at > now {
                                let remaining_ttl = entry.expires_at - now;
                                if remaining_ttl > 0 {
                                    self.releases_cache
                                        .insert(key.clone(), entry.value.clone())
                                        .await;
                                    store.releases.insert(key.clone(), entry.clone());
                                    loaded_count += 1;
                                }
                            }
                        }

                        // 加载 latest_release 缓存
                        for (key, entry) in persistent_cache.latest_release.iter() {
                            if entry.expires_at > now {
                                let remaining_ttl = entry.expires_at - now;
                                if remaining_ttl > 0 {
                                    self.latest_release_cache
                                        .insert(key.clone(), entry.value.clone())
                                        .await;
                                    store.latest_release.insert(key.clone(), entry.clone());
                                    loaded_count += 1;
                                }
                            }
                        }

                        log::info!("从磁盘加载了 {} 个缓存条目", loaded_count);
                    }
                    Err(e) => {
                        log::warn!("无法解析缓存文件: {}", e);
                    }
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("无法读取缓存文件: {}", e);
                }
            }
        }
    }

    // 保存缓存到磁盘（保留用于可能的手动调用）
    #[allow(dead_code)]
    async fn save_to_disk(&self) {
        if !self.config.enabled {
            return;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let store = self.persistent_store.read().await;
        
        // 过滤掉已过期的条目
        let persistent_cache = PersistentCache {
            repo_info: store
                .repo_info
                .iter()
                .filter(|(_, entry)| entry.expires_at > now)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            releases: store
                .releases
                .iter()
                .filter(|(_, entry)| entry.expires_at > now)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            latest_release: store
                .latest_release
                .iter()
                .filter(|(_, entry)| entry.expires_at > now)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        };

        match serde_json::to_string_pretty(&persistent_cache) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.cache_file_path, json) {
                    log::warn!("无法保存缓存文件: {}", e);
                }
            }
            Err(e) => {
                log::warn!("无法序列化缓存: {}", e);
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    // 生成缓存键
    fn repo_info_key(owner: &str, repo: &str) -> CacheKey {
        format!("repo_info:{}:{}", owner, repo)
    }

    fn releases_key(owner: &str, repo: &str) -> CacheKey {
        format!("releases:{}:{}", owner, repo)
    }

    fn latest_release_key(owner: &str, repo: &str) -> CacheKey {
        format!("latest_release:{}:{}", owner, repo)
    }

    // 获取仓库信息（带缓存）
    pub async fn get_repo_info(&self, owner: &str, repo: &str) -> Option<RepoInfo> {
        if !self.is_enabled() {
            return None;
        }
        let key = Self::repo_info_key(owner, repo);
        self.repo_info_cache.get(&key).await
    }

    // 存储仓库信息到缓存
    pub async fn set_repo_info(&self, owner: &str, repo: &str, info: RepoInfo) {
        if self.is_enabled() {
            let key = Self::repo_info_key(owner, repo);
            self.repo_info_cache.insert(key.clone(), info.clone()).await;
            
            // 更新持久化存储
            let expires_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + self.config.ttl_seconds;
            
            let mut store = self.persistent_store.write().await;
            store.repo_info.insert(key, CachedEntry {
                value: info,
                expires_at,
            });
        }
    }

    // 获取 releases（带缓存）
    pub async fn get_releases(&self, owner: &str, repo: &str) -> Option<Vec<ReleaseInfo>> {
        if !self.is_enabled() {
            return None;
        }
        let key = Self::releases_key(owner, repo);
        self.releases_cache.get(&key).await
    }

    // 存储 releases 到缓存
    pub async fn set_releases(&self, owner: &str, repo: &str, releases: Vec<ReleaseInfo>) {
        if self.is_enabled() {
            let key = Self::releases_key(owner, repo);
            self.releases_cache.insert(key.clone(), releases.clone()).await;
            
            // 更新持久化存储
            let expires_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + self.config.ttl_seconds;
            
            let mut store = self.persistent_store.write().await;
            store.releases.insert(key, CachedEntry {
                value: releases,
                expires_at,
            });
        }
    }

    // 获取最新 release（带缓存）
    pub async fn get_latest_release(&self, owner: &str, repo: &str) -> Option<LatestReleaseInfo> {
        if !self.is_enabled() {
            return None;
        }
        let key = Self::latest_release_key(owner, repo);
        self.latest_release_cache.get(&key).await
    }

    // 存储最新 release 到缓存
    pub async fn set_latest_release(&self, owner: &str, repo: &str, release: LatestReleaseInfo) {
        if self.is_enabled() {
            let key = Self::latest_release_key(owner, repo);
            self.latest_release_cache.insert(key.clone(), release.clone()).await;
            
            // 更新持久化存储
            let expires_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + self.config.ttl_seconds;
            
            let mut store = self.persistent_store.write().await;
            store.latest_release.insert(key, CachedEntry {
                value: release,
                expires_at,
            });
        }
    }

    // 生成文件缓存键（基于URL的hash）
    fn file_cache_key(url: &str) -> CacheKey {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("file:{}", hex::encode(hasher.finalize()))
    }

    // 获取文件缓存元数据
    pub async fn get_file_cache(&self, url: &str) -> Option<FileCacheMetadata> {
        if !self.is_enabled() {
            return None;
        }
        let key = Self::file_cache_key(url);
        if let Some(mut metadata) = self.file_cache.get(&key).await {
            // 检查文件是否仍然存在
            if metadata.file_path.exists() {
                // 检查是否过期
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if metadata.expires_at > now {
                    // 更新访问时间
                    metadata.last_accessed_at = now;
                    // 更新缓存中的访问时间
                    let key_clone = key.clone();
                    let metadata_clone = metadata.clone();
                    self.file_cache.insert(key_clone, metadata_clone).await;
                    return Some(metadata);
                }
            }
        }
        None
    }

    // 保存文件到缓存
    pub async fn set_file_cache(
        &self,
        url: &str,
        file_path: PathBuf,
        original_filename: String,
        content_type: Option<String>,
    ) {
        if self.is_enabled() {
            let key = Self::file_cache_key(url);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let expires_at = now + self.config.ttl_seconds;
            
            let metadata = FileCacheMetadata {
                url: url.to_string(),
                file_path: file_path.clone(),
                original_filename,
                content_type,
                expires_at,
                last_accessed_at: now, // 设置初始访问时间为当前时间
            };
            
            self.file_cache.insert(key.clone(), metadata.clone()).await;
            
            // 更新文件路径到缓存键的映射
            let mut mapping = self.file_path_to_key.write().await;
            mapping.insert(file_path.clone(), key);
            drop(mapping);
            
            log::debug!("文件已缓存: {} -> {:?}", url, file_path);
            
            // 清理旧文件，保留最常访问的50个
            self.cleanup_file_cache(50).await;
        }
    }

    // 获取文件缓存目录
    pub fn get_file_cache_dir(&self) -> &PathBuf {
        &self.file_cache_dir
    }

    // 清理文件缓存，使用 LRV (Least Recently Visited) 算法保留最常访问的 N 个文件
    pub async fn cleanup_file_cache(&self, max_files: usize) {
        if !self.is_enabled() {
            return;
        }

        // 收集所有有效的文件缓存元数据
        let mut file_metadatas: Vec<(PathBuf, FileCacheMetadata)> = Vec::new();
        let mapping = self.file_path_to_key.read().await;

        // 扫描文件缓存目录，收集所有文件的元数据
        match std::fs::read_dir(&self.file_cache_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    if file_path.is_file() {
                        // 通过文件路径查找对应的缓存键
                        if let Some(cache_key) = mapping.get(&file_path) {
                            // 从缓存中获取元数据
                            if let Some(metadata) = self.file_cache.get(cache_key).await {
                                // 检查文件是否仍然存在且未过期
                                let now = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();
                                if metadata.file_path.exists() && metadata.expires_at > now {
                                    file_metadatas.push((file_path.clone(), metadata));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("无法读取文件缓存目录: {}", e);
                return;
            }
        }

        drop(mapping);

        // 按访问时间排序（最近访问的在前）
        file_metadatas.sort_by(|a, b| b.1.last_accessed_at.cmp(&a.1.last_accessed_at));

        // 如果文件数量超过限制，删除最旧的文件
        if file_metadatas.len() > max_files {
            let files_to_delete = &file_metadatas[max_files..];
            let mut deleted_count = 0;
            let mut mapping = self.file_path_to_key.write().await;
            
            for (file_path, metadata) in files_to_delete {
                // 删除文件
                if let Err(e) = std::fs::remove_file(file_path) {
                    log::warn!("无法删除缓存文件 {:?}: {}", file_path, e);
                } else {
                    deleted_count += 1;
                    log::debug!("已删除缓存文件: {:?} (URL: {})", file_path, metadata.url);
                    
                    // 从映射中删除
                    mapping.remove(file_path);
                    
                    // 从缓存中删除（通过缓存键）
                    let cache_key = Self::file_cache_key(&metadata.url);
                    self.file_cache.invalidate(&cache_key).await;
                }
            }
            
            log::info!("文件缓存清理完成: 保留 {} 个文件，删除 {} 个文件", max_files, deleted_count);
        }
    }
}

// 后台任务使用的缓存管理器（只用于保存）
struct BackgroundCacheManager {
    persistent_store: Arc<RwLock<PersistentCache>>,
    cache_file_path: PathBuf,
    config: CacheConfig,
}

impl BackgroundCacheManager {
    async fn save_to_disk(&self) {
        if !self.config.enabled {
            return;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 从内存缓存同步到持久化存储
        // 注意：moka 不提供遍历方法，所以我们只能保存持久化存储中的内容
        let store = self.persistent_store.read().await;
        
        // 过滤掉已过期的条目
        let persistent_cache = PersistentCache {
            repo_info: store
                .repo_info
                .iter()
                .filter(|(_, entry)| entry.expires_at > now)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            releases: store
                .releases
                .iter()
                .filter(|(_, entry)| entry.expires_at > now)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            latest_release: store
                .latest_release
                .iter()
                .filter(|(_, entry)| entry.expires_at > now)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        };

        match serde_json::to_string_pretty(&persistent_cache) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.cache_file_path, json) {
                    log::warn!("无法保存缓存文件: {}", e);
                }
            }
            Err(e) => {
                log::warn!("无法序列化缓存: {}", e);
            }
        }
    }
}

// 全局缓存管理器（使用 OnceCell）
use tokio::sync::OnceCell as AsyncOnceCell;

static CACHE_MANAGER: AsyncOnceCell<CacheManager> = AsyncOnceCell::const_new();

pub async fn get_cache_manager() -> &'static CacheManager {
    CACHE_MANAGER
        .get_or_init(|| async {
            let config = CacheConfig::from_env();
            CacheManager::new(config).await
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LatestReleaseInfo, ReleaseInfo, RepoInfo};

    fn create_test_cache_config(enabled: bool, ttl_seconds: u64) -> CacheConfig {
        CacheConfig {
            enabled,
            ttl_seconds,
        }
    }

    fn create_test_repo_info() -> RepoInfo {
        RepoInfo {
            repo: "test/test".to_string(),
            name: "test".to_string(),
            full_name: "test/test".to_string(),
            html_url: "https://github.com/test/test".to_string(),
            description: Some("Test repo".to_string()),
            stargazers_count: 100,
            forks_count: 50,
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn create_test_release_info() -> ReleaseInfo {
        ReleaseInfo {
            tag_name: "v1.0.0".to_string(),
            name: Some("Release 1.0.0".to_string()),
            changelog: Some("Changelog".to_string()),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            attachments: vec![(
                "file.zip".to_string(),
                "https://example.com/file.zip".to_string(),
            )],
        }
    }

    fn create_test_latest_release_info() -> LatestReleaseInfo {
        LatestReleaseInfo {
            repo: "test/test".to_string(),
            latest_version: "v1.0.0".to_string(),
            changelog: Some("Changelog".to_string()),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            attachments: vec![(
                "file.zip".to_string(),
                "https://example.com/file.zip".to_string(),
            )],
        }
    }

    #[tokio::test]
    async fn test_cache_manager_enabled() {
        let config = create_test_cache_config(true, 3600);
        let manager = CacheManager::new(config).await;
        assert!(manager.is_enabled());
    }

    #[tokio::test]
    async fn test_cache_manager_disabled() {
        let config = create_test_cache_config(false, 3600);
        let manager = CacheManager::new(config).await;
        assert!(!manager.is_enabled());
    }

    #[tokio::test]
    async fn test_repo_info_cache() {
        let config = create_test_cache_config(true, 3600);
        let manager = CacheManager::new(config).await;
        let repo_info = create_test_repo_info();

        // 测试缓存未命中
        assert!(manager.get_repo_info("test", "test").await.is_none());

        // 存储到缓存
        manager
            .set_repo_info("test", "test", repo_info.clone())
            .await;

        // 测试缓存命中
        let cached = manager.get_repo_info("test", "test").await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().repo, repo_info.repo);
    }

    #[tokio::test]
    async fn test_releases_cache() {
        let config = create_test_cache_config(true, 3600);
        let manager = CacheManager::new(config).await;
        let releases = vec![create_test_release_info()];

        // 测试缓存未命中
        assert!(manager.get_releases("test", "test").await.is_none());

        // 存储到缓存
        manager.set_releases("test", "test", releases.clone()).await;

        // 测试缓存命中
        let cached = manager.get_releases("test", "test").await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_latest_release_cache() {
        let config = create_test_cache_config(true, 3600);
        let manager = CacheManager::new(config).await;
        let latest_release = create_test_latest_release_info();

        // 测试缓存未命中
        assert!(manager.get_latest_release("test", "test").await.is_none());

        // 存储到缓存
        manager
            .set_latest_release("test", "test", latest_release.clone())
            .await;

        // 测试缓存命中
        let cached = manager.get_latest_release("test", "test").await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().repo, latest_release.repo);
    }

    #[tokio::test]
    async fn test_cache_disabled() {
        let config = create_test_cache_config(false, 3600);
        let manager = CacheManager::new(config).await;
        let repo_info = create_test_repo_info();

        // 即使存储，缓存被禁用时也不应该返回
        manager.set_repo_info("test", "test", repo_info).await;
        assert!(manager.get_repo_info("test", "test").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_key_generation() {
        let repo_info_key = CacheManager::repo_info_key("owner", "repo");
        assert_eq!(repo_info_key, "repo_info:owner:repo");

        let releases_key = CacheManager::releases_key("owner", "repo");
        assert_eq!(releases_key, "releases:owner:repo");

        let latest_release_key = CacheManager::latest_release_key("owner", "repo");
        assert_eq!(latest_release_key, "latest_release:owner:repo");
    }
}
