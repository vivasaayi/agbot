use std::collections::HashMap;
use bevy::prelude::*;

#[derive(Debug, Clone)]
pub struct Location {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub zoom_level: f32, // Suggested zoom level for this location
}

#[derive(Debug, Clone, Default, Resource)]
pub struct LocationDatabase {
    locations: HashMap<String, Location>,
    search_keys: Vec<String>, // For autocomplete
}

impl LocationDatabase {
    pub fn new() -> Self {
        let mut db = Self::default();
        db.populate_default_locations();
        db
    }

    fn populate_default_locations(&mut self) {
        // Major cities around the world
        let locations = vec![
            // Europe
            Location { name: "London, UK".to_string(), lat: 51.5074, lon: -0.1278, zoom_level: 8.0 },
            Location { name: "Paris, France".to_string(), lat: 48.8566, lon: 2.3522, zoom_level: 8.0 },
            Location { name: "Berlin, Germany".to_string(), lat: 52.5200, lon: 13.4050, zoom_level: 8.0 },
            Location { name: "Rome, Italy".to_string(), lat: 41.9028, lon: 12.4964, zoom_level: 8.0 },
            Location { name: "Madrid, Spain".to_string(), lat: 40.4168, lon: -3.7038, zoom_level: 8.0 },
            
            // North America
            Location { name: "New York, USA".to_string(), lat: 40.7128, lon: -74.0060, zoom_level: 8.0 },
            Location { name: "San Francisco, USA".to_string(), lat: 37.7749, lon: -122.4194, zoom_level: 8.0 },
            Location { name: "Los Angeles, USA".to_string(), lat: 34.0522, lon: -118.2437, zoom_level: 8.0 },
            Location { name: "Chicago, USA".to_string(), lat: 41.8781, lon: -87.6298, zoom_level: 8.0 },
            Location { name: "Toronto, Canada".to_string(), lat: 43.6532, lon: -79.3832, zoom_level: 8.0 },
            Location { name: "Mexico City, Mexico".to_string(), lat: 19.4326, lon: -99.1332, zoom_level: 8.0 },
            
            // Asia
            Location { name: "Tokyo, Japan".to_string(), lat: 35.6762, lon: 139.6503, zoom_level: 8.0 },
            Location { name: "Seoul, South Korea".to_string(), lat: 37.5665, lon: 126.9780, zoom_level: 8.0 },
            Location { name: "Beijing, China".to_string(), lat: 39.9042, lon: 116.4074, zoom_level: 8.0 },
            Location { name: "Shanghai, China".to_string(), lat: 31.2304, lon: 121.4737, zoom_level: 8.0 },
            Location { name: "Hong Kong".to_string(), lat: 22.3193, lon: 114.1694, zoom_level: 9.0 },
            Location { name: "Singapore".to_string(), lat: 1.3521, lon: 103.8198, zoom_level: 9.0 },
            Location { name: "Mumbai, India".to_string(), lat: 19.0760, lon: 72.8777, zoom_level: 8.0 },
            Location { name: "Delhi, India".to_string(), lat: 28.7041, lon: 77.1025, zoom_level: 8.0 },
            
            // Middle East & Africa
            Location { name: "Dubai, UAE".to_string(), lat: 25.2048, lon: 55.2708, zoom_level: 8.0 },
            Location { name: "Cairo, Egypt".to_string(), lat: 30.0444, lon: 31.2357, zoom_level: 8.0 },
            Location { name: "Cape Town, South Africa".to_string(), lat: -33.9249, lon: 18.4241, zoom_level: 8.0 },
            
            // Oceania
            Location { name: "Sydney, Australia".to_string(), lat: -33.8688, lon: 151.2093, zoom_level: 8.0 },
            Location { name: "Melbourne, Australia".to_string(), lat: -37.8136, lon: 144.9631, zoom_level: 8.0 },
            Location { name: "Auckland, New Zealand".to_string(), lat: -36.8485, lon: 174.7633, zoom_level: 8.0 },
            
            // South America
            Location { name: "São Paulo, Brazil".to_string(), lat: -23.5505, lon: -46.6333, zoom_level: 8.0 },
            Location { name: "Rio de Janeiro, Brazil".to_string(), lat: -22.9068, lon: -43.1729, zoom_level: 8.0 },
            Location { name: "Buenos Aires, Argentina".to_string(), lat: -34.6118, lon: -58.3960, zoom_level: 8.0 },
            
            // Special locations
            Location { name: "Mount Everest".to_string(), lat: 27.9881, lon: 86.9250, zoom_level: 12.0 },
            Location { name: "Grand Canyon, USA".to_string(), lat: 36.1069, lon: -112.1129, zoom_level: 10.0 },
            Location { name: "Niagara Falls".to_string(), lat: 43.0962, lon: -79.0377, zoom_level: 12.0 },
            Location { name: "Eiffel Tower, Paris".to_string(), lat: 48.8584, lon: 2.2945, zoom_level: 15.0 },
            Location { name: "Statue of Liberty, New York".to_string(), lat: 40.6892, lon: -74.0445, zoom_level: 15.0 },
        ];

        for location in locations {
            let key = location.name.to_lowercase();
            self.search_keys.push(key.clone());
            self.locations.insert(key, location);
        }
        
        // Sort search keys for better autocomplete
        self.search_keys.sort();
    }

