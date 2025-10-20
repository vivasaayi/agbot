#!/usr/bin/env python3
"""
Water Body Detection Demo - Python Implementation
Demonstrates the same NDWI functionality as the Rust implementation
"""

import numpy as np
import matplotlib.pyplot as plt
from datetime import datetime
import json
import os

class WaterMonitor:
    def __init__(self, threshold=0.3, alert_threshold_pct=10.0):
        self.threshold = threshold
        self.alert_threshold_pct = alert_threshold_pct
    
    def compute_ndwi(self, green, nir):
        """Compute NDWI: (Green - NIR) / (Green + NIR)"""
        with np.errstate(divide='ignore', invalid='ignore'):
            ndwi = (green - nir) / (green + nir)
            ndwi = np.where(np.isfinite(ndwi), ndwi, 0.0)
        return ndwi
    
    def threshold_ndwi(self, ndwi):
        """Create binary water mask"""
        return (ndwi > self.threshold).astype(np.uint8)
    
    def generate_mock_satellite_data(self, width=100, height=100):
        """Generate mock satellite data with water bodies"""
        green = np.zeros((height, width))
        nir = np.zeros((height, width))
        
        # Create coordinate grids
        y, x = np.ogrid[:height, :width]
        center_x, center_y = width // 2, height // 2
        
        # Main lake in center
        main_lake = (x - center_x)**2 + (y - center_y)**2 < 15**2
        
        # Smaller ponds
        pond1 = (x - 20)**2 + (y - 20)**2 < 8**2
        pond2 = (x - 80)**2 + (y - 60)**2 < 6**2
        
        # Water areas
        water_mask = main_lake | pond1 | pond2
        
        # Set reflectance values
        green[water_mask] = 0.8 + np.random.normal(0, 0.05, np.sum(water_mask))
        green[~water_mask] = 0.2 + np.random.normal(0, 0.05, np.sum(~water_mask))
        
        nir[water_mask] = 0.1 + np.random.normal(0, 0.05, np.sum(water_mask))
        nir[~water_mask] = 0.6 + np.random.normal(0, 0.1, np.sum(~water_mask))
        
        # Ensure values are in valid range
        green = np.clip(green, 0, 1)
        nir = np.clip(nir, 0, 1)
        
        return green, nir
    
    def detect_water_bodies(self, green, nir, output_dir="./water_output"):
        """Process satellite data for water body detection"""
        print("🛰️  Detecting water bodies from satellite data")
        
        # Create output directory
        os.makedirs(output_dir, exist_ok=True)
        
        # 1. Compute NDWI
        ndwi = self.compute_ndwi(green, nir)
        
        # 2. Threshold to get water mask
        water_mask = self.threshold_ndwi(ndwi)
        
        # 3. Calculate statistics
        total_pixels = water_mask.size
        water_pixels = np.sum(water_mask)
        water_percentage = (water_pixels / total_pixels) * 100
        
        # 4. Estimate water area (30m Landsat pixels)
        pixel_area = 900.0  # m²
        total_area = water_pixels * pixel_area
        
        # 5. Save visualizations
        self.save_results(green, nir, ndwi, water_mask, output_dir)
        
        # 6. Save statistics
        stats = {
            "timestamp": datetime.now().isoformat(),
            "total_pixels": int(total_pixels),
            "water_pixels": int(water_pixels),
            "water_percentage": float(water_percentage),
            "total_area_m2": float(total_area),
            "ndwi_threshold": float(self.threshold),
            "ndwi_min": float(np.min(ndwi)),
            "ndwi_max": float(np.max(ndwi)),
            "ndwi_mean": float(np.mean(ndwi))
        }
        
        with open(f"{output_dir}/water_stats.json", "w") as f:
            json.dump(stats, f, indent=2)
        
        print(f"💧 Detected {water_pixels} water pixels ({water_percentage:.1f}%) = {total_area:.0f} m²")
        return total_area
    
    def save_results(self, green, nir, ndwi, water_mask, output_dir):
        """Save visualization results"""
        fig, axes = plt.subplots(2, 2, figsize=(12, 10))
        fig.suptitle('Water Body Detection Results', fontsize=16)
        
        # Green band
        im1 = axes[0,0].imshow(green, cmap='Greens')
        axes[0,0].set_title('Green Band')
        axes[0,0].axis('off')
        plt.colorbar(im1, ax=axes[0,0])
        
        # NIR band
        im2 = axes[0,1].imshow(nir, cmap='Reds')
        axes[0,1].set_title('NIR Band')
        axes[0,1].axis('off')
        plt.colorbar(im2, ax=axes[0,1])
        
        # NDWI
        im3 = axes[1,0].imshow(ndwi, cmap='RdYlBu', vmin=-1, vmax=1)
        axes[1,0].set_title('NDWI')
        axes[1,0].axis('off')
        plt.colorbar(im3, ax=axes[1,0])
        
        # Water mask
        im4 = axes[1,1].imshow(water_mask, cmap='Blues')
        axes[1,1].set_title('Water Mask')
        axes[1,1].axis('off')
        plt.colorbar(im4, ax=axes[1,1])
        
        plt.tight_layout()
        plt.savefig(f"{output_dir}/water_detection_results.png", dpi=150, bbox_inches='tight')
        plt.close()
        
        print(f"📁 Results saved to: {output_dir}/")
    
    def check_for_alerts(self, aoi_id, prev_area, curr_area, next_rain_days=None):
        """Check for drought alerts"""
        if prev_area <= 0:
            return None
        
        drop_pct = (prev_area - curr_area) / prev_area * 100
        
        if drop_pct > self.alert_threshold_pct:
            if drop_pct > 30:
                level = "CRITICAL"
                emoji = "🚨"
            elif drop_pct > 20:
                level = "WARNING"
                emoji = "⚠️"
            else:
                level = "INFO"
                emoji = "ℹ️"
            
            alert = {
                "aoi_id": aoi_id,
                "prev_area": prev_area,
                "curr_area": curr_area,
                "drop_pct": drop_pct,
                "timestamp": datetime.now().isoformat(),
                "next_rain_days": next_rain_days,
                "alert_level": level
            }
            
            area_drop = prev_area - curr_area
            rain_msg = f"Next rain in {next_rain_days} days." if next_rain_days else "No rain forecast."
            
            message = f"{emoji} {level} Lake area dropped by {area_drop:.0f} m² ({drop_pct:.1f}%) this week. {rain_msg} Watch for drought risk."
            
            return alert, message
        
        return None

def main():
    print("🚀 Starting Water Body Detection Demo")
    
    # Initialize monitor
    monitor = WaterMonitor()
    
    # Generate mock satellite data
    print("📊 Generating mock satellite data (100x100 pixels)...")
    green, nir = monitor.generate_mock_satellite_data(100, 100)
    
    # Process the data
    current_area = monitor.detect_water_bodies(green, nir)
    
    # Simulate temporal analysis
    previous_area = 45000.0  # Previous measurement
    
    print("\n📈 Temporal Analysis:")
    print(f"Previous water area: {previous_area:.0f} m²")
    print(f"Current water area:  {current_area:.0f} m²")
    
    # Check for alerts
    result = monitor.check_for_alerts("lake-001", previous_area, current_area, 5)
    
    if result:
        alert, message = result
        print(f"\n{message}")
        print(f"Alert details: {json.dumps(alert, indent=2)}")
    else:
        print("\n✅ Water area stable - no alerts triggered")
    
    print("\n🎉 Water body detection demo completed successfully!")

if __name__ == "__main__":
    main()
