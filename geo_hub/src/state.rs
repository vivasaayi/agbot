use crate::{config::HubConfig, db::DbPool, landsat::LandsatSceneCandidate};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub config: Arc<HubConfig>,
    pub scene_search_cache: SceneSearchCache,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SceneSearchCacheKey {
    pub source: String,
    pub latitude_e6: i64,
    pub longitude_e6: i64,
    pub target_date: String,
    pub days: u8,
    pub limit: usize,
}

impl SceneSearchCacheKey {
    pub fn new(
        source: &str,
        latitude: f64,
        longitude: f64,
        target_date: &str,
        days: u8,
        limit: usize,
    ) -> Self {
        Self {
            source: source.trim().to_lowercase(),
            latitude_e6: (latitude * 1_000_000.0).round() as i64,
            longitude_e6: (longitude * 1_000_000.0).round() as i64,
            target_date: target_date.to_string(),
            days,
            limit,
        }
    }
}

#[derive(Clone, Default)]
pub struct SceneSearchCache {
    entries: Arc<Mutex<HashMap<SceneSearchCacheKey, Vec<LandsatSceneCandidate>>>>,
}

impl SceneSearchCache {
    pub fn get(&self, key: &SceneSearchCacheKey) -> Option<Vec<LandsatSceneCandidate>> {
        self.entries.lock().ok()?.get(key).cloned()
    }

    pub fn store(&self, key: SceneSearchCacheKey, scenes: Vec<LandsatSceneCandidate>) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(key, scenes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_search_cache_records_empty_results() {
        let cache = SceneSearchCache::default();
        let key = SceneSearchCacheKey::new("landsat", 41.25, -96.45, "2026-06-01", 14, 5);

        assert!(cache.get(&key).is_none());

        cache.store(key.clone(), Vec::new());

        assert!(cache.get(&key).is_some_and(|found| found.is_empty()));
    }
}