    pub fn search(&self, query: &str) -> Vec<&Location> {
        let query_lower = query.to_lowercase();
        
        if query_lower.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        
        // Exact matches first
        if let Some(location) = self.locations.get(&query_lower) {
            results.push(location);
            return results;
        }
        
        // Partial matches
        for (key, location) in &self.locations {
            if key.contains(&query_lower) {
                results.push(location);
            }
        }
        
        // Sort by relevance (shorter names first, exact word matches prioritized)
        results.sort_by(|a, b| {
            let a_name_lower = a.name.to_lowercase();
            let b_name_lower = b.name.to_lowercase();
            let a_words: Vec<&str> = a_name_lower.split_whitespace().collect();
            let b_words: Vec<&str> = b_name_lower.split_whitespace().collect();
            
            let a_exact_word = a_words.iter().any(|word| word.starts_with(&query_lower));
            let b_exact_word = b_words.iter().any(|word| word.starts_with(&query_lower));
            
            match (a_exact_word, b_exact_word) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.len().cmp(&b.name.len()),
            }
        });
        
        // Limit results to prevent UI overflow
        results.truncate(8);
        results
    }

    pub fn get_suggestions(&self, query: &str) -> Vec<String> {
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut suggestions = Vec::new();
        
        for key in &self.search_keys {
            if key.starts_with(&query_lower) {
                if let Some(location) = self.locations.get(key) {
                    suggestions.push(location.name.clone());
                }
            }
        }
        
        suggestions.truncate(5);
        suggestions
    }

    pub fn get_location(&self, name: &str) -> Option<&Location> {
        self.locations.get(&name.to_lowercase())
    }

    pub fn get_all_locations(&self) -> Vec<&Location> {
        self.locations.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location_search() {
        let db = LocationDatabase::new();
        
        // Test exact match
        let results = db.search("london");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "London, UK");
        
        // Test partial match
        let results = db.search("new");
        assert!(results.len() > 0);
        assert!(results.iter().any(|loc| loc.name.contains("New")));
        
        // Test case insensitivity
        let results = db.search("TOKYO");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Tokyo, Japan");
    }

    #[test]
    fn test_suggestions() {
        let db = LocationDatabase::new();
        
        let suggestions = db.get_suggestions("lo");
        assert!(suggestions.iter().any(|s| s.contains("London")));
        assert!(suggestions.iter().any(|s| s.contains("Los Angeles")));
    }
}
