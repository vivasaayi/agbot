use bevy::prelude::*;
use crate::world_exploration::WorldLocation;

pub struct LocationDatabasePlugin;

impl Plugin for LocationDatabasePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CityDatabase>();
    }
}

#[derive(Resource)]
pub struct CityDatabase {
    cities: Vec<WorldLocation>,
}

impl Default for CityDatabase {
    fn default() -> Self {
        Self {
            cities: create_default_cities(),
        }
    }
}

impl CityDatabase {
    /// Search for cities matching the query string
    pub fn search_cities(&self, query: &str) -> Vec<WorldLocation> {
        if query.is_empty() {
            return Vec::new();
        }
        
        let query_lower = query.to_lowercase();
        let mut results: Vec<WorldLocation> = self.cities
            .iter()
            .filter(|city| {
                city.name.to_lowercase().contains(&query_lower) ||
                city.country.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect();
        
        // Sort by relevance (exact matches first, then partial matches)
        results.sort_by(|a, b| {
            let a_exact = a.name.to_lowercase() == query_lower;
            let b_exact = b.name.to_lowercase() == query_lower;
            
            match (a_exact, b_exact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        
        // Limit results to top 10
        results.truncate(10);
        results
    }
    
    /// Get autocomplete suggestions
    pub fn get_suggestions(&self, query: &str) -> Vec<String> {
        let results = self.search_cities(query);
        results.into_iter().map(|city| city.name).collect()
    }
    
    /// Find exact city by name
    pub fn find_city(&self, name: &str) -> Option<WorldLocation> {
        self.cities.iter()
            .find(|city| city.name.to_lowercase() == name.to_lowercase())
            .cloned()
    }
}

/// Create a comprehensive list of major world cities
fn create_default_cities() -> Vec<WorldLocation> {
    vec![
        // Major world cities
        WorldLocation {
            name: "Paris".to_string(),
            latitude: 48.8566,
            longitude: 2.3522,
            country: "France".to_string(),
        },
        WorldLocation {
            name: "London".to_string(),
            latitude: 51.5074,
            longitude: -0.1278,
            country: "United Kingdom".to_string(),
        },
        WorldLocation {
            name: "New York".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            country: "United States".to_string(),
        },
        WorldLocation {
            name: "Tokyo".to_string(),
            latitude: 35.6762,
            longitude: 139.6503,
            country: "Japan".to_string(),
        },
        WorldLocation {
            name: "Berlin".to_string(),
            latitude: 52.5200,
            longitude: 13.4050,
            country: "Germany".to_string(),
        },
        WorldLocation {
            name: "Sydney".to_string(),
            latitude: -33.8688,
            longitude: 151.2093,
            country: "Australia".to_string(),
        },
        WorldLocation {
            name: "São Paulo".to_string(),
            latitude: -23.5505,
            longitude: -46.6333,
            country: "Brazil".to_string(),
        },
        WorldLocation {
            name: "Mumbai".to_string(),
            latitude: 19.0760,
            longitude: 72.8777,
            country: "India".to_string(),
        },
        WorldLocation {
            name: "Cairo".to_string(),
            latitude: 30.0444,
            longitude: 31.2357,
            country: "Egypt".to_string(),
        },
        WorldLocation {
            name: "Moscow".to_string(),
            latitude: 55.7558,
            longitude: 37.6176,
            country: "Russia".to_string(),
        },
        WorldLocation {
            name: "Beijing".to_string(),
            latitude: 39.9042,
            longitude: 116.4074,
            country: "China".to_string(),
        },
        WorldLocation {
            name: "Los Angeles".to_string(),
            latitude: 34.0522,
            longitude: -118.2437,
            country: "United States".to_string(),
        },
        WorldLocation {
            name: "Barcelona".to_string(),
            latitude: 41.3851,
            longitude: 2.1734,
            country: "Spain".to_string(),
        },
        WorldLocation {
            name: "Rome".to_string(),
            latitude: 41.9028,
            longitude: 12.4964,
            country: "Italy".to_string(),
        },
        WorldLocation {
            name: "Amsterdam".to_string(),
            latitude: 52.3676,
            longitude: 4.9041,
            country: "Netherlands".to_string(),
        },
        WorldLocation {
            name: "Singapore".to_string(),
            latitude: 1.3521,
            longitude: 103.8198,
            country: "Singapore".to_string(),
        },
        WorldLocation {
            name: "Dubai".to_string(),
            latitude: 25.2048,
            longitude: 55.2708,
            country: "United Arab Emirates".to_string(),
        },
        WorldLocation {
            name: "Toronto".to_string(),
            latitude: 43.6532,
            longitude: -79.3832,
            country: "Canada".to_string(),
        },
        WorldLocation {
            name: "Buenos Aires".to_string(),
            latitude: -34.6118,
            longitude: -58.3960,
            country: "Argentina".to_string(),
        },
        WorldLocation {
            name: "Cape Town".to_string(),
            latitude: -33.9249,
            longitude: 18.4241,
            country: "South Africa".to_string(),
        },
        // US Cities
        WorldLocation {
            name: "San Francisco".to_string(),
            latitude: 37.7749,
            longitude: -122.4194,
            country: "United States".to_string(),
        },
        WorldLocation {
            name: "Chicago".to_string(),
            latitude: 41.8781,
            longitude: -87.6298,
            country: "United States".to_string(),
        },
        WorldLocation {
            name: "Miami".to_string(),
            latitude: 25.7617,
            longitude: -80.1918,
            country: "United States".to_string(),
        },
        WorldLocation {
            name: "Seattle".to_string(),
            latitude: 47.6062,
            longitude: -122.3321,
            country: "United States".to_string(),
        },
        // More international cities
        WorldLocation {
            name: "Bangkok".to_string(),
            latitude: 13.7563,
            longitude: 100.5018,
            country: "Thailand".to_string(),
        },
        WorldLocation {
            name: "Istanbul".to_string(),
            latitude: 41.0082,
            longitude: 28.9784,
            country: "Turkey".to_string(),
        },
        WorldLocation {
            name: "Lagos".to_string(),
            latitude: 6.5244,
            longitude: 3.3792,
            country: "Nigeria".to_string(),
        },
        WorldLocation {
            name: "Mexico City".to_string(),
            latitude: 19.4326,
            longitude: -99.1332,
            country: "Mexico".to_string(),
        },
        WorldLocation {
            name: "Seoul".to_string(),
            latitude: 37.5665,
            longitude: 126.9780,
            country: "South Korea".to_string(),
        },
        WorldLocation {
            name: "Jakarta".to_string(),
            latitude: -6.2088,
            longitude: 106.8456,
            country: "Indonesia".to_string(),
        },
    ]
}
