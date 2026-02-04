#!/bin/bash
# AgBot Terrain Viewer Demo Script
# This script guides you through the key features of the new GIS terrain viewer

echo "🌾 AgBot Terrain Viewer Demo"
echo "============================"
echo ""
echo "Starting the visualizer..."
echo ""
echo "Once it loads, follow these steps:"
echo ""
echo "1️⃣  LOAD TERRAIN"
echo "   Press key '1' to load Nebraska farm"
echo "   (The globe should disappear and satellite imagery will appear)"
echo ""
echo "2️⃣  EXPLORE THE TERRAIN"
echo "   Use WASD to move around"
echo "   Press and drag RIGHT mouse button to rotate the view"
echo "   Use Q/E to move camera up and down (best way to see elevation!)"
echo "   Scroll wheel to zoom"
echo ""
echo "3️⃣  TOGGLE OVERLAYS"
echo "   Press 'N' to show NDVI vegetation index"
echo "   Press 'C' to show crop classification"
echo "   Press 'O' to show OpenStreetMap features"
echo "   Press '[' or ']' to adjust opacity"
echo ""
echo "4️⃣  TRY OTHER LOCATIONS"
echo "   Press '2' for Iowa corn belt"
echo "   Press '3' for California valley"
echo "   Press '4' for Salinas valley"
echo ""
echo "Starting visualizer now..."
echo ""

# Run the visualizer
cd "$(dirname "$0")"
cargo run -p visualizer
