use anyhow::Result;
use serde::{Deserialize, Serialize};
use reqwest;
use crate::WeatherConstraints;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherData {
    pub temperature_celsius: f32,
    pub humidity_percent: f32,
    pub wind_speed_ms: f32,
    pub wind_direction_degrees: f32,
    pub precipitation_mm: f32,
    pub visibility_m: f32,
    pub pressure_hpa: f32,
    pub cloud_cover_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherForecast {
    pub current: WeatherData,
    pub hourly: Vec<WeatherData>,
    pub alerts: Vec<WeatherAlert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherAlert {
    pub severity: AlertSeverity,
    pub message: String,
    pub valid_until: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

pub struct WeatherIntegration {
    api_key: Option<String>,
    client: reqwest::Client,
}

impl WeatherIntegration {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_current_weather(&self, lat: f64, lon: f64) -> Result<WeatherData> {
        // In a real implementation, this would call a weather API
        // For simulation, return mock data
        Ok(self.generate_mock_weather())
    }

    pub async fn get_forecast(&self, lat: f64, lon: f64, hours: u8) -> Result<WeatherForecast> {
        let current = self.get_current_weather(lat, lon).await?;
        
        // Generate hourly forecast
        let hourly = (0..hours)
            .map(|_| self.generate_mock_weather())
            .collect();

        // Check for weather alerts
        let alerts = self.check_weather_alerts(&current);

        Ok(WeatherForecast {
            current,
            hourly,
            alerts,
        })
    }

    pub fn check_flight_conditions(
        &self,
        weather: &WeatherData,
        constraints: &WeatherConstraints,
    ) -> FlightConditionResult {
        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // Check wind speed
        if weather.wind_speed_ms > constraints.max_wind_speed_ms {
            issues.push(format!(
                "Wind speed ({:.1} m/s) exceeds maximum ({:.1} m/s)",
                weather.wind_speed_ms, constraints.max_wind_speed_ms
            ));
        } else if weather.wind_speed_ms > constraints.max_wind_speed_ms * 0.8 {
            warnings.push(format!(
                "Wind speed ({:.1} m/s) approaching maximum",
                weather.wind_speed_ms
            ));
        }

        // Check precipitation
        if weather.precipitation_mm > constraints.max_precipitation_mm {
            issues.push(format!(
                "Precipitation ({:.1} mm) exceeds maximum ({:.1} mm)",
                weather.precipitation_mm, constraints.max_precipitation_mm
            ));
        }

        // Check visibility
        if weather.visibility_m < constraints.min_visibility_m {
            issues.push(format!(
                "Visibility ({:.0} m) below minimum ({:.0} m)",
                weather.visibility_m, constraints.min_visibility_m
            ));
        }

        // Check temperature range
        if weather.temperature_celsius < constraints.temperature_range_celsius.0
            || weather.temperature_celsius > constraints.temperature_range_celsius.1
        {
            issues.push(format!(
                "Temperature ({:.1}°C) outside operating range ({:.1}°C to {:.1}°C)",
                weather.temperature_celsius,
                constraints.temperature_range_celsius.0,
                constraints.temperature_range_celsius.1
            ));
        }

        let flight_safe = issues.is_empty();
        
        FlightConditionResult {
            flight_safe,
            issues,
            warnings,
            weather_score: self.calculate_weather_score(weather, constraints),
        }
    }

    fn generate_mock_weather(&self) -> WeatherData {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        WeatherData {
            temperature_celsius: rng.gen_range(15.0..25.0),
            humidity_percent: rng.gen_range(40.0..70.0),
            wind_speed_ms: rng.gen_range(2.0..12.0),
            wind_direction_degrees: rng.gen_range(0.0..360.0),
            precipitation_mm: rng.gen_range(0.0..2.0),
            visibility_m: rng.gen_range(5000.0..15000.0),
            pressure_hpa: rng.gen_range(1010.0..1025.0),
            cloud_cover_percent: rng.gen_range(10.0..80.0),
        }
    }

    fn check_weather_alerts(&self, weather: &WeatherData) -> Vec<WeatherAlert> {
        let mut alerts = Vec::new();

        if weather.wind_speed_ms > 15.0 {
            alerts.push(WeatherAlert {
                severity: AlertSeverity::High,
                message: "High wind conditions - flight not recommended".to_string(),
                valid_until: chrono::Utc::now() + chrono::Duration::hours(2),
            });
        }

        if weather.precipitation_mm > 1.0 {
            alerts.push(WeatherAlert {
                severity: AlertSeverity::Medium,
                message: "Precipitation detected - monitor conditions".to_string(),
                valid_until: chrono::Utc::now() + chrono::Duration::hours(1),
            });
        }

        if weather.visibility_m < 1000.0 {
            alerts.push(WeatherAlert {
                severity: AlertSeverity::Critical,
                message: "Poor visibility - do not fly".to_string(),
                valid_until: chrono::Utc::now() + chrono::Duration::hours(3),
            });
        }

        alerts
    }

    fn calculate_weather_score(&self, weather: &WeatherData, constraints: &WeatherConstraints) -> f32 {
        let mut score = 100.0;

        // Wind penalty
        let wind_ratio = weather.wind_speed_ms / constraints.max_wind_speed_ms;
        score -= wind_ratio * 30.0;

        // Precipitation penalty
        let precip_ratio = weather.precipitation_mm / constraints.max_precipitation_mm;
        score -= precip_ratio * 25.0;

        // Visibility bonus/penalty
        let vis_ratio = weather.visibility_m / constraints.min_visibility_m;
        if vis_ratio < 1.0 {
            score -= (1.0 - vis_ratio) * 40.0;
        } else {
            score += (vis_ratio - 1.0).min(0.2) * 10.0; // Bonus for good visibility
        }

        // Temperature penalty
        let temp_center = (constraints.temperature_range_celsius.0 + constraints.temperature_range_celsius.1) / 2.0;
        let temp_range = constraints.temperature_range_celsius.1 - constraints.temperature_range_celsius.0;
        let temp_deviation = (weather.temperature_celsius - temp_center).abs() / (temp_range / 2.0);
        score -= temp_deviation * 10.0;

        score.max(0.0).min(100.0)
    }
}

#[derive(Debug, Clone)]
pub struct FlightConditionResult {
    pub flight_safe: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub weather_score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weather_check() {
        let integration = WeatherIntegration::new(None);
        let constraints = WeatherConstraints::default();
        
        let good_weather = WeatherData {
            temperature_celsius: 20.0,
            humidity_percent: 50.0,
            wind_speed_ms: 5.0,
            wind_direction_degrees: 180.0,
            precipitation_mm: 0.0,
            visibility_m: 10000.0,
            pressure_hpa: 1015.0,
            cloud_cover_percent: 30.0,
        };

        let result = integration.check_flight_conditions(&good_weather, &constraints);
        assert!(result.flight_safe);
        assert!(result.issues.is_empty());
        assert!(result.weather_score > 70.0);
    }

    #[test]
    fn test_bad_weather() {
        let integration = WeatherIntegration::new(None);
        let constraints = WeatherConstraints::default();
        
        let bad_weather = WeatherData {
            temperature_celsius: 20.0,
            humidity_percent: 50.0,
            wind_speed_ms: 20.0, // Too windy
            wind_direction_degrees: 180.0,
            precipitation_mm: 5.0, // Too much rain
            visibility_m: 500.0,   // Poor visibility
            pressure_hpa: 1015.0,
            cloud_cover_percent: 90.0,
        };

        let result = integration.check_flight_conditions(&bad_weather, &constraints);
        assert!(!result.flight_safe);
        assert!(!result.issues.is_empty());
        assert!(result.weather_score < 50.0);
    }
}
