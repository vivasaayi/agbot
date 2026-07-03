#include "agbot_worldgen/Feature.hpp"
#include "agbot_worldgen/FeatureExtractor.hpp"
#include "agbot_worldgen/HeightResolver.hpp"
#include "agbot_worldgen/Crs.hpp"
#include "agbot_worldgen/SceneBridge.hpp"
#include "agbot_worldgen/SceneMesh.hpp"
#include "agbot_worldgen/WorldCompiler.hpp"
#include "agbot_worldgen/extractors/VectorImport.hpp"

#include "agbot_flight_sim/SceneSynthesis.hpp"

#include <algorithm>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <memory>
#include <sstream>
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
    using agbot::worldgen::HeightSource;
    const auto measured =
        agbot::worldgen::resolve_height(42.0, 100.0, 5.0, params);
    expect(near(measured.height_m, 42.0, 1e-9) && measured.source == HeightSource::Measured,
           "resolver prefers measured height (metres, no unit scale)");
    const auto from_attr =
        agbot::worldgen::resolve_height(std::nullopt, 100.0, 5.0, params);
    expect(near(from_attr.height_m, 30.48, 1e-9) && from_attr.source == HeightSource::Attribute,
           "resolver prefers attribute when no measured height");
    const auto from_levels =
        agbot::worldgen::resolve_height(std::nullopt, std::nullopt, 5.0, params);
    expect(near(from_levels.height_m, 15.0, 1e-9) && from_levels.source == HeightSource::Levels,
           "resolver falls back to levels");
    const auto from_default =
        agbot::worldgen::resolve_height(std::nullopt, std::nullopt, std::nullopt, params);
    expect(near(from_default.height_m, 4.0, 1e-9) && from_default.source == HeightSource::Default,
           "resolver falls back to default");
    const auto zero_measured =
        agbot::worldgen::resolve_height(0.0, std::nullopt, 2.0, params);
    expect(zero_measured.source == HeightSource::Levels,
           "non-positive measured height falls through");
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

// --- Gate 1: world-compile determinism ------------------------------------

// Hermetic terrain: a single synthetic_detail layer over the fixture AOI,
// dem_locked fusion, validation against itself. No terrarium tiles, no basemap
// draping, no file writes -> fully reproducible without network or cache.
const char* kHermeticTerrain = R"toml(
[pipeline]
target_gsd_m = 30.0
resolution = 48
aoi = { min_lat = 40.700, min_lon = -74.010, max_lat = 40.710, max_lon = -74.000 }

[[layer]]
algorithm = "synthetic_detail"
weight = 1.0
  [layer.params]
  amplitude_m = 8.0
  octaves = 4
  frequency = 8.0
  seed = 7
  confidence = 1.0

[fusion]
method = "dem_locked"

[validation]
enabled = true
reference_layer = 0
)toml";

agbot::worldgen::WorldCompileSpec gate1_spec() {
    agbot::worldgen::WorldCompileSpec spec;
    spec.seed = 4242;
    spec.terrain_config_toml = kHermeticTerrain;
    spec.terrain_license = "test-synthetic";
    spec.buildings_path = kFixturePath;
    spec.buildings_license = "test-fixture";
    spec.building_params["height_attr"] = std::string("height_roof");
    spec.building_params["height_units"] = std::string("feet");
    spec.building_params["base_elev_attr"] = std::string("ground_elevation");
    spec.building_params["base_units"] = std::string("feet");
    spec.building_params["levels_attr"] = std::string("num_floors");
    spec.building_params["id_attr"] = std::string("bin");
    spec.building_params["min_area_m2"] = 10.0;
    return spec;
}

