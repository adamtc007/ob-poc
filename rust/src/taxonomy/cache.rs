//! Caching layer for taxonomy service discovery
//!
//! Provides in-memory caching for frequently accessed service configurations
//! to reduce database load.

use crate::models::taxonomy::{Service, ServiceWithOptions};
use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
struct CachedServices {
    services: Vec<Service>,
    cached_at: Instant,
    ttl_seconds: u64,
}

impl CachedServices {
    fn is_valid(&self) -> bool {
        self.cached_at.elapsed().as_secs() < self.ttl_seconds
    }
}

#[derive(Clone)]
struct CachedServiceWithOptions {
    service: ServiceWithOptions,
    cached_at: Instant,
    ttl_seconds: u64,
}

impl CachedServiceWithOptions {
    fn is_valid(&self) -> bool {
        self.cached_at.elapsed().as_secs() < self.ttl_seconds
    }
}

/// Cache for service discovery operations
pub struct ServiceDiscoveryCache {
    pool: PgPool,
    memory_cache: Arc<RwLock<HashMap<Uuid, CachedServices>>>,
    service_options_cache: Arc<RwLock<HashMap<Uuid, CachedServiceWithOptions>>>,
    default_ttl: Duration,
}

impl ServiceDiscoveryCache {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            service_options_cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Duration::from_secs(300), // 5 minutes default TTL
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    /// Get services for a product, using cache when available
    pub async fn get_services_for_product(&self, product_id: Uuid) -> Result<Vec<Service>> {
        // Check cache first
        {
            let cache = self.memory_cache.read().await;
            if let Some(cached) = cache.get(&product_id) {
                if cached.is_valid() {
                    tracing::debug!("Cache hit for product_id: {}", product_id);
                    return Ok(cached.services.clone());
                }
            }
        }

        // Cache miss - fetch from database
        tracing::debug!("Cache miss for product_id: {}", product_id);
        let services = self.fetch_services_from_db(product_id).await?;

        // Update cache
        self.cache_services(product_id, services.clone()).await;

        Ok(services)
    }

    /// Get service with options, using cache when available
    pub async fn get_service_with_options(&self, service_id: Uuid) -> Result<ServiceWithOptions> {
        // Check cache first
        {
            let cache = self.service_options_cache.read().await;
            if let Some(cached) = cache.get(&service_id) {
                if cached.is_valid() {
                    tracing::debug!("Cache hit for service_id: {}", service_id);
                    return Ok(cached.service.clone());
                }
            }
        }

        // Cache miss - fetch from database
        tracing::debug!("Cache miss for service_id: {}", service_id);
        let service = self.fetch_service_with_options_from_db(service_id).await?;

        // Update cache
        self.cache_service_with_options(service_id, service.clone())
            .await;

        Ok(service)
    }

    /// Invalidate cache for a specific product
    pub async fn invalidate_product(&self, product_id: Uuid) {
        let mut cache = self.memory_cache.write().await;
        cache.remove(&product_id);
        tracing::debug!("Invalidated cache for product_id: {}", product_id);
    }

    /// Invalidate cache for a specific service
    pub async fn invalidate_service(&self, service_id: Uuid) {
        let mut cache = self.service_options_cache.write().await;
        cache.remove(&service_id);
        tracing::debug!("Invalidated cache for service_id: {}", service_id);
    }

    /// Clear all cached data
    pub async fn clear_all(&self) {
        let mut cache = self.memory_cache.write().await;
        cache.clear();
        let mut options_cache = self.service_options_cache.write().await;
        options_cache.clear();
        tracing::info!("Cleared all cache entries");
    }

    async fn fetch_services_from_db(&self, product_id: Uuid) -> Result<Vec<Service>> {
        let services = sqlx::query_as!(
            Service,
            r#"
            SELECT s.* FROM "ob-poc".services s
            JOIN "ob-poc".product_services ps ON s.service_id = ps.service_id
            WHERE ps.product_id = $1 AND s.is_active = true
            ORDER BY ps.display_order, s.name
            "#,
            product_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(services)
    }

    async fn fetch_service_with_options_from_db(
        &self,
        service_id: Uuid,
    ) -> Result<ServiceWithOptions> {
        use crate::database::TaxonomyRepository;

        let repo = TaxonomyRepository::new(self.pool.clone());
        repo.get_service_with_options(service_id).await
    }

    async fn cache_services(&self, product_id: Uuid, services: Vec<Service>) {
        let mut cache = self.memory_cache.write().await;
        cache.insert(
            product_id,
            CachedServices {
                services,
                cached_at: Instant::now(),
                ttl_seconds: self.default_ttl.as_secs(),
            },
        );
    }

    async fn cache_service_with_options(&self, service_id: Uuid, service: ServiceWithOptions) {
        let mut cache = self.service_options_cache.write().await;
        cache.insert(
            service_id,
            CachedServiceWithOptions {
                service,
                cached_at: Instant::now(),
                ttl_seconds: self.default_ttl.as_secs(),
            },
        );
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let cache = self.memory_cache.read().await;
        let options_cache = self.service_options_cache.read().await;

        CacheStats {
            product_cache_size: cache.len(),
            service_cache_size: options_cache.len(),
            ttl_seconds: self.default_ttl.as_secs(),
        }
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub product_cache_size: usize,
    pub service_cache_size: usize,
    pub ttl_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_services_validity() {
        let cached = CachedServices {
            services: vec![],
            cached_at: Instant::now(),
            ttl_seconds: 300,
        };

        assert!(cached.is_valid());

        let expired = CachedServices {
            services: vec![],
            cached_at: Instant::now() - Duration::from_secs(400),
            ttl_seconds: 300,
        };

        assert!(!expired.is_valid());
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let pool = PgPool::connect_lazy("postgresql://test").unwrap();
        let cache = ServiceDiscoveryCache::new(pool);

        let product_id = Uuid::new_v4();

        // Manually insert into cache
        cache.cache_services(product_id, vec![]).await;

        // Verify it's there
        {
            let cache_map = cache.memory_cache.read().await;
            assert!(cache_map.contains_key(&product_id));
        }

        // Invalidate
        cache.invalidate_product(product_id).await;

        // Verify it's gone
        {
            let cache_map = cache.memory_cache.read().await;
            assert!(!cache_map.contains_key(&product_id));
        }
    }
}
