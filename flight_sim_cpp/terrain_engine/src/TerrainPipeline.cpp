#include "agbot_terrain/TerrainPipeline.hpp"

#include "agbot_config/Toml.hpp"
#include "agbot_terrain/ElevationEstimator.hpp"
#include "agbot_terrain/Fusion.hpp"

#include <algorithm>
#include <cmath>

namespace agbot::terrain {
namespace {

namespace cfg = agbot::config;

bool parse_aoi(const cfg::ParamTable& pipeline, GeoBounds& aoi, std::string& error) {
    const cfg::ParamTable* aoi_table = cfg::find_table(pipeline, "aoi");
    if (aoi_table == nullptr) {
        error = "pipeline_missing_aoi";
        return false;
    }
    aoi.min_latitude = cfg::double_or(*aoi_table, "min_lat", 0.0);
    aoi.min_longitude = cfg::double_or(*aoi_table, "min_lon", 0.0);
    aoi.max_latitude = cfg::double_or(*aoi_table, "max_lat", 0.0);
    aoi.max_longitude = cfg::double_or(*aoi_table, "max_lon", 0.0);
    if (aoi.max_latitude <= aoi.min_latitude || aoi.max_longitude <= aoi.min_longitude) {
        error = "pipeline_degenerate_aoi";
        return false;
    }
    return true;
}

} // namespace

PipelineResult run_terrain_pipeline(const cfg::ParamTable& config) {
    PipelineResult result;
    result.param_hash = cfg::param_hash(config);
    result.validation.param_hash = result.param_hash;

    const cfg::ParamTable* pipeline = cfg::find_table(config, "pipeline");
    if (pipeline == nullptr) {
        result.error = "config_missing_pipeline_table";
        return result;
    }

    ImageryBundle bundle;
    if (!parse_aoi(*pipeline, bundle.aoi, result.error)) {
        return result;
    }
    bundle.target_gsd_m = cfg::double_or(*pipeline, "target_gsd_m", 10.0);
    const int resolution = static_cast<int>(cfg::integer_or(*pipeline, "resolution", 0));
    if (resolution > 0) {
        bundle.grid_width = resolution;
        bundle.grid_height = resolution;
    }

    const cfg::ParamArray* layer_entries = cfg::find_array(config, "layer");
    if (layer_entries == nullptr || layer_entries->empty()) {
        result.error = "config_missing_layers";
        return result;
    }

    const auto& registry = estimator_registry();
    std::vector<FusionLayer> layers;
    layers.reserve(layer_entries->size());
    for (const cfg::ParamValue& entry : *layer_entries) {
        if (!entry.is_table()) {
            result.error = "config_layer_not_a_table";
            return result;
        }
        const cfg::ParamTable& layer_table = entry.as_table();
        const std::string algorithm = cfg::string_or(layer_table, "algorithm", "");
        const double weight = cfg::double_or(layer_table, "weight", 1.0);
        const bool optional = cfg::bool_or(layer_table, "optional", false);
        auto estimator = registry.create(algorithm);
        if (estimator == nullptr) {
            result.error = "config_unknown_estimator:" + algorithm;
            return result;
        }
        cfg::ParamTable params;
        if (const cfg::ParamTable* layer_params = cfg::find_table(layer_table, "params")) {
            params = *layer_params;
        }
        if (!estimator->accepts(bundle)) {
            if (optional) {
                continue;
            }
            result.error = "estimator_rejects_bundle:" + algorithm;
            return result;
        }
        EstimateResult estimate = estimator->estimate(bundle, params);
        if (!estimate.ok) {
            if (optional) {
                continue;
            }
            result.error = "estimator_failed:" + algorithm + ":" + estimate.error;
            return result;
        }
        layers.push_back(FusionLayer { std::move(estimate.field), weight });
    }
    if (layers.empty()) {
        result.error = "config_no_usable_layers";
        return result;
    }

    std::string fusion_method = "dem_locked";
    cfg::ParamTable fusion_params;
    if (const cfg::ParamTable* fusion_table = cfg::find_table(config, "fusion")) {
        fusion_method = cfg::string_or(*fusion_table, "method", fusion_method);
        fusion_params = *fusion_table;
    }
    FusionResult fused = FusionEngine::fuse(fusion_method, layers, fusion_params);
    if (!fused.ok) {
        result.error = fused.error;
        return result;
    }
    result.fused = std::move(fused.field);
    result.validation.fused_raster_hash = raster_hash(result.fused.elevation);

    if (const cfg::ParamTable* validation = cfg::find_table(config, "validation")) {
        if (cfg::bool_or(*validation, "enabled", true)) {
            const std::int64_t reference_layer =
                cfg::integer_or(*validation, "reference_layer", 0);
            if (reference_layer < 0 ||
                reference_layer >= static_cast<std::int64_t>(layers.size())) {
                result.error = "validation_reference_layer_out_of_range";
                return result;
            }
            const FusionLayer& reference = layers[static_cast<std::size_t>(reference_layer)];
            result.validation.reference_name = reference.field.source_algorithm;
            result.validation.metrics =
                compute_metrics(result.fused.elevation, reference.field.elevation);
            if (result.validation.metrics.sample_count == 0) {
                result.validation.error = "validation_no_samples";
            } else {
                result.validation.ok = true;
            }
            const std::string output_json = cfg::string_or(*validation, "output_json", "");
            if (!output_json.empty()) {
                std::string write_error;
                if (!write_validation_json(output_json, result.validation, &write_error)) {
                    result.error = write_error;
                    return result;
                }
            }
        }
    }

    result.ok = true;
    return result;
}

PipelineResult run_terrain_pipeline_file(const std::filesystem::path& toml_path) {
    const cfg::TomlParseResult parsed = cfg::parse_toml_file(toml_path);
    if (!parsed.ok) {
        PipelineResult result;
        result.error = "config_parse_failed:" + parsed.error;
        return result;
    }
    return run_terrain_pipeline(parsed.root);
}

MeshResult mesh_from_heightfield(const HeightField& field, double vertical_scale) {
    MeshResult result;
    if (!field.valid()) {
        result.error = "mesh_invalid_heightfield";
        return result;
    }
    if (field.elevation.width != field.elevation.height || field.elevation.width < 2) {
        result.error = "mesh_requires_square_grid";
        return result;
    }
    std::vector<float> heightmap = field.elevation.values;
    for (float& value : heightmap) {
        if (Raster::is_nodata(value)) {
            value = 0.0f;
        }
    }
    result.mesh = agbot::flight_sim::build_terrain_mesh(
        heightmap,
        field.elevation.width,
        field.elevation.bounds.width_m(),
        field.elevation.bounds.height_m(),
        vertical_scale);
    result.ok = true;
    return result;
}

} // namespace agbot::terrain
