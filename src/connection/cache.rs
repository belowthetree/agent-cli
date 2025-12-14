use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use crate::connection::CommonConnectionContent;

/// 缓存条目
#[derive(Clone)]
struct CacheEntry {
    response: Vec<CommonConnectionContent>,
    expires_at: Instant,
}

/// 响应缓存
#[derive(Clone)]
pub struct ResponseCache {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    default_ttl: Duration,
}

impl ResponseCache {
    /// 创建新的响应缓存
    pub fn new(default_ttl_seconds: u64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Duration::from_secs(default_ttl_seconds),
        }
    }

    /// 从缓存中获取响应
    pub async fn get(&self, key: &str) -> Option<Vec<CommonConnectionContent>> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(key) {
            if entry.expires_at > Instant::now() {
                return Some(entry.response.clone());
            }
        }
        None
    }

    /// 将响应存入缓存
    pub async fn set(&self, key: String, response: Vec<CommonConnectionContent>) {
        self.set_with_ttl(key, response, self.default_ttl).await;
    }

    /// 使用自定义TTL将响应存入缓存
    pub async fn set_with_ttl(&self, key: String, response: Vec<CommonConnectionContent>, ttl: Duration) {
        let mut cache = self.cache.write().await;
        let entry = CacheEntry {
            response,
            expires_at: Instant::now() + ttl,
        };
        cache.insert(key, entry);
    }

    /// 清理过期的缓存条目
    pub async fn cleanup(&self) {
        let mut cache = self.cache.write().await;
        let now = Instant::now();
        cache.retain(|_, entry| entry.expires_at > now);
    }

    /// 清除所有缓存
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// 获取缓存大小
    pub async fn size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
}

/// 生成缓存键
pub fn generate_cache_key(url: &str, body: &str) -> String {
    // 使用简单的哈希作为缓存键
    format!("{}:{:x}", url, md5::compute(body))
}

/// 全局响应缓存
static RESPONSE_CACHE: once_cell::sync::Lazy<ResponseCache> = once_cell::sync::Lazy::new(|| {
    ResponseCache::new(300) // 默认5分钟TTL
});

/// 获取全局响应缓存
pub fn get_response_cache() -> &'static ResponseCache {
    &RESPONSE_CACHE
}
