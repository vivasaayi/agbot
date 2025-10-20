# Comprehensive Satellite Image Analysis System

A powerful, modular Rust-based system for comprehensive analysis of satellite imagery, specifically designed for agricultural and environmental monitoring applications.

## 🌟 Features

### Vegetation Analysis
- **NDVI** (Normalized Difference Vegetation Index)
- **EVI** (Enhanced Vegetation Index)
- **SAVI** (Soil Adjusted Vegetation Index)
- **ARVI** (Atmospherically Resistant Vegetation Index)
- **MSAVI** (Modified Soil Adjusted Vegetation Index)
- **CVI** (Chlorophyll Vegetation Index)
- **LAI** (Leaf Area Index)
- **fCover** (Fractional Cover)
- Health classification and stress detection
- Biomass estimation and carbon stock calculation
- Phenology analysis and growth stage detection

### Water Body Analysis
- **NDWI** (Normalized Difference Water Index)
- **MNDWI** (Modified NDWI)
- **AWEI** (Automated Water Extraction Index)
- Water body detection and vectorization
- Water quality assessment
- Turbidity and algae detection
- Temporal water change monitoring

### Drought Monitoring
- **VHI** (Vegetation Health Index)
- **VCI** (Vegetation Condition Index)
- **TCI** (Temperature Condition Index)
- **PDI** (Palmer Drought Index approximation)
- Drought severity classification
- Impact assessment (crop yield, economic, ecosystem)
- Recovery probability estimation

### Fire/Burn Analysis
- **NBR** (Normalized Burn Ratio)
- **dNBR** (differenced NBR)
- **BAI** (Burn Area Index)
- Burn severity mapping
- Fire progression tracking
- Recovery stage assessment
- Pre/post-fire comparison

### Urban and Land Cover
- **NDBI** (Normalized Difference Built-up Index)
- **UI** (Urban Index)
- **IBI** (Index-based Built-up Index)
- Automated land cover classification
- Change detection and hotspot identification

### Snow and Ice Analysis
- **NDSI** (Normalized Difference Snow Index)
- **NDSII** (Enhanced NDSI)
- **S3** (Snow Index)
- Snow cover mapping and monitoring

### Soil Analysis
- **BSI** (Bare Soil Index)
- **SI** (Soil Index)
- **RI** (Redness Index)
- Soil health and erosion assessment

### Multi-Temporal Analysis
- Time series trend analysis
- Anomaly detection
- Seasonal pattern recognition
- Forecasting and prediction
- Breakpoint detection

## 🚀 Quick Start

### Installation

1. **Clone the repository**:
   ```bash
   git clone <repository-url>
   cd agbot/ndvi_processor
   ```

2. **Build the project**:
   ```bash
   cargo build --release
   ```

### Running the Analysis

#### Demo Mode (Synthetic Data)
```bash
# Run comprehensive demo with all analyses
cargo run --release -- --demo --output-dir ./results --all-analyses

# Run specific analyses in demo mode
cargo run --release -- --demo --output-dir ./results --analysis-types "ndvi,water,drought"

# Verbose output
cargo run --release -- --demo --output-dir ./results --verbose
```

#### Real Data Processing
```bash
# Process real satellite imagery
cargo run --release -- --input-dir ./satellite_data --output-dir ./results --analysis-types "vegetation,water,landcover"

# Process with all available analyses
cargo run --release -- --input-dir ./satellite_data --output-dir ./results --all-analyses
```

### Command Line Options

```
USAGE:
    ndvi_processor [OPTIONS]

OPTIONS:
    -i, --input-dir <INPUT_DIR>           Input directory containing satellite images
    -o, --output-dir <OUTPUT_DIR>         Output directory for analysis results
    -a, --analysis-types <ANALYSIS_TYPES> Analysis types to perform (comma-separated)
                                         Available: ndvi,evi,savi,ndwi,nbr,drought,water,vegetation,landcover,temporal
        --all-analyses                   Run all available analyses
        --demo                           Enable demonstration mode with synthetic data
    -v, --verbose                        Verbose output
    -h, --help                           Print help information
    -V, --version                        Print version information
```

## 📊 Analysis Types

### Vegetation Indices
- `ndvi` - Normalized Difference Vegetation Index
- `evi` - Enhanced Vegetation Index
- `savi` - Soil Adjusted Vegetation Index
- `arvi` - Atmospherically Resistant Vegetation Index
- `msavi` - Modified Soil Adjusted Vegetation Index
- `cvi` - Chlorophyll Vegetation Index
- `lai` - Leaf Area Index
- `fcover` - Fractional Cover

### Water Indices
- `ndwi` - Normalized Difference Water Index
- `mndwi` - Modified NDWI
- `awei` - Automated Water Extraction Index