void test_world_compiler_determinism() {
    const auto first = agbot::worldgen::compile_world(gate1_spec());
    expect(first.ok, "gate1 world compiles");
    if (!first.ok) {
        std::cout << "  gate1 compile error: " << first.error_code << " — "
                  << first.error_detail << "\n";
        return;
    }
    expect(first.manifest.world_hash != 0, "gate1 world_hash is nonzero");
    expect(first.manifest.tiles.size() == 1, "gate1 emits a single AOI tile");
    expect(first.manifest.sources.size() == 2, "gate1 records terrain + building sources");
    expect(
        !first.manifest.tiles.empty() && first.manifest.tiles.front().provenance.size() == 2,
        "gate1 tile carries terrain + building provenance");
    expect(first.manifest.quality.building_count == 5, "gate1 building count matches fixture");
    expect(first.buildings.size() == 5 && first.city.indices.size() % 3 == 0,
           "gate1 city mesh triangulated from fixture buildings");
    // Synthetic terrain is not authoritative -> Fallback, no-silent-zero reason.
    expect(!first.manifest.tiles.empty() &&
               first.manifest.tiles.front().elevation_state ==
                   agbot::worldgen::ElevationState::Fallback &&
               first.manifest.tiles.front().elevation_fallback_reason == "NO_AUTHORITATIVE_SOURCE",
           "gate1 non-authoritative terrain is labelled Fallback");

    const auto second = agbot::worldgen::compile_world(gate1_spec());
    expect(second.ok, "gate1 recompiles");
    expect(first.manifest.world_hash == second.manifest.world_hash,
           "gate1 world_hash reproducible across compiles");
    expect(
        first.manifest.tiles.front().content_hash == second.manifest.tiles.front().content_hash,
        "gate1 tile content hash reproducible");
    expect(first.manifest.to_json() == second.manifest.to_json(),
           "gate1 manifest JSON byte-identical across compiles");

    // The world seed is part of world identity but not of tile geometry.
    auto reseeded = gate1_spec();
    reseeded.seed = 9999;
    const auto diff = agbot::worldgen::compile_world(reseeded);
    expect(diff.ok && diff.manifest.world_hash != first.manifest.world_hash,
           "gate1 changing seed changes world_hash");
    expect(diff.ok && diff.manifest.tiles.front().content_hash ==
                          first.manifest.tiles.front().content_hash,
           "gate1 changing seed leaves geometry hash stable");

    // Provenance is auditable: every tile layer names a declared source.
    bool provenance_resolves = true;
    for (const auto& tile : first.manifest.tiles) {
        for (const auto& layer : tile.provenance) {
            const bool found = std::any_of(
                first.manifest.sources.begin(), first.manifest.sources.end(),
                [&layer](const agbot::worldgen::SourceSnapshot& s) {
                    return s.source_id == layer.source_id;
                });
            provenance_resolves = provenance_resolves && found;
        }
    }
    expect(provenance_resolves, "gate1 every layer provenance resolves to a source");
}

// --- M2: CRS / datum discipline --------------------------------------------

void test_crs_conversions() {
    namespace wg = agbot::worldgen;

    // EPSG string parsing.
    expect(wg::horizontal_crs_from_epsg("EPSG:2263") ==
               wg::HorizontalCrs::StatePlaneNyLongIslandFt,
           "crs parses EPSG:2263");
    expect(wg::horizontal_crs_from_epsg("epsg:26918") == wg::HorizontalCrs::Utm18N,
           "crs parses EPSG:26918 case-insensitively");
    expect(wg::horizontal_crs_from_epsg("") == wg::HorizontalCrs::Wgs84Lonlat,
           "empty crs defaults to WGS84");

    // EPSG:2263 analytic anchor: the false origin (984250 ft, 0 ft) is the
    // latitude/central-meridian origin 40°10'N, 74°00'W.
    const auto origin = wg::wgs84_from_state_plane_li_ft(984250.0, 0.0);
    expect(near(origin.latitude, 40.0 + 10.0 / 60.0, 1e-5), "2263 false origin -> lat 40d10m");
    expect(near(origin.longitude, -74.0, 1e-5), "2263 false origin -> lon -74");

    // Independent scale check: one degree of latitude north of the origin along
    // the central meridian is ~111 km ~ 364k ft of northing (LCC scale ~1 near
    // the standard parallels). A wrong cone constant would break this badly.
    double e_deg = 0.0;
    double n_deg = 0.0;
    wg::state_plane_li_ft_from_wgs84({40.0 + 10.0 / 60.0 + 1.0, -74.0, 0.0}, e_deg, n_deg);
    expect(near(e_deg, 984250.0, 1.0), "2263 northing runs up the central meridian");
    expect(n_deg > 360000.0 && n_deg < 370000.0, "2263 one-degree northing ~364k ft");

    // Round-trip a Manhattan point through EPSG:2263.
    const agbot::flight_sim::GeoCoordinate manhattan{40.7128, -74.0060, 0.0};
    double e_ft = 0.0;
    double n_ft = 0.0;
    wg::state_plane_li_ft_from_wgs84(manhattan, e_ft, n_ft);
    expect(e_ft > 975000.0 && e_ft < 990000.0, "2263 manhattan easting plausible");
    expect(n_ft > 185000.0 && n_ft < 210000.0, "2263 manhattan northing plausible");
    const auto back_2263 = wg::wgs84_from_state_plane_li_ft(e_ft, n_ft);
    expect(near(back_2263.latitude, manhattan.latitude, 1e-7) &&
               near(back_2263.longitude, manhattan.longitude, 1e-7),
           "2263 round-trips within 1e-7 deg");

    // UTM 18N round-trip + plausibility (zone CM -75, NYC ~1 deg east).
    const wg::ProjXY utm = wg::utm18n_from_wgs84(manhattan);
    expect(utm.x > 575000.0 && utm.x < 590000.0, "utm18n manhattan easting plausible");
    expect(utm.y > 4490000.0 && utm.y < 4520000.0, "utm18n manhattan northing plausible");
    const auto back_utm = wg::wgs84_from_utm18n(utm);
    expect(near(back_utm.latitude, manhattan.latitude, 1e-7) &&
               near(back_utm.longitude, manhattan.longitude, 1e-7),
           "utm18n round-trips within 1e-7 deg");

    // Independent TM meridian-arc check: one degree of latitude ~110.9 km.
    const wg::ProjXY utm_n = wg::utm18n_from_wgs84({41.7128, -74.0060, 0.0});
    expect(near(utm_n.y - utm.y, 111000.0, 600.0), "utm18n one-degree northing ~111 km");
}

