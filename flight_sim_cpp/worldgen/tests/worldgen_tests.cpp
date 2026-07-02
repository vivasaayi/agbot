#include "agbot_worldgen/Feature.hpp"
#include "agbot_worldgen/FeatureExtractor.hpp"
#include "agbot_worldgen/HeightResolver.hpp"
#include "agbot_worldgen/SceneBridge.hpp"
#include "agbot_worldgen/SceneMesh.hpp"
#include "agbot_worldgen/extractors/VectorImport.hpp"

#include "agbot_flight_sim/SceneSynthesis.hpp"

#include <algorithm>
#include <cmath>
#include <filesystem>
#include <iostream>
#include <memory>
#include <string>
#include <vector>

namespace {

int failures = 0;

void expect(bool condition, const std::string& label) {
    if (condition) {
        std::cout << "PASS " << label << "\n";
    } else {
        std::cout << "FAIL " << label << "\n";
        ++failures;
    }
}

bool near(double actual, double expected, double tolerance) {
    return std::abs(actual - expected) <= tolerance;
}

const std::string kFixturePath =
    std::string(WORLDGEN_SOURCE_DIR) + "/tests/fixtures/buildings_fixture.geojson";
const std::string kManhattanPath =
    std::string(WORLDGEN_SOURCE_DIR) + "/../data/worldgen/manhattan_buildings.geojson";

agbot::flight_sim::GeoBounds fixture_aoi() {
    return {40.700, -74.010, 40.710, -74.000};
}

agbot::config::ParamTable fixture_params() {
    agbot::config::ParamTable params;
    params["path"] = kFixturePath;
    params["height_attr"] = "height_roof";
    params["height_units"] = "feet";
    params["base_elev_attr"] = "ground_elevation";
    params["base_units"] = "feet";
    params["levels_attr"] = "num_floors";
    params["default_level_height_m"] = 3.0;
    params["default_height_m"] = 4.0;
    params["class_attr"] = "";
    params["default_class"] = "building";
    params["id_attr"] = "bin";
    params["min_area_m2"] = 10.0;
    params["simplify_tol_m"] = 0.0;
    params["max_features"] = 0;
    return params;
}

agbot::worldgen::ExtractionResult run_extract(const agbot::config::ParamTable& params) {
    const std::unique_ptr<agbot::worldgen::FeatureExtractor> extractor =
        agbot::worldgen::extractor_registry().create("vector_import");
    if (!extractor) {
        return {};
    }
    const agbot::worldgen::ExtractionContext context{fixture_aoi(), params};
    return extractor->extract(context);
}

const agbot::worldgen::ExtractedFeature* find_feature(
    const std::vector<agbot::worldgen::ExtractedFeature>& features,
    const std::string& source_id) {
    const auto it = std::find_if(
        features.begin(), features.end(),
        [&source_id](const agbot::worldgen::ExtractedFeature& feature) {
            return feature.source_id == source_id;
        });
    return it != features.end() ? &(*it) : nullptr;
}

std::string height_source_of(const agbot::worldgen::ExtractedFeature& feature) {
    const auto it = feature.attributes.find("height_source");
    return it != feature.attributes.end() ? it->second : "";
}

void test_registry() {
    auto& registry = agbot::worldgen::extractor_registry();
    expect(registry.contains("vector_import"), "registry contains vector_import");
    const auto extractor = registry.create("vector_import");
    expect(extractor != nullptr, "registry creates vector_import");
    expect(extractor && extractor->id() == "vector_import", "extractor id matches");
    expect(
        extractor && !extractor->produces().empty(), "extractor declares produced classes");
}

void test_error_paths() {
    agbot::config::ParamTable params;
    const auto missing = run_extract(params);
    expect(!missing.ok && missing.error_code == "params_missing_path", "missing path reason-coded");

    params["path"] = std::string(WORLDGEN_SOURCE_DIR) + "/tests/fixtures/nope.geojson";
    const auto absent = run_extract(params);
    expect(!absent.ok && absent.error_code == "file_not_found", "absent file reason-coded");
}

void test_fixture_extraction() {
    const auto result = run_extract(fixture_params());
    expect(result.ok, "fixture extraction succeeds");
    expect(result.algorithm_id == "vector_import", "result records algorithm id");
    expect(result.params_hash != 0, "result records params hash");

    // 6 fixture inputs: tiny building filtered by min_area, far building
    // dropped by the AOI bbox filter, MultiPolygon splits into two features.
    expect(result.features.size() == 5, "fixture yields 5 features");

    const auto* bldg_a = find_feature(result.features, "1000001");
    expect(bldg_a != nullptr, "bldg_a present");
    if (bldg_a != nullptr) {
        expect(near(bldg_a->height_m.value_or(0.0), 30.48, 1e-9), "100 ft converts to 30.48 m");
        expect(near(bldg_a->base_elev_m.value_or(0.0), 3.048, 1e-9), "10 ft base converts to 3.048 m");
        expect(height_source_of(*bldg_a) == "attr", "attr height precedence recorded");
        expect(bldg_a->cls == agbot::worldgen::FeatureClass::Building, "default class is building");
        expect(bldg_a->exterior.size() == 5, "closing point dropped, collinear point kept");
    }

    const auto* bldg_hole = find_feature(result.features, "1000002");
    expect(bldg_hole != nullptr, "bldg_hole present");
    if (bldg_hole != nullptr) {
        expect(bldg_hole->holes.size() == 1, "hole preserved");
        expect(bldg_hole->holes.front().size() == 4, "hole ring parsed");
    }

    const auto* multi_p0 = find_feature(result.features, "1000003:p0");
    const auto* multi_p1 = find_feature(result.features, "1000003:p1");
    expect(multi_p0 != nullptr && multi_p1 != nullptr, "MultiPolygon splits into two features");
    if (multi_p0 != nullptr) {
        expect(near(multi_p0->height_m.value_or(0.0), 15.0, 1e-9), "levels fallback: 5 x 3 m");
        expect(height_source_of(*multi_p0) == "levels", "levels height precedence recorded");
    }

    const auto* bldg_default = find_feature(result.features, "1000005");
    expect(bldg_default != nullptr, "default-height building present");
    if (bldg_default != nullptr) {
        expect(near(bldg_default->height_m.value_or(0.0), 4.0, 1e-9), "default height applied");
        expect(height_source_of(*bldg_default) == "default", "default height precedence recorded");
    }

    expect(find_feature(result.features, "1000004") == nullptr, "tiny footprint filtered by min_area");
    expect(find_feature(result.features, "1000006") == nullptr, "feature outside aoi dropped");
}

void test_param_variants() {
    auto params = fixture_params();
    params["min_area_m2"] = 0.0;
    const auto no_area_filter = run_extract(params);
    expect(no_area_filter.ok && no_area_filter.features.size() == 6, "min_area 0 keeps tiny footprint");

    params = fixture_params();
    params["max_features"] = 2;
    const auto capped = run_extract(params);
    expect(capped.ok && capped.features.size() == 2, "max_features caps output");

    params = fixture_params();
    params["simplify_tol_m"] = 0.5;
    const auto simplified = run_extract(params);
    const auto* bldg_a =
        simplified.ok ? find_feature(simplified.features, "1000001") : nullptr;
    expect(bldg_a != nullptr && bldg_a->exterior.size() == 4, "simplify removes collinear point");
}

void test_height_resolver() {
    const agbot::worldgen::HeightResolverParams params{0.3048, 3.0, 4.0};
    const auto from_attr = agbot::worldgen::resolve_height(100.0, 5.0, params);
    expect(
        near(from_attr.height_m, 30.48, 1e-9) &&
            from_attr.source == agbot::worldgen::HeightSource::Attribute,
        "resolver prefers attribute");
    const auto from_levels = agbot::worldgen::resolve_height(std::nullopt, 5.0, params);
    expect(
        near(from_levels.height_m, 15.0, 1e-9) &&
            from_levels.source == agbot::worldgen::HeightSource::Levels,
        "resolver falls back to levels");
    const auto from_default = agbot::worldgen::resolve_height(std::nullopt, std::nullopt, params);
    expect(
        near(from_default.height_m, 4.0, 1e-9) &&
            from_default.source == agbot::worldgen::HeightSource::Default,
        "resolver falls back to default");
    const auto zero_attr = agbot::worldgen::resolve_height(0.0, 2.0, params);
    expect(
        zero_attr.source == agbot::worldgen::HeightSource::Levels,
        "non-positive attribute falls through");
}

void test_scene_bridge() {
    const auto result = run_extract(fixture_params());
    const auto input = agbot::worldgen::to_scene_input(result.features, fixture_aoi(), 42);
    expect(input.buildings.size() == 5, "scene input carries 5 buildings");
    expect(input.profile.asserted, "scene profile asserted");
    expect(input.seed == 42, "scene seed forwarded");

    const auto manifest = agbot::worldgen::scene_manifest_for(result.features, fixture_aoi(), 42);
    expect(
        manifest.status == agbot::flight_sim::SceneSynthesisStatus::Ready,
        "scene manifest is Ready");
    expect(manifest.objects.size() == 5, "scene manifest has 5 objects");
    bool local_footprints_ok = !manifest.objects.empty();
    for (const auto& object : manifest.objects) {
        local_footprints_ok = local_footprints_ok &&
            object.footprint_local_m.size() == object.footprint_geo.size() &&
            object.footprint_local_m.size() >= 3;
    }
    expect(local_footprints_ok, "scene objects carry local footprints");
}

agbot::worldgen::ExtractedFeature donut_feature(const std::string& source_id, double lon_offset) {
    // Square exterior (4 points) with a square hole (4 points) near the
    // fixture AOI center; heights chosen so top = 3 + 30 = 33 m.
    agbot::worldgen::ExtractedFeature feature;
    feature.cls = agbot::worldgen::FeatureClass::Building;
    feature.class_name = "building";
    feature.source_id = source_id;
    feature.height_m = 30.0;
    feature.base_elev_m = 3.0;
    const double lon = -74.005 + lon_offset;
    feature.exterior = {
        {40.7048, lon, 0.0},
        {40.7048, lon + 0.0005, 0.0},
        {40.7052, lon + 0.0005, 0.0},
        {40.7052, lon, 0.0},
    };
    feature.holes.push_back({
        {40.70495, lon + 0.0002, 0.0},
        {40.70495, lon + 0.0003, 0.0},
        {40.70505, lon + 0.0003, 0.0},
        {40.70505, lon + 0.0002, 0.0},
    });
    return feature;
}

void test_mesh_builder() {
    const agbot::flight_sim::GeoCoordinate origin = fixture_aoi().center();
    agbot::worldgen::SceneMeshParams params;

    std::vector<agbot::worldgen::ExtractedFeature> features{donut_feature("donut", 0.0)};
    const auto mesh = agbot::worldgen::build_city_mesh(features, origin, params);

    // Watertight-ish counts: 8 ring vertices, 1 hole -> 8 cap triangles
    // (N + 2H - 2), 8 wall edges -> 16 wall triangles.
    const std::size_t triangle_count = mesh.indices.size() / 3;
    expect(triangle_count == 24, "donut mesh has 8 cap + 16 wall triangles");
    expect(mesh.vertices.size() == 8 + 8 * 4, "donut mesh vertex count");
    expect(mesh.batches.size() == 1, "single tile batch");
    if (!mesh.batches.empty()) {
        expect(mesh.batches.front().index_count == mesh.indices.size(), "batch spans all indices");
        expect(
            near(mesh.batches.front().aabb.max[1], 33.0f, 1e-3) &&
                near(mesh.batches.front().aabb.min[1], 3.0f, 1e-3),
            "batch aabb spans base..base+height");
    }

    bool up_cap_found = false;
    bool normals_unit = true;
    for (const auto& vertex : mesh.vertices) {
        const double length = std::sqrt(
            static_cast<double>(vertex.normal[0]) * vertex.normal[0] +
            static_cast<double>(vertex.normal[1]) * vertex.normal[1] +
            static_cast<double>(vertex.normal[2]) * vertex.normal[2]);
        normals_unit = normals_unit && near(length, 1.0, 1e-4);
        up_cap_found = up_cap_found || vertex.normal[1] > 0.99f;
    }
    expect(normals_unit, "mesh normals are unit length");
    expect(up_cap_found, "cap normals face up");
    expect(
        std::all_of(
            mesh.vertices.begin(), mesh.vertices.end(),
            [](const agbot::worldgen::CityVertex& vertex) { return vertex.class_id == 1; }),
        "building class id assigned");

    // Deterministic: identical inputs hash identically, input order ignored.
    std::vector<agbot::worldgen::ExtractedFeature> pair_a{
        donut_feature("a", 0.0), donut_feature("b", 0.01)};
    std::vector<agbot::worldgen::ExtractedFeature> pair_b{
        donut_feature("b", 0.01), donut_feature("a", 0.0)};
    const auto mesh_a = agbot::worldgen::build_city_mesh(pair_a, origin, params);
    const auto mesh_b = agbot::worldgen::build_city_mesh(pair_b, origin, params);
    expect(
        agbot::worldgen::city_mesh_vertex_hash(mesh_a) ==
            agbot::worldgen::city_mesh_vertex_hash(mesh_b),
        "mesh vertex hash deterministic across runs and input order");
    expect(mesh_a.batches.size() == 2, "0.01 deg offset splits into two 500 m tiles");
}

void test_manhattan_integration() {
    if (!std::filesystem::exists(kManhattanPath)) {
        std::cout << "SKIP manhattan integration (data file absent: " << kManhattanPath << ")\n";
        return;
    }
    agbot::config::ParamTable params;
    params["path"] = kManhattanPath;
    params["height_attr"] = "height_roof";
    params["height_units"] = "feet";
    params["base_elev_attr"] = "ground_elevation";
    params["base_units"] = "feet";
    params["id_attr"] = "bin";
    params["min_area_m2"] = 10.0;

    const agbot::flight_sim::GeoBounds aoi{40.700, -74.020, 40.740, -73.980};
    const std::unique_ptr<agbot::worldgen::FeatureExtractor> extractor =
        agbot::worldgen::extractor_registry().create("vector_import");
    const agbot::worldgen::ExtractionContext context{aoi, params};
    const auto result = extractor->extract(context);

    expect(result.ok, "manhattan extraction succeeds");
    expect(result.features.size() > 1000, "manhattan yields >1000 buildings");

    double max_height = 0.0;
    std::size_t with_attr_height = 0;
    for (const auto& feature : result.features) {
        const double height = feature.height_m.value_or(0.0);
        max_height = std::max(max_height, height);
        if (height_source_of(feature) == "attr") {
            ++with_attr_height;
        }
    }
    expect(
        max_height >= 200.0 && max_height <= 400.0,
        "lower manhattan max building height in 200..400 m");
    expect(
        with_attr_height > result.features.size() / 2,
        "most manhattan heights come from height_roof");

    const auto mesh = agbot::worldgen::build_city_mesh(
        result.features, aoi.center(), agbot::worldgen::SceneMeshParams{});
    expect(mesh.batches.size() > 10, "manhattan mesh splits into many tiles");
    expect(!mesh.indices.empty() && mesh.indices.size() % 3 == 0, "manhattan mesh triangulated");
    std::cout << "  manhattan: " << result.features.size() << " buildings, max height "
              << max_height << " m, " << mesh.vertices.size() << " vertices, "
              << mesh.indices.size() / 3 << " triangles, " << mesh.batches.size() << " batches\n";
}

} // namespace

int main() {
    test_registry();
    test_error_paths();
    test_fixture_extraction();
    test_param_variants();
    test_height_resolver();
    test_scene_bridge();
    test_mesh_builder();
    test_manhattan_integration();

    if (failures > 0) {
        std::cout << failures << " test(s) failed\n";
        return 1;
    }
    std::cout << "all worldgen tests passed\n";
    return 0;
}