### Drought Indices
- `vhi` - Vegetation Health Index
- `vci` - Vegetation Condition Index
- `tci` - Temperature Condition Index
- `pdi` - Palmer Drought Index

### Burn/Fire Indices
- `nbr` - Normalized Burn Ratio
- `dnbr` - differenced NBR
- `bai` - Burn Area Index

### Urban/Built-up Indices
- `ndbi` - Normalized Difference Built-up Index
- `ui` - Urban Index
- `ibi` - Index-based Built-up Index

### Snow Indices
- `ndsi` - Normalized Difference Snow Index
- `ndsii` - Enhanced NDSI
- `s3` - Snow Index

### Soil Indices
- `bsi` - Bare Soil Index
- `si` - Soil Index
- `ri` - Redness Index

### Composite Analyses
- `vegetation` - Comprehensive vegetation analysis
- `water` - Comprehensive water body analysis
- `drought` - Comprehensive drought monitoring
- `burn` - Comprehensive fire/burn analysis
- `landcover` - Land cover classification
- `temporal` - Multi-temporal analysis

## 📁 Expected Data Format

The system expects satellite imagery in the following band structure:

### Required Bands
- **Blue** (450-495 nm)
- **Green** (495-570 nm)
- **Red** (620-750 nm)
- **NIR** (750-950 nm)

### Optional Bands (for advanced analyses)
- **SWIR1** (1550-1750 nm)
- **SWIR2** (2080-2350 nm)

### Supported Formats
- GeoTIFF (.tif, .tiff)
- Individual band files or multi-band files
- Common satellite data formats (Landsat, Sentinel-2, etc.)

## 📈 Output

The system generates comprehensive analysis results including:

### Raster Outputs
- Index maps (GeoTIFF format)
- Classification maps
- Confidence maps
- Change detection maps

### Vector Outputs
- Water body polygons (GeoJSON)
- Degraded area boundaries
- Land cover boundaries

### Reports
- Comprehensive analysis report (Markdown)
- Statistical summaries (JSON)
- Quality assessment reports
- Recommendations and alerts

### Example Output Structure
```
results/
├── comprehensive_analysis_report.md
├── vegetation/
│   ├── ndvi_map.tif
│   ├── health_classification.tif
│   ├── biomass_estimate.json
│   └── stress_indicators.json
├── water/
│   ├── water_mask.tif
│   ├── water_bodies.geojson
│   ├── quality_assessment.json
│   └── temporal_change.json
├── drought/
│   ├── drought_severity.tif
│   ├── vhi_map.tif
│   └── impact_assessment.json
└── landcover/
    ├── classification.tif
    ├── confidence.tif
    └── class_statistics.json
```

## 🔬 Technical Details

### Architecture
- **Modular Design**: Each analysis type is implemented as a separate module
- **Trait-Based**: Common interfaces for all analysis types
- **Async Processing**: Efficient handling of large datasets
- **Memory Efficient**: Streaming processing for large rasters

### Performance Optimizations
- **Parallel Processing**: Multi-threaded computation where applicable
- **SIMD Operations**: Vectorized mathematical operations
- **Efficient Algorithms**: Optimized implementations of spectral indices
- **Memory Management**: Smart caching and lazy evaluation

### Quality Assurance
- **Input Validation**: Comprehensive data quality checks
- **Error Handling**: Robust error handling and recovery
- **Logging**: Detailed logging for debugging and monitoring
- **Documentation**: Comprehensive API documentation

## 🧪 Testing

### Unit Tests
```bash
cargo test
```

### Integration Tests
```bash
cargo test --test integration
```

### Demo Testing
```bash
cargo run --release -- --demo --output-dir ./test_results
```

## 📚 API Documentation

Generate and view API documentation:
```bash
cargo doc --open
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Guidelines
- Follow Rust naming conventions
- Add comprehensive documentation
- Include unit tests for new features
- Maintain backwards compatibility
- Update this README for new features

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [USGS](https://www.usgs.gov/) for Landsat data
- [ESA](https://www.esa.int/) for Sentinel data
- [NASA](https://www.nasa.gov/) for MODIS data
- Remote sensing research community for algorithm development

## 🔗 Related Projects

- [GDAL](https://gdal.org/) - Geospatial Data Abstraction Library
- [QGIS](https://qgis.org/) - Geographic Information System
- [Google Earth Engine](https://earthengine.google.com/) - Planetary-scale geospatial analysis

## 📞 Support

For questions, issues, or feature requests:
- Open an issue on GitHub
- Check the documentation
- Review existing issues and discussions

---

**Built with ❤️ for sustainable agriculture and environmental monitoring**