void test_datum_discipline() {
    namespace wg = agbot::worldgen;
    using wg::VerticalDatum;

    expect(wg::vertical_datums_compatible(VerticalDatum::Navd88, VerticalDatum::Navd88Geoid18),
           "NAVD88 family is self-compatible");
    expect(!wg::vertical_datums_compatible(VerticalDatum::Navd88, VerticalDatum::Ellipsoidal),
           "orthometric vs ellipsoidal is incompatible");
    expect(wg::vertical_datums_compatible(VerticalDatum::Unknown, VerticalDatum::Ellipsoidal),
           "unknown datum defers (compatible)");
    expect(wg::vertical_datums_compatible(VerticalDatum::None, VerticalDatum::Navd88),
           "no-z source is compatible with anything");

    // Compiler rejects mixed datums when buildings contribute base elevations.
    auto spec = gate1_spec();
    spec.terrain_vertical_datum = "NAVD88";
    spec.buildings_vertical_datum = "ellipsoidal";
    const auto rejected = wg::compile_world(spec);
    expect(!rejected.ok && rejected.error_code == "mixed_vertical_datum",
           "compiler rejects orthometric terrain + ellipsoidal building base");

    spec.buildings_vertical_datum = "NAVD88";
    const auto accepted = wg::compile_world(spec);
    expect(accepted.ok, "compiler accepts compatible NAVD88 datums");
    expect(accepted.ok && accepted.manifest.crs_policy.vertical_datum == "NAVD88",
           "manifest records the resolved vertical datum");
    bool building_source_datum_ok = false;
    for (const auto& source : accepted.manifest.sources) {
        if (source.source_id == "buildings") {
            building_source_datum_ok = source.vertical_datum == "NAVD88";
        }
    }
    expect(building_source_datum_ok, "building source records NAVD88 vertical datum");
}

void test_2263_ingest() {
    namespace wg = agbot::worldgen;
    // A ~50 m square around a Lower Manhattan point, expressed in EPSG:2263 US
    // survey feet. If the ingest transform is skipped, the feet coordinates
    // read as absurd lon/lat and the feature is dropped by the AOI filter.
    const agbot::flight_sim::GeoCoordinate center{40.7075, -74.0050, 0.0};
    double ec = 0.0;
    double nc = 0.0;
    wg::state_plane_li_ft_from_wgs84(center, ec, nc);
    const double d = 82.0; // ~25 m in feet

    std::ostringstream json;
    json << std::fixed << std::setprecision(4);
    json << R"({"type":"FeatureCollection","features":[{"type":"Feature",)"
         << R"("properties":{"bin":"SP1","height_roof":100.0,"ground_elevation":10.0},)"
         << R"("geometry":{"type":"Polygon","coordinates":[[)"
         << "[" << ec - d << "," << nc - d << "],"
         << "[" << ec + d << "," << nc - d << "],"
         << "[" << ec + d << "," << nc + d << "],"
         << "[" << ec - d << "," << nc + d << "],"
         << "[" << ec - d << "," << nc - d << "]"
         << "]]}}]}";

    const std::filesystem::path fixture =
        std::filesystem::temp_directory_path() / "agbot_crs_2263_fixture.geojson";
    {
        std::ofstream out(fixture, std::ios::binary);
        out << json.str();
    }

    agbot::config::ParamTable params;
    params["path"] = fixture.string();
    params["source_crs"] = std::string("EPSG:2263");
    params["height_attr"] = std::string("height_roof");
    params["height_units"] = std::string("feet");
    params["id_attr"] = std::string("bin");
    params["min_area_m2"] = 10.0;

    const agbot::flight_sim::GeoBounds aoi{center.latitude - 0.002, center.longitude - 0.002,
                                           center.latitude + 0.002, center.longitude + 0.002};
    const auto extractor = agbot::worldgen::extractor_registry().create("vector_import");
    const auto result = extractor->extract({aoi, params});
    expect(result.ok, "2263 ingest extraction succeeds");
    expect(result.features.size() == 1, "2263 square lands one feature inside the AOI");
    if (result.features.size() == 1) {
        double lat_sum = 0.0;
        double lon_sum = 0.0;
        for (const auto& p : result.features.front().exterior) {
            lat_sum += p.latitude;
            lon_sum += p.longitude;
        }
        const double n = static_cast<double>(result.features.front().exterior.size());
        expect(near(lat_sum / n, center.latitude, 5e-4) &&
                   near(lon_sum / n, center.longitude, 5e-4),
               "2263 ingested footprint centroid matches the source WGS84 point");
    }
    std::error_code ec_rm;
    std::filesystem::remove(fixture, ec_rm);
}

