use anyhow::Result;
use std::path::Path;
use chrono::Utc;

// Standalone demo without shared crate dependencies

#[derive(Debug, Clone)]
pub struct BBox {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

#[derive(Debug, Clone)]
pub struct AOI {
    pub id: String,
    pub bbox: BBox,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct WaterAlert {
    pub aoi_id: String,
    pub prev_area: f64,
    pub curr_area: f64,
    pub drop_pct: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub next_rain_days: Option<u32>,
    pub alert_level: AlertLevel,
}

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

    /// Compute NDWI from band data arrays
    pub fn compute_ndwi_from_arrays(&self, green: &[f32], nir: &[f32]) -> Result<Vec<f32>> {
        if green.len() != nir.len() {
            anyhow::bail!("Green and NIR arrays must have the same length");
        }
        
        let ndwi: Vec<f32> = green.iter().zip(nir.iter())
            .map(|(&g, &n)| if (g + n).abs() > 1e-6 { (g - n) / (g + n) } else { 0.0 })
            .collect();
        
        Ok(ndwi)
    }

    /// Threshold NDWI to create a binary water mask
    pub fn threshold_ndwi(&self, ndwi: &[f32]) -> Vec<u8> {
        ndwi.iter().map(|&v| if v > self.threshold { 1 } else { 0 }).collect()
    }

    /// Process band arrays for water body detection
    pub async fn detect_water_bodies_from_arrays(
        &self, 
        green: &[f32], 
        nir: &[f32], 
        width: usize, 
        height: usize,
        output_dir: &Path
    ) -> Result<f64> {
        println!("🛰️  Detecting water bodies from {}x{} satellite data", width, height);

        // Create output directory
        std::fs::create_dir_all(output_dir)?;

        // 1. Compute NDWI
        let ndwi = self.compute_ndwi_from_arrays(green, nir)?;
        
        // 2. Threshold NDWI to get water mask
        let mask = self.threshold_ndwi(&ndwi);
        
        // 3. Calculate water statistics
        let water_pixels = mask.iter().filter(|&&v| v == 1).count();
        let total_pixels = mask.len();
        let water_percentage = (water_pixels as f64 / total_pixels as f64) * 100.0;
        
        // 4. Estimate water area (assuming 30m Landsat pixels)
        let pixel_area = 900.0; // 30m x 30m
        let total_area = water_pixels as f64 * pixel_area;

        // 5. Save results
        let ndwi_stats = format!(
            "NDWI Statistics:\n\
             - Total pixels: {}\n\
             - Water pixels: {} ({:.1}%)\n\
             - Estimated water area: {:.0} m²\n\
             - NDWI threshold: {:.2}",
            total_pixels, water_pixels, water_percentage, total_area, self.threshold
        );
        
        let results_path = output_dir.join("water_detection_results.txt");
        std::fs::write(results_path, ndwi_stats)?;

        println!("💧 Detected {} water pixels ({:.1}%) = {:.0} m²", 
                 water_pixels, water_percentage, total_area);
        
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
    pub fn generate_mock_satellite_data(width: usize, height: usize) -> (Vec<f32>, Vec<f32>) {
        let mut green = Vec::with_capacity(width * height);
        let mut nir = Vec::with_capacity(width * height);
        
        for y in 0..height {
            for x in 0..width {
                // Create a pattern with multiple "water bodies"
                let center_x = width / 2;
                let center_y = height / 2;
                let distance = ((x as f32 - center_x as f32).powi(2) + (y as f32 - center_y as f32).powi(2)).sqrt();
                
                // Create a "lake" in center
                let in_main_lake = distance < 15.0;
                
                // Create smaller "ponds"
                let in_pond1 = ((x as f32 - 20.0).powi(2) + (y as f32 - 20.0).powi(2)).sqrt() < 8.0;
                let in_pond2 = ((x as f32 - 80.0).powi(2) + (y as f32 - 60.0).powi(2)).sqrt() < 6.0;
                
                if in_main_lake || in_pond1 || in_pond2 {
                    // Water area - higher green reflectance, lower NIR
                    green.push(0.8 + (x as f32 / width as f32) * 0.1); // Slight variation
                    nir.push(0.1 + (y as f32 / height as f32) * 0.1);
                } else {
                    // Land/vegetation - lower green, higher NIR
                    green.push(0.2 + (x as f32 / width as f32) * 0.2);
                    nir.push(0.6 + (y as f32 / height as f32) * 0.2);
                }
            }
        }
        
        (green, nir)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Water Body Detection Demo");
    
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
    
    println!("📊 Generating mock satellite data (100x100 pixels)...");
    
    // Generate mock satellite data
    let (green, nir) = monitor.generate_mock_satellite_data(100, 100);
    
    // Process the mock data
    let current_area = monitor.detect_water_bodies_from_arrays(&green, &nir, 100, 100, output_dir).await?;
    
    // Simulate previous measurement for comparison
    let previous_area = 45000.0; // Previous area in m²
    
    println!("\n📈 Temporal Analysis:");
    println!("Previous water area: {:.0} m²", previous_area);
    println!("Current water area:  {:.0} m²", current_area);
    
    // Check for alerts
    if let Some(alert) = monitor.check_for_alerts(&aoi, previous_area, current_area, Some(5)) {
        let message = monitor.format_alert_message(&alert);
        println!("\n{}", message);
        println!("Alert details: {:?}", alert);
    } else {
        println!("\n✅ Water area stable - no alerts triggered");
    }

    println!("\n📁 Results saved to: {}", output_dir.display());
    println!("🎉 Water body detection demo completed successfully!");

    Ok(())
}
