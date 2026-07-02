#include "agbot_worldgen/extractors/ClassicalIndex.hpp"

#include "agbot_worldgen/Polygonize.hpp"

#include "agbot_terrain/Png.hpp"

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <filesystem>
#include <string>
#include <vector>

namespace agbot::worldgen {
namespace {

namespace cfg = agbot::config;

constexpr std::uint8_t kBackground = 0;
constexpr std::uint8_t kVegetation = 1;
constexpr std::uint8_t kWater = 2;

struct ClassicalParams {
    std::string imagery_path;
    std::string veg_method = "exg";
    double veg_thresh = 0.08;
    std::string water_method = "blueness";
    double water_thresh = 0.05;
    int morph_open_px = 0;
    double min_area_m2 = 10.0;
    double simplify_tol_m = 0.0;
    double confidence = 0.6;
};

ClassicalParams read_params(const cfg::ParamTable& table) {
    ClassicalParams params;
    params.imagery_path = cfg::string_or(table, "imagery_path", "");
    params.veg_method = cfg::string_or(table, "veg_method", "exg");
    params.veg_thresh = cfg::double_or(table, "veg_thresh", 0.08);
    params.water_method = cfg::string_or(table, "water_method", "blueness");
    params.water_thresh = cfg::double_or(table, "water_thresh", 0.05);
    params.morph_open_px =
        static_cast<int>(cfg::integer_or(table, "morph_open_px", 0));
    params.min_area_m2 = cfg::double_or(table, "min_area_m2", 10.0);
    params.simplify_tol_m = cfg::double_or(table, "simplify_tol_m", 0.0);
    params.confidence = cfg::double_or(table, "confidence", 0.6);
    return params;
}

// Binary opening (erosion then dilation) with a (2r+1)^2 square structuring
// element. Removes salt noise smaller than the element while keeping the
// bulk shape of larger blobs.
void binary_open(std::vector<std::uint8_t>& mask, int width, int height, int radius) {
    if (radius <= 0) {
        return;
    }
    const auto pass = [&](const std::vector<std::uint8_t>& src, bool erode) {
        std::vector<std::uint8_t> dst(src.size(), 0);
        for (int y = 0; y < height; ++y) {
            for (int x = 0; x < width; ++x) {
                bool all = true;
                bool any = false;
                for (int dy = -radius; dy <= radius && (erode ? all : !any); ++dy) {
                    for (int dx = -radius; dx <= radius && (erode ? all : !any); ++dx) {
                        const int nx = x + dx;
                        const int ny = y + dy;
                        const bool value = nx >= 0 && nx < width && ny >= 0 && ny < height &&
                            src[static_cast<std::size_t>(ny) * width + nx] != 0;
                        all = all && value;
                        any = any || value;
                    }
                }
                dst[static_cast<std::size_t>(y) * width + x] =
                    (erode ? all : any) ? 1 : 0;
            }
        }
        return dst;
    };
    mask = pass(pass(mask, /*erode=*/true), /*erode=*/false);
}

} // namespace

std::string ClassicalIndexExtractor::id() const {
    return kId;
}

std::vector<FeatureClass> ClassicalIndexExtractor::produces() const {
    return {FeatureClass::Vegetation, FeatureClass::Water};
}

ExtractionResult ClassicalIndexExtractor::extract(const ExtractionContext& context) const {
    ExtractionResult result;
    result.algorithm_id = kId;
    result.params_hash = cfg::param_hash(context.params);

    const ClassicalParams params = read_params(context.params);
    if (params.imagery_path.empty()) {
        result.error_code = "params_missing_imagery_path";
        result.error_detail = "classical_index requires an 'imagery_path' parameter";
        return result;
    }
    if (!std::filesystem::exists(params.imagery_path)) {
        result.error_code = "imagery_not_found";
        result.error_detail = "cannot open imagery file: " + params.imagery_path;
        return result;
    }
    if (params.veg_method != "exg" && params.veg_method != "green_ratio") {
        result.error_code = "unknown_veg_method";
        result.error_detail = "unsupported veg_method: " + params.veg_method;
        return result;
    }
    if (params.water_method != "blueness") {
        result.error_code = "unknown_water_method";
        result.error_detail = "unsupported water_method: " + params.water_method;
        return result;
    }

    const agbot::terrain::PngImage image =
        agbot::terrain::decode_png_rgba_file(params.imagery_path);
    if (!image.ok || image.width <= 0 || image.height <= 0) {
        result.error_code = "imagery_decode_failed";
        result.error_detail = "PNG decode failed (" + image.error + "): " + params.imagery_path;
        return result;
    }

    const std::size_t pixel_count =
        static_cast<std::size_t>(image.width) * static_cast<std::size_t>(image.height);
    std::vector<std::uint8_t> veg_mask(pixel_count, 0);
    std::vector<std::uint8_t> water_mask(pixel_count, 0);
    for (std::size_t index = 0; index < pixel_count; ++index) {
        const double r = image.rgba[index * 4 + 0] / 255.0;
        const double g = image.rgba[index * 4 + 1] / 255.0;
        const double b = image.rgba[index * 4 + 2] / 255.0;
        const double sum = r + g + b;

        bool is_veg = false;
        if (sum > 1e-9) {
            if (params.veg_method == "exg") {
                // Excess green over chromatic coordinates: 2g' - r' - b'.
                is_veg = (2.0 * g - r - b) / sum > params.veg_thresh;
            } else { // green_ratio
                is_veg = g / sum > params.veg_thresh;
            }
        }
        if (is_veg) {
            veg_mask[index] = 1;
            continue; // vegetation wins ties
        }
        if (b - std::max(r, g) > params.water_thresh) {
            water_mask[index] = 1;
        }
    }
    binary_open(veg_mask, image.width, image.height, params.morph_open_px);
    binary_open(water_mask, image.width, image.height, params.morph_open_px);

    ClassMask class_mask;
    class_mask.width = image.width;
    class_mask.height = image.height;
    class_mask.classes.assign(pixel_count, kBackground);
    for (std::size_t index = 0; index < pixel_count; ++index) {
        if (veg_mask[index] != 0) {
            class_mask.classes[index] = kVegetation;
        } else if (water_mask[index] != 0) {
            class_mask.classes[index] = kWater;
        }
    }

    PolygonizeOptions options;
    options.simplify_tol_m = params.simplify_tol_m;
    options.min_area_m2 = params.min_area_m2;

    const struct {
        std::uint8_t class_id;
        FeatureClass cls;
        const char* name;
    } layers[] = {
        {kVegetation, FeatureClass::Vegetation, "vegetation"},
        {kWater, FeatureClass::Water, "water"},
    };
    for (const auto& layer : layers) {
        const std::vector<RasterPolygon> polygons =
            polygonize_class(class_mask, context.aoi, layer.class_id, options);
        for (const RasterPolygon& polygon : polygons) {
            ExtractedFeature feature;
            feature.cls = layer.cls;
            feature.class_name = layer.name;
            feature.exterior = polygon.exterior;
            feature.holes = polygon.holes;
            feature.confidence = params.confidence;
            feature.source_id = std::string(layer.name) + ":" +
                std::to_string(polygon.component_label);
            feature.attributes["cells"] = std::to_string(polygon.cell_count);
            feature.attributes["method"] =
                layer.class_id == kVegetation ? params.veg_method : params.water_method;
            result.features.push_back(std::move(feature));
        }
    }

    result.ok = true;
    return result;
}

} // namespace agbot::worldgen