void test_elevation_state_authoritative() {
    namespace wg = agbot::worldgen;
    const std::string dem_fixture =
        std::string(WORLDGEN_SOURCE_DIR) + "/../terrain_engine/tests/fixtures/dem_128.tif";
    if (!std::filesystem::exists(dem_fixture)) {
        std::cout << "SKIP elevation-state authoritative (DEM fixture absent)\n";
        return;
    }
    // AOI strictly inside the DEM fixture bounds (40.705..40.715 / -74.015..-74.005)
    // so the authoritative DEM fully covers it.
    const std::string terrain_toml =
        "[pipeline]\n"
        "target_gsd_m = 30.0\n"
        "resolution = 32\n"
        "aoi = { min_lat = 40.706, min_lon = -74.014, max_lat = 40.714, max_lon = -74.006 }\n"
        "[[layer]]\n"
        "algorithm = \"dem_fusion\"\n"
        "weight = 1.0\n"
        "  [layer.params]\n"
        "  source = \"geotiff\"\n"
        "  path = \"" + dem_fixture + "\"\n"
        "  resample = \"bilinear\"\n"
        "[fusion]\n"
        "method = \"dem_locked\"\n"
        "[validation]\n"
        "enabled = true\n"
        "reference_layer = 0\n";

    wg::WorldCompileSpec spec;
    spec.seed = 7;
    spec.terrain_config_toml = terrain_toml;
    spec.terrain_authoritative = true;
    spec.terrain_vertical_datum = "NAVD88";
    spec.buildings_path = kFixturePath;
    spec.building_params["id_attr"] = std::string("bin");
    spec.building_params["min_area_m2"] = 10.0;

    const auto world = wg::compile_world(spec);
    expect(world.ok, "authoritative compile succeeds from geotiff DEM");
    if (!world.ok) {
        std::cout << "  error: " << world.error_code << " — " << world.error_detail << "\n";
        return;
    }
    const auto& tile = world.manifest.tiles.front();
    expect(tile.elevation_state == wg::ElevationState::Authoritative,
           "geotiff DEM yields Authoritative elevation state");
    expect(tile.elevation_fallback_reason.empty(),
           "fully-covered authoritative tile has no fallback reason");
    const auto& q = world.manifest.quality;
    expect(q.terrain_cell_count == 32 * 32, "authoritative terrain cell count");
    expect(q.terrain_nodata_cells == 0 && q.terrain_authoritative_cells == q.terrain_cell_count,
           "authoritative AOI is fully covered (no silent-zero)");
    expect(world.manifest.to_json().find("\"elevation_state\": \"authoritative\"") !=
               std::string::npos,
           "manifest serializes the authoritative elevation state");
}

void test_gate3_building_quality() {
    namespace wg = agbot::worldgen;
    // The building fixture yields 5 features spanning all height sources
    // (attr/levels/default) with one holed footprint (a courtyard).
    const auto world = wg::compile_world(gate1_spec());
    expect(world.ok, "gate3 compile succeeds");
    if (!world.ok) {
        return;
    }
    const auto& q = world.manifest.quality;
    expect(q.building_count == 5, "gate3 building count");
    expect(q.height_from_measured + q.height_from_attribute + q.height_from_levels +
                   q.height_from_default ==
               q.building_count,
           "gate3 every building has a ranked height provenance");
    expect(q.height_from_attribute >= 1 && q.height_from_levels >= 1 && q.height_from_default >= 1,
           "gate3 fixture exercises attr/levels/default height sources");
    expect(q.building_with_courtyard_count >= 1, "gate3 courtyard (hole) preserved and counted");
    expect(q.building_footprint_area_m2 > 0.0, "gate3 footprint area accumulated");
    expect(q.median_building_height_m > 0.0, "gate3 median height computed");
    expect(world.manifest.to_json().find("\"height_source\": {\"measured\":") != std::string::npos,
           "gate3 manifest serializes the height-source breakdown");
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
    test_world_compiler_determinism();
    test_crs_conversions();
    test_datum_discipline();
    test_2263_ingest();
    test_elevation_state_authoritative();
    test_gate3_building_quality();

    if (failures > 0) {
        std::cout << failures << " test(s) failed\n";
        return 1;
    }
    std::cout << "all worldgen tests passed\n";
    return 0;
}
