//! Tile caching system for GIS data
//!
//! Caches downloaded elevation, imagery, and other tile data locally
//! to avoid repeated network requests. Implements LRU eviction for memory
//! and persistent disk caching.

use anyhow::{Context, Result};
use bevy::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::Instant;

use super::TileCoord;

/// Cache configuration and state
#[derive(Resource)]
pub struct TileCache {
    pub cache_dir: PathBuf,
    pub max_memory_tiles: usize,
    /// Max concurrent HTTP requests
    pub max_concurrent_requests: usize,
    memory_cache: HashMap<String, CachedTile>,
    /// LRU tracking - most recently used at back
    lru_order: VecDeque<String>,
    /// Statistics for monitoring
    pub stats: CacheStats,
}

#[derive(Default, Clone, Debug)]
pub struct CacheStats {
    pub memory_hits: u64,
    pub disk_hits: u64,
    pub network_fetches: u64,
    pub evictions: u64,
}

impl Default for TileCache {
    fn default() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("agbot")
            .join("tiles");
        
        Self {
            cache_dir,
            max_memory_tiles: 256,
            max_concurrent_requests: 4,
            memory_cache: HashMap::new(),
            lru_order: VecDeque::new(),
            stats: CacheStats::default(),
        }
    }
}

#[derive(Clone)]
pub struct CachedTile {
    pub data: Vec<u8>,
    pub tile_type: TileType,
    /// When this tile was last accessed
    pub last_access: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileType {
    Elevation,
    Imagery,
    Ndvi,
    CropType,
    Cdl,
    OsmFeatures,
}

impl TileCache {
    /// Get the file path for a cached tile
    pub fn tile_path(&self, coord: &TileCoord, tile_type: TileType) -> PathBuf {
        let type_dir = match tile_type {
            TileType::Elevation => "elevation",
            TileType::Imagery => "imagery",
            TileType::Ndvi => "ndvi",
            TileType::CropType => "croptype",
            TileType::Cdl => "cdl",
            TileType::OsmFeatures => "osm",
        };
        
        self.cache_dir
            .join(type_dir)
            .join(format!("{}", coord.z))
            .join(format!("{}", coord.x))
            .join(format!("{}.bin", coord.y))
    }
    
    /// Check if a tile exists in cache
    pub fn has_tile(&self, coord: &TileCoord, tile_type: TileType) -> bool {
        let key = Self::cache_key(coord, tile_type);
        if self.memory_cache.contains_key(&key) {
            return true;
        }
        self.tile_path(coord, tile_type).exists()
    }
    
    /// Get a tile from cache (memory first, then disk) with LRU tracking
    pub fn get_tile(&mut self, coord: &TileCoord, tile_type: TileType) -> Option<CachedTile> {
        let key = Self::cache_key(coord, tile_type);
        
        // Check memory cache
        if self.memory_cache.contains_key(&key) {
            // Clone first to avoid borrow issues, then update LRU
            let tile = self.memory_cache.get(&key).cloned();
            if let Some(ref t) = tile {
                self.stats.memory_hits += 1;
                // Update LRU order - move to back (most recent)
                self.update_lru_position(&key);
                // Update last access time
                if let Some(cached) = self.memory_cache.get_mut(&key) {
                    cached.last_access = Instant::now();
                }
            }
            return tile;
        }
        
        // Check disk cache
        let path = self.tile_path(coord, tile_type);
        if path.exists() {
            if let Ok(data) = std::fs::read(&path) {
                self.stats.disk_hits += 1;
                let tile = CachedTile { 
                    data, 
                    tile_type,
                    last_access: Instant::now(),
                };
                
                // Promote to memory cache
                self.memory_cache.insert(key.clone(), tile.clone());
                self.lru_order.push_back(key);
                self.evict_lru_if_needed();
                
                return Some(tile);
            }
        }
        
        None
    }
    
    /// Store a tile in cache (both memory and disk)
    pub fn store_tile(&mut self, coord: &TileCoord, tile_type: TileType, data: Vec<u8>) -> Result<()> {
        let key = Self::cache_key(coord, tile_type);
        let path = self.tile_path(coord, tile_type);
        
        // Store to disk
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create cache directory")?;
        }
        std::fs::write(&path, &data)
            .context("Failed to write tile to cache")?;
        
        // Store to memory with LRU tracking
        let tile = CachedTile { 
            data, 
            tile_type,
            last_access: Instant::now(),
        };
        
        // Remove old entry from LRU if updating
        if self.memory_cache.contains_key(&key) {
            self.lru_order.retain(|k| k != &key);
        }
        
        self.memory_cache.insert(key.clone(), tile);
        self.lru_order.push_back(key);
        self.evict_lru_if_needed();
        
        self.stats.network_fetches += 1;
        
        Ok(())
    }
    
    fn cache_key(coord: &TileCoord, tile_type: TileType) -> String {
        format!("{:?}_{}_{}_{}", tile_type, coord.z, coord.x, coord.y)
    }
    
    fn update_lru_position(&mut self, key: &str) {
        // Remove from current position and add to back (most recent)
        self.lru_order.retain(|k| k != key);
        self.lru_order.push_back(key.to_string());
    }
    
    fn evict_lru_if_needed(&mut self) {
        // LRU eviction - remove from front (least recent)
        while self.memory_cache.len() > self.max_memory_tiles {
            if let Some(key) = self.lru_order.pop_front() {
                self.memory_cache.remove(&key);
                self.stats.evictions += 1;
            } else {
                break;
            }
        }
    }
    
    /// Get cache statistics for monitoring
    pub fn get_stats(&self) -> CacheStats {
        self.stats.clone()
    }
    
    /// Clear memory cache (disk cache preserved)
    pub fn clear_memory(&mut self) {
        self.memory_cache.clear();
        self.lru_order.clear();
    }
    
    /// Get number of tiles currently in memory
    pub fn memory_tile_count(&self) -> usize {
        self.memory_cache.len()
    }
}
