use serde::{Deserialize, Serialize};
use rand::Rng;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConditions {
    pub temperature_celsius: f32,
    pub humidity_percent: f32,
    pub pressure_hpa: f32,
    pub wind_speed_ms: f32,
    pub wind_direction_rad: f32,
    pub visibility_m: f32,
    pub precipitation_mm: f32,
    pub cloud_cover_percent: f32,
    pub air_density: f32, // kg/m³
    pub solar_irradiance: f32, // W/m²
}

pub struct Environment {
    conditions: EnvironmentConditions,
    terrain_height_map: Vec<Vec<f32>>,
    map_size_m: f32,
    map_resolution_m: f32,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            conditions: EnvironmentConditions::default(),
            terrain_height_map: Self::generate_terrain(100, 100),
            map_size_m: 1000.0,
            map_resolution_m: 10.0,
        }
    }

    pub fn with_conditions(mut self, conditions: EnvironmentConditions) -> Self {
        self.conditions = conditions;
        self
    }

    pub fn get_conditions(&self) -> &EnvironmentConditions {
        &self.conditions
    }

    pub fn update_conditions(&mut self, conditions: EnvironmentConditions) {
        self.conditions = conditions;
    }

    pub fn get_terrain_height(&self, x: f32, z: f32) -> f32 {
        // Convert world coordinates to map coordinates
        let map_x = ((x + self.map_size_m / 2.0) / self.map_resolution_m) as usize;
        let map_z = ((z + self.map_size_m / 2.0) / self.map_resolution_m) as usize;

        if map_x < self.terrain_height_map.len() && map_z < self.terrain_height_map[0].len() {
            self.terrain_height_map[map_x][map_z]
        } else {
            0.0 // Default ground level
        }
    }

    pub fn is_safe_altitude(&self, x: f32, z: f32, altitude: f32) -> bool {
        let terrain_height = self.get_terrain_height(x, z);
        altitude > terrain_height + 10.0 // 10m safety margin
    }

    pub fn calculate_wind_effect(&self, altitude: f32) -> (f32, f32) {
        // Wind speed typically increases with altitude
        let altitude_factor = (altitude / 100.0).min(2.0); // Cap at 2x at 100m
        let effective_wind_speed = self.conditions.wind_speed_ms * (1.0 + altitude_factor * 0.5);
        
        (effective_wind_speed, self.conditions.wind_direction_rad)
    }

    pub fn get_visibility_at_altitude(&self, altitude: f32) -> f32 {
        // Visibility generally improves with altitude up to a point
        let altitude_factor = if altitude < 100.0 {
            1.0 + altitude / 1000.0 // Slight improvement up to 100m
        } else {
            1.1 // Cap improvement
        };
        
        self.conditions.visibility_m * altitude_factor
    }

    pub fn simulate_weather_change(&mut self) {
        let mut rng = rand::thread_rng();
        
        // Small random changes to simulate dynamic weather
        self.conditions.wind_speed_ms += rng.gen_range(-0.5..0.5);
        self.conditions.wind_speed_ms = self.conditions.wind_speed_ms.max(0.0);
        
        self.conditions.wind_direction_rad += rng.gen_range(-0.1..0.1);
        
        self.conditions.visibility_m += rng.gen_range(-100.0..100.0);
        self.conditions.visibility_m = self.conditions.visibility_m.clamp(100.0, 20000.0);
        
        self.conditions.cloud_cover_percent += rng.gen_range(-2.0..2.0);
        self.conditions.cloud_cover_percent = self.conditions.cloud_cover_percent.clamp(0.0, 100.0);
    }

    fn generate_terrain(width: usize, height: usize) -> Vec<Vec<f32>> {
        let mut terrain = vec![vec![0.0; height]; width];
        let mut rng = rand::thread_rng();
        
        // Generate simple random terrain
        for i in 0..width {
            for j in 0..height {
                // Create some hills and valleys
                let x = i as f32 / width as f32;
                let y = j as f32 / height as f32;
                
                let height_value = 
                    20.0 * (x * 2.0 * std::f32::consts::PI).sin() * 
                    (y * 2.0 * std::f32::consts::PI).cos() +
                    10.0 * rng.gen_range(-1.0..1.0);
                
                terrain[i][j] = height_value.max(0.0);
            }
        }
        
        terrain
    }

    pub fn add_obstacle(&mut self, x: f32, z: f32, height: f32) {
        let map_x = ((x + self.map_size_m / 2.0) / self.map_resolution_m) as usize;
        let map_z = ((z + self.map_size_m / 2.0) / self.map_resolution_m) as usize;

        if map_x < self.terrain_height_map.len() && map_z < self.terrain_height_map[0].len() {
            self.terrain_height_map[map_x][map_z] = height;
        }
    }

    pub fn get_map_bounds(&self) -> (f32, f32) {
        (self.map_size_m, self.map_size_m)
    }
}

impl Default for EnvironmentConditions {
    fn default() -> Self {
        Self {
            temperature_celsius: 20.0,
            humidity_percent: 50.0,
            pressure_hpa: 1013.25,
            wind_speed_ms: 5.0,
            wind_direction_rad: 0.0,
            visibility_m: 10000.0,
            precipitation_mm: 0.0,
            cloud_cover_percent: 25.0,
            air_density: 1.225, // kg/m³ at sea level
            solar_irradiance: 1000.0, // W/m² clear day
        }
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_creation() {
        let env = Environment::new();
        assert_eq!(env.conditions.temperature_celsius, 20.0);
        assert!(env.terrain_height_map.len() > 0);
    }

    #[test]
    fn test_terrain_height() {
        let env = Environment::new();
        let height = env.get_terrain_height(0.0, 0.0);
        assert!(height >= 0.0);
    }

    #[test]
    fn test_wind_effect() {
        let env = Environment::new();
        let (wind_speed, wind_dir) = env.calculate_wind_effect(50.0);
        assert!(wind_speed >= env.conditions.wind_speed_ms);
        assert_eq!(wind_dir, env.conditions.wind_direction_rad);
    }

    #[test]
    fn test_safety_altitude() {
        let env = Environment::new();
        let safe = env.is_safe_altitude(0.0, 0.0, 50.0);
        assert!(safe); // 50m should be safe for most terrain
    }
}
