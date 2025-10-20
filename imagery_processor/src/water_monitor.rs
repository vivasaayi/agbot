use anyhow::Result;
use std::path::Path;
use chrono::Utc;
use shared::schemas::{AOI, BBox, WaterAlert, AlertLevel};
use crate::{ndwi, vectorization};
use tracing::{info, warn};

/// Water Body Monitoring Service
pub struct WaterMonitor {
    threshold: f32,
    alert_threshold_pct: f64,
}

impl WaterMonitor {
    pub fn new() -> Self {
        Self {
            threshold: 0.3,
            alert_threshold_pct: 10.0,
        }
    }

    /// Process band arrays for water body detection (simplified demo version)
    pub async fn detect_water_bodies_from_arrays(
        &self, 
        green: &[f32], 
        nir: &[f32], 
        width: usize, 
        height: usize,
        output_dir: &Path
    ) -> Result<f64> {
        info!("Detecting water bodies from band arrays: {}x{}", width, height);

        // Create output paths
        let ndwi_path = output_dir.join("ndwi.txt");
        let mask_path = output_dir.join("water_mask.txt");
        let geojson_path = output_dir.join("water_polygons.geojson");

        // 1. Compute NDWI
        let (ndwi, shape) = ndwi::compute_ndwi_from_arrays(green, nir, width, height)?;
        ndwi::write_ndwi_text(&ndwi, shape, &ndwi_path)?;

        // 2. Threshold NDWI to get water mask
        let mask = ndwi::threshold_ndwi(&ndwi, self.threshold);
        ndwi::write_mask_text(&mask, shape, &mask_path)?;

        // 3. Simulate polygonization (in real implementation, use GDAL)
        let water_pixels = mask.iter().filter(|&&v| v == 1).count();
        let pixel_area = 900.0; // Assume 30m x 30m pixels (Landsat resolution)
        let total_area = water_pixels as f64 * pixel_area;

        // 4. Create a simple GeoJSON for demonstration
        let geojson_content = format!(
            r#"{{
  "type": "FeatureCollection",
  "features": [
    {{
      "type": "Feature",
      "properties": {{
        "area_m2": {:.2}
      }},
      "geometry": {{
        "type": "Polygon",
        "coordinates": [[
          [-120.0, 35.0],
          [-119.9, 35.0],
          [-119.9, 35.1],
          [-120.0, 35.1],
          [-120.0, 35.0]
        ]]
      }}
    }}
  ]
}}"#,
            total_area
        );
        std::fs::write(geojson_path, geojson_content)?;

        info!("Detected {} water pixels with total area: {:.2} m²", water_pixels, total_area);
        Ok(total_area)
    }

    /// Monitor water body changes and generate alerts
    pub fn check_for_alerts(&self, aoi: &AOI, prev_area: f64, curr_area: f64, next_rain_days: Option<u32>) -> Option<WaterAlert> {
        if prev_area <= 0.0 {
            return None;
        }

        let drop_pct = (prev_area - curr_area) / prev_area * 100.0;
        
        if drop_pct > self.alert_threshold_pct {
            let alert_level = if drop_pct > 30.0 {
                AlertLevel::Critical
            } else if drop_pct > 20.0 {
                AlertLevel::Warning
            } else {
                AlertLevel::Info
            };

            Some(WaterAlert {
                aoi_id: aoi.id.clone(),
                prev_area,
                curr_area,
                drop_pct,
                timestamp: Utc::now(),
                next_rain_days,
                alert_level,
            })
        } else {
            None
        }
    }

    /// Format alert message for notification
    pub fn format_alert_message(&self, alert: &WaterAlert) -> String {
        let area_drop = alert.prev_area - alert.curr_area;
        let rain_msg = if let Some(days) = alert.next_rain_days {
            format!("Next rain in {} days.", days)
        } else {
            "No rain forecast available.".to_string()
        };

        let severity = match alert.alert_level {
            AlertLevel::Critical => "🚨 CRITICAL",
            AlertLevel::Warning => "⚠️ WARNING",
            AlertLevel::Info => "ℹ️ INFO",
        };

        format!(
            "{} Lake area dropped by {:.0} m² ({:.1}%) this week. {} Watch for drought risk.",
            severity, area_drop, alert.drop_pct, rain_msg
        )
    }

    /// Generate mock satellite data for demonstration
    pub fn generate_mock_satellite_data(&self, width: usize, height: usize) -> (Vec<f32>, Vec<f32>) {
        let mut green = Vec::with_capacity(width * height);
        let mut nir = Vec::with_capacity(width * height);
        
        for y in 0..height {
            for x in 0..width {
                // Create a simple pattern with a "water body" in the center
                let center_x = width / 2;
                let center_y = height / 2;
                let distance = ((x as f32 - center_x as f32).powi(2) + (y as f32 - center_y as f32).powi(2)).sqrt();
                
                if distance < 20.0 {
                    // Water area - higher green, lower NIR
                    green.push(0.8);
                    nir.push(0.2);
                } else {
                    // Land area - lower green, higher NIR
                    green.push(0.3);
                    nir.push(0.7);
                }
            }
        }
        
        (green, nir)
    }
}

/// Example usage demonstration
#[tokio::main]
async fn main() -> Result<()> {
    shared::init_logging()?;

    // Example AOI
    let aoi = AOI {
        id: "lake-001".to_string(),
        bbox: BBox {
            min_lon: -120.0,
            min_lat: 35.0,
            max_lon: -119.0,
            max_lat: 36.0,
        },
        name: Some("Demo Lake".to_string()),
    };

    let monitor = WaterMonitor::new();
    let output_dir = Path::new("./water_output");
    
    // Create output directory
    std::fs::create_dir_all(output_dir)?;
    
    // Generate mock satellite data
    let (green, nir) = monitor.generate_mock_satellite_data(100, 100);
    
    // Process the mock data
    let current_area = monitor.detect_water_bodies_from_arrays(&green, &nir, 100, 100, output_dir).await?;
    
    // Simulate previous measurement
    let previous_area = 45000.0; // Previous area in m²
    
    // Check for alerts
    if let Some(alert) = monitor.check_for_alerts(&aoi, previous_area, current_area, Some(5)) {
        let message = monitor.format_alert_message(&alert);
        println!("{}", message);
        info!("Alert generated: {:?}", alert);
    } else {
        println!("Water area stable: {:.2} m²", current_area);
        info!("No significant water area change detected");
    }

    Ok(())
}
