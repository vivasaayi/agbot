#include "agbot_worldgen/extractors/OnnxSemSeg.hpp"

#include "agbot_worldgen/Polygonize.hpp"
#include "agbot_worldgen/SegTiler.hpp"

#include "agbot_terrain/Png.hpp"

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <filesystem>
#include <map>
#include <string>
#include <vector>

#if defined(AGBOT_WORLDGEN_HAS_ONNX)
#include <onnxruntime_cxx_api.h>

#include <array>
#include <memory>
#include <mutex>
#include <unordered_map>
#endif

namespace agbot::worldgen {
namespace {

namespace cfg = agbot::config;

} // namespace

std::string OnnxSemSegExtractor::id() const {
    return kId;
}

std::vector<FeatureClass> OnnxSemSegExtractor::produces() const {
    return {
        FeatureClass::Building,
        FeatureClass::Road,
        FeatureClass::Water,
        FeatureClass::Vegetation,
        FeatureClass::Bare,
        FeatureClass::Unknown,
    };
}

#if !defined(AGBOT_WORLDGEN_HAS_ONNX)

// Stub: keeps pipelines loadable without ONNX Runtime; extraction reports a
// stable reason code instead of crashing or silently returning nothing.
ExtractionResult OnnxSemSegExtractor::extract(const ExtractionContext& context) const {
    ExtractionResult result;
    result.algorithm_id = kId;
    result.params_hash = cfg::param_hash(context.params);
    result.error_code = "onnx_runtime_unavailable";
    result.error_detail =
        "onnx_semseg was built without ONNX Runtime; rebuild with "
        "-DAGBOT_WORLDGEN_WITH_ONNX=ON and onnxruntime installed";
    return result;
}

#else // AGBOT_WORLDGEN_HAS_ONNX

namespace {

std::string default_model_path() {
    return std::string(AGBOT_FLIGHT_SIM_SOURCE_DIR) +
        "/data/models/segformer_b0_ade20k_512.onnx";
}

// ADE20K (150 classes, 0-based) subset relevant to worldgen. Cityscapes
// exports were not publicly downloadable, so the default model/class map is
// ADE20K; override via the class_map param for other models.
std::map<int, std::string> default_class_map() {
    return {
        {1, "building"},
        {4, "vegetation"}, // tree
        {6, "road"},
        {9, "vegetation"}, // grass
        {11, "road"},      // sidewalk
        {13, "bare"},      // earth
        {21, "water"},
        {26, "water"},     // sea
        {60, "water"},     // river
    };
}

struct SemSegParams {
    std::string model_path;
    std::string imagery_path;
    int input_size = 512;
    int overlap_px = 64;
    std::map<int, std::string> class_map;
    std::string execution_provider = "cpu";
    std::array<float, 3> norm_mean{0.485f, 0.456f, 0.406f};
    std::array<float, 3> norm_std{0.229f, 0.224f, 0.225f};
    double min_area_m2 = 10.0;
    double simplify_tol_m = 0.0;
    double default_confidence = 0.5;
};

std::array<float, 3> vec3_or(
    const cfg::ParamTable& params, const std::string& key, std::array<float, 3> fallback) {
    const cfg::ParamArray* array = cfg::find_array(params, key);
    if (array == nullptr || array->size() != 3) {
        return fallback;
    }
    std::array<float, 3> out = fallback;
    for (std::size_t i = 0; i < 3; ++i) {
        if (!(*array)[i].is_number()) {
            return fallback;
        }
        out[i] = static_cast<float>((*array)[i].as_double());
    }
    return out;
}

SemSegParams read_params(const cfg::ParamTable& table) {
    SemSegParams params;
    params.model_path = cfg::string_or(table, "model_path", default_model_path());
    params.imagery_path = cfg::string_or(table, "imagery_path", "");
    int input_size = static_cast<int>(cfg::integer_or(table, "input_size", 512));
    input_size = std::clamp(input_size, 64, 2048);
    params.input_size = (input_size / 32) * 32;
    params.overlap_px = static_cast<int>(cfg::integer_or(table, "overlap_px", 64));
    params.execution_provider = cfg::string_or(table, "execution_provider", "cpu");
    params.norm_mean = vec3_or(table, "norm_mean", params.norm_mean);
    params.norm_std = vec3_or(table, "norm_std", params.norm_std);
    params.min_area_m2 = cfg::double_or(table, "min_area_m2", 10.0);
    params.simplify_tol_m = cfg::double_or(table, "simplify_tol_m", 0.0);
    params.default_confidence = cfg::double_or(table, "default_confidence", 0.5);

    if (const cfg::ParamTable* map_table = cfg::find_table(table, "class_map")) {
        for (const auto& [key, value] : *map_table) {
            if (!value.is_string()) {
                continue;
            }
            try {
                params.class_map[std::stoi(key)] = value.as_string();
            } catch (const std::exception&) {
                // Non-numeric key: skip; the class map is index -> name.
            }
        }
    } else {
        params.class_map = default_class_map();
    }
    return params;
}

// Cached ORT session so repeated extractions (multi-layer pipelines,
// determinism re-runs) do not reload the model each call.
struct OrtSessionCache {
    std::mutex mutex;
    std::string model_path;
    std::string execution_provider;
    std::unique_ptr<Ort::Session> session;
};

Ort::Env& ort_env() {
    static Ort::Env env(ORT_LOGGING_LEVEL_ERROR, "agbot_worldgen");
    return env;
}

struct TileInference {
    bool ok = false;
    std::string error;
    std::vector<std::uint8_t> classes;  // rect.width * rect.height
    std::vector<float> confidence;      // softmax prob of the argmax class
};

// Runs one tile through the model and produces per-pixel argmax class ids
// plus softmax confidences, nearest-neighbor upsampled from the logits grid
// to tile pixels. Content is placed top-left in the model input; padding is
// the normalized-zero color (channel mean).
TileInference run_tile(
    Ort::Session& session,
    const agbot::terrain::PngImage& image,
    const TileRect& rect,
    const SemSegParams& params) {
    TileInference out;
    const int side = params.input_size;
    const std::size_t plane = static_cast<std::size_t>(side) * static_cast<std::size_t>(side);
    std::vector<float> input(plane * 3u, 0.0f);
    for (int channel = 0; channel < 3; ++channel) {
        float* dst = input.data() + static_cast<std::size_t>(channel) * plane;
        const float mean = params.norm_mean[static_cast<std::size_t>(channel)];
        const float stddev = params.norm_std[static_cast<std::size_t>(channel)];
        for (int y = 0; y < std::min(side, rect.height); ++y) {
            const std::size_t src_row =
                (static_cast<std::size_t>(rect.y0 + y) * static_cast<std::size_t>(image.width) +
                 static_cast<std::size_t>(rect.x0)) * 4u;
            for (int x = 0; x < std::min(side, rect.width); ++x) {
                const float value01 =
                    image.rgba[src_row + static_cast<std::size_t>(x) * 4u +
                               static_cast<std::size_t>(channel)] / 255.0f;
                dst[static_cast<std::size_t>(y) * static_cast<std::size_t>(side) +
                    static_cast<std::size_t>(x)] = (value01 - mean) / stddev;
            }
        }
    }

    int classes = 0;
    int out_h = 0;
    int out_w = 0;
    std::vector<float> logits;
    try {
        Ort::AllocatorWithDefaultOptions allocator;
        const auto input_name = session.GetInputNameAllocated(0, allocator);
        const auto output_name = session.GetOutputNameAllocated(0, allocator);
        const std::array<std::int64_t, 4> shape = {
            1, 3, static_cast<std::int64_t>(side), static_cast<std::int64_t>(side)};
        Ort::MemoryInfo memory_info =
            Ort::MemoryInfo::CreateCpu(OrtArenaAllocator, OrtMemTypeDefault);
        Ort::Value input_value = Ort::Value::CreateTensor<float>(
            memory_info, input.data(), input.size(), shape.data(), shape.size());
        const char* input_names[] = {input_name.get()};
        const char* output_names[] = {output_name.get()};
        auto outputs = session.Run(
            Ort::RunOptions{nullptr}, input_names, &input_value, 1, output_names, 1);
        if (outputs.empty() || !outputs.front().IsTensor()) {
            out.error = "inference_failed:no_tensor_output";
            return out;
        }
        const auto info = outputs.front().GetTensorTypeAndShapeInfo();
        const std::vector<std::int64_t> out_shape = info.GetShape();
        if (out_shape.size() != 4 || out_shape[0] != 1) {
            out.error = "inference_failed:unexpected_output_rank";
            return out;
        }
        classes = static_cast<int>(out_shape[1]);
        out_h = static_cast<int>(out_shape[2]);
        out_w = static_cast<int>(out_shape[3]);
        if (classes <= 0 || classes > 255 || out_h <= 0 || out_w <= 0) {
            out.error = "inference_failed:unexpected_output_shape";
            return out;
        }
        const float* data = outputs.front().GetTensorData<float>();
        logits.assign(
            data,
            data + static_cast<std::size_t>(classes) * static_cast<std::size_t>(out_h) *
                static_cast<std::size_t>(out_w));
    } catch (const std::exception& error) {
        out.error = std::string("inference_failed:") + error.what();
        return out;
    } catch (...) {
        out.error = "inference_failed:unknown";
        return out;
    }

    // Argmax + softmax confidence per logits cell.
    const std::size_t cell_count =
        static_cast<std::size_t>(out_h) * static_cast<std::size_t>(out_w);
    std::vector<std::uint8_t> cell_class(cell_count, 0);
    std::vector<float> cell_conf(cell_count, 0.0f);
    for (std::size_t cell = 0; cell < cell_count; ++cell) {
        int best = 0;
        float best_logit = logits[cell];
        for (int c = 1; c < classes; ++c) {
            const float logit = logits[static_cast<std::size_t>(c) * cell_count + cell];
            if (logit > best_logit) {
                best_logit = logit;
                best = c;
            }
        }
        double denom = 0.0;
        for (int c = 0; c < classes; ++c) {
            denom += std::exp(
                static_cast<double>(logits[static_cast<std::size_t>(c) * cell_count + cell]) -
                static_cast<double>(best_logit));
        }
        cell_class[cell] = static_cast<std::uint8_t>(best);
        cell_conf[cell] = denom > 0.0 ? static_cast<float>(1.0 / denom) : 0.0f;
    }

    // Nearest-neighbor upsample of the content region to tile pixels.
    const std::size_t tile_pixels =
        static_cast<std::size_t>(rect.width) * static_cast<std::size_t>(rect.height);
    out.classes.resize(tile_pixels);
    out.confidence.resize(tile_pixels);
    for (int y = 0; y < rect.height; ++y) {
        const int ly = std::min(
            out_h - 1, static_cast<int>((static_cast<double>(y) + 0.5) * out_h / side));
        for (int x = 0; x < rect.width; ++x) {
            const int lx = std::min(
                out_w - 1, static_cast<int>((static_cast<double>(x) + 0.5) * out_w / side));
            const std::size_t cell =
                static_cast<std::size_t>(ly) * static_cast<std::size_t>(out_w) +
                static_cast<std::size_t>(lx);
            const std::size_t pixel =
                static_cast<std::size_t>(y) * static_cast<std::size_t>(rect.width) +
                static_cast<std::size_t>(x);
            out.classes[pixel] = cell_class[cell];
            out.confidence[pixel] = cell_conf[cell];
        }
    }
    out.ok = true;
    return out;
}

} // namespace

ExtractionResult OnnxSemSegExtractor::extract(const ExtractionContext& context) const {
    ExtractionResult result;
    result.algorithm_id = kId;
    result.params_hash = cfg::param_hash(context.params);

    const SemSegParams params = read_params(context.params);
    if (params.imagery_path.empty()) {
        result.error_code = "params_missing_imagery_path";
        result.error_detail = "onnx_semseg requires an 'imagery_path' parameter";
        return result;
    }
    if (!std::filesystem::exists(params.imagery_path)) {
        result.error_code = "imagery_not_found";
        result.error_detail = "cannot open imagery file: " + params.imagery_path;
        return result;
    }
    if (!std::filesystem::exists(params.model_path)) {
        result.error_code = "model_missing";
        result.error_detail = "ONNX model not found: " + params.model_path +
            " (run worldgen/tools/fetch_seg_model.sh)";
        return result;
    }
    if (params.class_map.empty()) {
        result.error_code = "class_map_empty";
        result.error_detail = "class_map maps no model class to a feature class";
        return result;
    }

    const agbot::terrain::PngImage image =
        agbot::terrain::decode_png_rgba_file(params.imagery_path);
    if (!image.ok || image.width <= 0 || image.height <= 0) {
        result.error_code = "imagery_decode_failed";
        result.error_detail = "PNG decode failed (" + image.error + "): " + params.imagery_path;
        return result;
    }

    // Session init/caching. ORT throws; keep exceptions inside this boundary.
    static OrtSessionCache cache;
    std::lock_guard<std::mutex> lock(cache.mutex);
    try {
        if (cache.session == nullptr || cache.model_path != params.model_path ||
            cache.execution_provider != params.execution_provider) {
            Ort::SessionOptions options;
            options.SetGraphOptimizationLevel(ORT_ENABLE_ALL);
            if (params.execution_provider == "coreml") {
                try {
                    std::unordered_map<std::string, std::string> coreml_options;
                    options.AppendExecutionProvider("CoreML", coreml_options);
                } catch (const std::exception&) {
                    options = Ort::SessionOptions();
                    options.SetGraphOptimizationLevel(ORT_ENABLE_ALL);
                }
            }
            cache.session = std::make_unique<Ort::Session>(
                ort_env(), params.model_path.c_str(), options);
            cache.model_path = params.model_path;
            cache.execution_provider = params.execution_provider;
        }
    } catch (const std::exception& error) {
        cache.session.reset();
        result.error_code = "ort_init_failed";
        result.error_detail = error.what();
        return result;
    }

    const std::vector<TileRect> tiles =
        plan_tiles(image.width, image.height, params.input_size, params.overlap_px);
    TileStitcher stitcher(image.width, image.height);
    for (const TileRect& rect : tiles) {
        const TileInference tile = run_tile(*cache.session, image, rect, params);
        if (!tile.ok) {
            result.error_code = "inference_failed";
            result.error_detail = tile.error;
            return result;
        }
        stitcher.commit(rect, tile.classes, tile.confidence);
    }

    ClassMask mask;
    mask.width = image.width;
    mask.height = image.height;
    mask.classes = stitcher.classes();

    PolygonizeOptions options;
    options.simplify_tol_m = params.simplify_tol_m;
    options.min_area_m2 = params.min_area_m2;

    const std::vector<float>& confidence = stitcher.confidence();
    for (const auto& [model_class, class_name] : params.class_map) {
        if (model_class < 0 || model_class > 255) {
            continue;
        }
        std::vector<std::int32_t> labels;
        const std::vector<RasterPolygon> polygons = polygonize_class(
            mask, context.aoi, static_cast<std::uint8_t>(model_class), options, &labels);
        if (polygons.empty()) {
            continue;
        }
        // Mean softmax probability per component for blob confidence.
        std::int32_t max_label = 0;
        for (const RasterPolygon& polygon : polygons) {
            max_label = std::max(max_label, polygon.component_label);
        }
        std::vector<double> prob_sum(static_cast<std::size_t>(max_label) + 1, 0.0);
        std::vector<int> prob_count(static_cast<std::size_t>(max_label) + 1, 0);
        for (std::size_t index = 0; index < labels.size(); ++index) {
            const std::int32_t label = labels[index];
            if (label >= 0 && label <= max_label) {
                prob_sum[static_cast<std::size_t>(label)] += confidence[index];
                ++prob_count[static_cast<std::size_t>(label)];
            }
        }

        for (const RasterPolygon& polygon : polygons) {
            ExtractedFeature feature;
            feature.class_name = class_name;
            feature.cls = feature_class_from_name(class_name);
            feature.exterior = polygon.exterior;
            feature.holes = polygon.holes;
            const std::size_t label = static_cast<std::size_t>(polygon.component_label);
            feature.confidence = prob_count[label] > 0
                ? prob_sum[label] / prob_count[label]
                : params.default_confidence;
            feature.source_id = "seg:" + std::to_string(model_class) + ":" +
                std::to_string(polygon.component_label);
            feature.attributes["model_class"] = std::to_string(model_class);
            feature.attributes["cells"] = std::to_string(polygon.cell_count);
            result.features.push_back(std::move(feature));
        }
    }

    result.ok = true;
    return result;
}

#endif // AGBOT_WORLDGEN_HAS_ONNX

} // namespace agbot::worldgen
