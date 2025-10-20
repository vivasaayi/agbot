#!/usr/bin/env python3
"""
Water Body Detection Demo Output
Shows what the Rust implementation would produce
"""

import json
from datetime import datetime
import math
import random

def simulate_water_detection():
    print("🚀 Starting Water Body Detection Demo")
    print("📊 Generating mock satellite data (100x100 pixels)...")
    
    # Simulate NDWI computation
    width, height = 100, 100
    total_pixels = width * height
    
    # Simulate water detection results
    water_pixels = 0
    
    # Create mock "water bodies" - main lake + 2 ponds
    center_x, center_y = width // 2, height // 2
    
    for y in range(height):
        for x in range(width):
            # Main lake (radius 15)
            dist_center = math.sqrt((x - center_x)**2 + (y - center_y)**2)
            in_main_lake = dist_center < 15
            
            # Pond 1 (radius 8 at position 20,20)
            dist_pond1 = math.sqrt((x - 20)**2 + (y - 20)**2)
            in_pond1 = dist_pond1 < 8
            
            # Pond 2 (radius 6 at position 80,60)
            dist_pond2 = math.sqrt((x - 80)**2 + (y - 60)**2)
            in_pond2 = dist_pond2 < 6
            
            if in_main_lake or in_pond1 or in_pond2:
                water_pixels += 1
    
    water_percentage = (water_pixels / total_pixels) * 100
    pixel_area = 900.0  # 30m x 30m Landsat pixels
    total_area = water_pixels * pixel_area
    
    print("🛰️  Detecting water bodies from 100x100 satellite data")
    print(f"💧 Detected {water_pixels} water pixels ({water_percentage:.1f}%) = {total_area:.0f} m²")
    
    # Simulate NDWI statistics
    stats = {
        "timestamp": datetime.now().isoformat(),
        "total_pixels": total_pixels,
        "water_pixels": water_pixels,
        "water_percentage": round(water_percentage, 1),
        "total_area_m2": total_area,
        "ndwi_threshold": 0.3,
        "ndwi_min": -0.8,
        "ndwi_max": 0.6,
        "ndwi_mean": 0.1,
        "water_bodies_count": 3
    }
    
    print("\n📊 NDWI Statistics:")
    print(f"- Total pixels: {total_pixels}")
    print(f"- Water pixels: {water_pixels} ({water_percentage:.1f}%)")
    print(f"- Estimated water area: {total_area:.0f} m²")
    print(f"- NDWI threshold: 0.3")
    print(f"- Water bodies detected: 3")
    
    # Simulate temporal analysis
    previous_area = 45000.0
    print(f"\n📈 Temporal Analysis:")
    print(f"Previous water area: {previous_area:.0f} m²")
    print(f"Current water area:  {total_area:.0f} m²")
    
    # Check for alerts
    drop_pct = (previous_area - total_area) / previous_area * 100
    
    if drop_pct > 10:  # Alert threshold
        area_drop = previous_area - total_area
        if drop_pct > 30:
            level = "🚨 CRITICAL"
        elif drop_pct > 20:
            level = "⚠️ WARNING"
        else:
            level = "ℹ️ INFO"
        
        print(f"\n{level} Lake area dropped by {area_drop:.0f} m² ({drop_pct:.1f}%) this week. Next rain in 5 days. Watch for drought risk.")
        
        alert = {
            "aoi_id": "lake-001",
            "prev_area": previous_area,
            "curr_area": total_area,
            "drop_pct": round(drop_pct, 1),
            "timestamp": datetime.now().isoformat(),
            "next_rain_days": 5,
            "alert_level": level.split()[1]
        }
        
        print(f"Alert details: {json.dumps(alert, indent=2)}")
    else:
        print("\n✅ Water area stable - no alerts triggered")
    
    print("\n📁 Results would be saved to: ./water_output/")
    print("   - water_detection_results.txt")
    print("   - ndwi.txt")
    print("   - water_mask.txt") 
    print("   - water_polygons.geojson")
    
    print("\n🎉 Water body detection demo completed successfully!")
    
    return stats

if __name__ == "__main__":
    simulate_water_detection()
