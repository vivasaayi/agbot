#!/bin/bash

# Comprehensive Satellite Image Analysis Demo Script
# This script demonstrates all the analysis capabilities

echo "🛰️ Comprehensive Satellite Image Analysis System Demo"
echo "=================================================="

# Create demo output directory
OUTPUT_DIR="./demo_results"
mkdir -p "$OUTPUT_DIR"

echo ""
echo "🧪 Running comprehensive demo with synthetic data..."
echo "This demo will showcase all analysis capabilities:"
echo "  - Vegetation health and biomass estimation"
echo "  - Water body detection and quality assessment"
echo "  - Drought monitoring and impact assessment"
echo "  - Fire/burn analysis and recovery tracking"
echo "  - Land cover classification"
echo "  - Multi-index comprehensive analysis"
echo ""

# Run the comprehensive demo
cargo run --release -- \
    --demo \
    --output-dir "$OUTPUT_DIR" \
    --all-analyses \
    --verbose

echo ""
echo "✅ Demo completed! Check the results in $OUTPUT_DIR"
echo ""

# Display results summary
if [ -f "$OUTPUT_DIR/comprehensive_analysis_report.md" ]; then
    echo "📄 Analysis Report Generated:"
    echo "----------------------------"
    head -30 "$OUTPUT_DIR/comprehensive_analysis_report.md"
    echo ""
    echo "📁 Full report available at: $OUTPUT_DIR/comprehensive_analysis_report.md"
fi

echo ""
echo "🔍 Available analysis types:"
echo "  Vegetation: ndvi, evi, savi, arvi, msavi, cvi, lai, fcover"
echo "  Water:      ndwi, mndwi, awei"
echo "  Drought:    vhi, vci, tci, pdi"
echo "  Fire/Burn:  nbr, dnbr, bai"
echo "  Urban:      ndbi, ui, ibi"
echo "  Snow:       ndsi, ndsii, s3"
echo "  Soil:       bsi, si, ri"
echo "  Composite:  vegetation, water, drought, burn, landcover, temporal"
echo ""

echo "🚀 Example commands for real data:"
echo "  # Basic vegetation analysis:"
echo "  cargo run --release -- -i ./satellite_data -o ./results -a ndvi,evi"
echo ""
echo "  # Comprehensive water analysis:"
echo "  cargo run --release -- -i ./satellite_data -o ./results -a water"
echo ""
echo "  # All analyses:"
echo "  cargo run --release -- -i ./satellite_data -o ./results --all-analyses"
echo ""

echo "📊 For more information, see COMPREHENSIVE_ANALYSIS_README.md"
