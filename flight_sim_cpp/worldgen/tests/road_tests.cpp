#include "agbot_worldgen/Feature.hpp"
#include "agbot_worldgen/FeatureExtractor.hpp"
#include "agbot_worldgen/RoadMesh.hpp"
#include "agbot_worldgen/RoadNetwork.hpp"
#include "agbot_worldgen/extractors/RoadImport.hpp"

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

const std::string kOverpassFixture =
    std::string(WORLDGEN_SOURCE_DIR) + "/tests/fixtures/roads_fixture.json";
const std::string kGeojsonFixture =
    std::string(WORLDGEN_SOURCE_DIR) + "/tests/fixtures/roads_fixture.geojson";
const std::string kManhattanRoads =
    std::string(WORLDGEN_SOURCE_DIR) + "/../data/worldgen/manhattan_roads.json";

// Fixture grid lives around (0, 0): one degree == 111320 m in both axes, so
// the 0.002 deg block size is exactly 222.64 m.
constexpr double kBlockM = 222.64;

agbot::flight_sim::GeoBounds fixture_aoi() {
    return {-0.01, -0.01, 0.01, 0.01};
}

agbot::worldgen::ExtractionResult run_road_import(const agbot::config::ParamTable& params) {
    const std::unique_ptr<agbot::worldgen::FeatureExtractor> extractor =
        agbot::worldgen::extractor_registry().create("road_import");
    if (!extractor) {
        return {};
    }
    return extractor->extract({fixture_aoi(), params});
}

const agbot::worldgen::ExtractedFeature* find_feature(
    const std::vector<agbot::worldgen::ExtractedFeature>& features,
    const std::string& source_id) {
    const auto it = std::find_if(
        features.begin(), features.end(),
        [&source_id](const agbot::worldgen::ExtractedFeature& feature) {
            return feature.source_id == source_id;
        });
    return it == features.end() ? nullptr : &*it;
}

std::string attribute_or(
    const agbot::worldgen::ExtractedFeature& feature,
    const std::string& key,
    const std::string& fallback) {
    const auto it = feature.attributes.find(key);
    return it == feature.attributes.end() ? fallback : it->second;
}

const agbot::flight_sim::GeoCoordinate kOrigin{0.0, 0.0, 0.0};

void test_registry() {
    expect(agbot::worldgen::extractor_registry().contains("road_import"),
           "registry contains road_import");
    const auto extractor = agbot::worldgen::extractor_registry().create("road_import");
    expect(extractor && extractor->id() == "road_import", "extractor id matches");
    expect(extractor && extractor->produces() ==
               std::vector<agbot::worldgen::FeatureClass>{agbot::worldgen::FeatureClass::Road},
           "extractor produces Road");
}

void test_reason_codes() {
    agbot::config::ParamTable params;
    const auto missing = run_road_import(params);
    expect(!missing.ok && missing.error_code == "params_missing_path",
           "missing path is reason-coded");

    params["path"] = std::string(WORLDGEN_SOURCE_DIR) + "/does_not_exist.json";
    const auto absent = run_road_import(params);
    expect(!absent.ok && absent.error_code == "file_not_found", "missing file is reason-coded");
}

void test_overpass_import() {
    agbot::config::ParamTable params;
    params["path"] = kOverpassFixture;
    const auto result = run_road_import(params);
    expect(result.ok, "overpass fixture imports");
    // 101..107 pass; 108 is footway (filtered), 109 is outside the AOI.
    expect(result.features.size() == 7, "overpass fixture yields 7 roads");

    const auto* a_street = find_feature(result.features, "way:101");
    expect(a_street != nullptr, "A Street imported");
    if (a_street != nullptr) {
        expect(a_street->cls == agbot::worldgen::FeatureClass::Road, "A Street class is Road");
        expect(a_street->exterior.size() == 3, "A Street keeps 3 polyline points");
        expect(attribute_or(*a_street, "geometry_type", "") == "polyline",
               "A Street marked as open polyline");
        expect(attribute_or(*a_street, "highway", "") == "residential",
               "A Street highway class");
        expect(attribute_or(*a_street, "oneway", "") == "no", "A Street two-way");
        expect(attribute_or(*a_street, "name", "") == "A Street", "A Street name kept");
    }
    const auto* second_ave = find_feature(result.features, "way:104");
    expect(second_ave != nullptr &&
               attribute_or(*second_ave, "oneway", "") == "yes",
           "Second Avenue normalized oneway=yes");
    const auto* spur = find_feature(result.features, "way:106");
    expect(spur != nullptr && attribute_or(*spur, "lanes", "") == "2", "Spur lanes kept");
    expect(find_feature(result.features, "way:108") == nullptr, "footway filtered out");
    expect(find_feature(result.features, "way:109") == nullptr, "out-of-AOI way dropped");
}

void test_highway_filter_param() {
    agbot::config::ParamTable params;
    params["path"] = kOverpassFixture;
    params["highway_filter"] = agbot::config::ParamArray{std::string("primary")};
    const auto result = run_road_import(params);
    expect(result.ok && result.features.size() == 1 &&
               result.features.front().source_id == "way:103",
           "highway_filter narrows import to primary");
}

void test_geojson_import() {
    agbot::config::ParamTable params;
    params["path"] = kGeojsonFixture;
    const auto result = run_road_import(params);
    expect(result.ok, "geojson fixture imports");
    expect(result.features.size() == 3, "geojson yields LineString + 2 MultiLineString parts");
    const auto* street = find_feature(result.features, "way:201");
    expect(street != nullptr && attribute_or(*street, "oneway", "") == "yes" &&
               attribute_or(*street, "lanes", "") == "3",
           "geojson properties carried through");
}

std::vector<agbot::worldgen::ExtractedFeature> fixture_features() {
    agbot::config::ParamTable params;
    params["path"] = kOverpassFixture;
    return run_road_import(params).features;
}

void test_road_network() {
    const std::vector<agbot::worldgen::ExtractedFeature> features = fixture_features();
    const agbot::worldgen::RoadNetworkParams params;
    const agbot::worldgen::RoadNetwork network =
        agbot::worldgen::RoadNetwork::build(features, kOrigin, params);

    // 6 grid corners + the Spur end: interior shared vertices weld onto the
    // grid corners, so exactly 7 nodes.
    expect(network.nodes().size() == 7, "weld produces 7 nodes");
    // 8 two-way street segments (16 directed) + 1 oneway = 17 directed edges.
    expect(network.edges().size() == 17, "grid yields 17 directed edges");
    expect(network.largest_component_size() == 7, "fixture graph fully connected");

    std::size_t oneway_edges = 0;
    for (const agbot::worldgen::RoadEdge& edge : network.edges()) {
        if (edge.way_id == "104") {
            ++oneway_edges;
            const agbot::worldgen::RoadNode& from = network.nodes()[edge.from];
            const agbot::worldgen::RoadNode& to = network.nodes()[edge.to];
            expect(near(from.z, 0.0, 0.05) && near(to.z, kBlockM, 0.05),
                   "oneway edge points south -> north");
            expect(network.reverse_edge(edge.id) == nullptr, "oneway edge has no reverse");
        }
        if (edge.way_id == "101") {
            expect(near(edge.length_m, kBlockM, 0.05), "A Street edge length is one block");
            expect(network.reverse_edge(edge.id) != nullptr, "two-way edge has a reverse");
            expect(near(edge.travel_time_s, kBlockM / 8.0, 0.05),
                   "residential travel time uses 8 m/s");
        }
        if (edge.way_id == "106") {
            expect(near(edge.length_m, kBlockM / 2.0, 0.05), "Spur edge length is half block");
            expect(edge.lanes == 2, "Spur lanes parsed");
        }
        if (edge.way_id == "107") {
            expect(near(edge.length_m, kBlockM * std::sqrt(2.0), 0.05),
                   "Shortcut diagonal length");
            expect(near(edge.speed_mps, 5.0, 1e-9), "service speed is 5 m/s");
        }
        if (edge.way_id == "103") {
            expect(near(edge.speed_mps, 14.0, 1e-9), "primary speed is 14 m/s");
        }
    }
    expect(oneway_edges == 1, "oneway street yields a single directed edge");

    const agbot::worldgen::EdgeProjection projection =
        network.nearest_edge_point(kBlockM / 2.0, 5.0);
    expect(projection.ok, "nearest_edge_point finds an edge");
    expect(near(projection.point.x, kBlockM / 2.0, 0.05) && near(projection.point.z, 0.0, 0.05),
           "projection lands on A Street centerline");
    expect(near(projection.distance_m, 5.0, 0.05), "projection distance is the offset");
    expect(near(projection.s_along_m, kBlockM / 2.0, 0.5), "projection arc length");
}

void test_network_determinism() {
    std::vector<agbot::worldgen::ExtractedFeature> features = fixture_features();
    const agbot::worldgen::RoadNetworkParams params;
    const std::uint64_t first =
        agbot::worldgen::RoadNetwork::build(features, kOrigin, params).graph_hash();
    std::reverse(features.begin(), features.end());
    const std::uint64_t second =
        agbot::worldgen::RoadNetwork::build(features, kOrigin, params).graph_hash();
    expect(first == second, "graph hash independent of feature order");
    expect(first != 0, "graph hash non-trivial");
}

void test_road_mesh() {
    const std::vector<agbot::worldgen::ExtractedFeature> features = fixture_features();
    const agbot::worldgen::RoadNetwork network =
        agbot::worldgen::RoadNetwork::build(features, kOrigin, {});
    agbot::worldgen::RoadMeshParams mesh_params;
    const agbot::worldgen::CityMesh mesh =
        agbot::worldgen::build_road_mesh(network, mesh_params);

    // 9 undirected street segments (two-way pairs meshed once), each a
    // 2-point polyline: 4 vertices and 2 triangles per segment.
    expect(mesh.vertices.size() == 36, "road mesh vertex count");
    expect(mesh.indices.size() == 54, "road mesh index count");
    expect(mesh.batches.size() == 1 && mesh.batches.front().index_count == 54,
           "road mesh single batch covers all indices");

    bool class_ok = true;
    bool offset_ok = true;
    for (const agbot::worldgen::CityVertex& vertex : mesh.vertices) {
        class_ok = class_ok &&
            vertex.class_id == agbot::worldgen::class_id_for(agbot::worldgen::FeatureClass::Road);
        offset_ok = offset_ok && near(vertex.position[1], 0.15, 1e-6);
    }
    expect(class_ok, "road mesh uses Road class id");
    expect(offset_ok, "road mesh sits at the ground offset");

    // The Spur (way 106, lanes=2) runs north-south at x = 2 blocks: its
    // ribbon width must be lanes * lane_width = 6.4 m. Only the Spur's south
    // corners reach z < -10 (other ribbons stay within half a street width
    // of z >= 0).
    double spur_min_x = 1e18;
    double spur_max_x = -1e18;
    for (const agbot::worldgen::CityVertex& vertex : mesh.vertices) {
        if (vertex.position[2] < -10.0) {
            spur_min_x = std::min(spur_min_x, static_cast<double>(vertex.position[0]));
            spur_max_x = std::max(spur_max_x, static_cast<double>(vertex.position[0]));
        }
    }
    expect(near(spur_max_x - spur_min_x, 2.0 * mesh_params.lane_width_m, 1e-3),
           "lane count drives ribbon width");

    const agbot::worldgen::CityMesh again = agbot::worldgen::build_road_mesh(network, mesh_params);
    expect(agbot::worldgen::city_mesh_vertex_hash(mesh) ==
               agbot::worldgen::city_mesh_vertex_hash(again),
           "road mesh deterministic");
}

void test_manhattan_integration() {
    if (!std::filesystem::exists(kManhattanRoads)) {
        std::cout << "SKIP manhattan roads integration ("
                  << "data/worldgen/manhattan_roads.json absent; run "
                  << "worldgen/tools/fetch_osm_roads.sh)\n";
        return;
    }
    const agbot::flight_sim::GeoBounds aoi{40.700, -74.020, 40.740, -73.980};
    agbot::config::ParamTable params;
    params["path"] = kManhattanRoads;
    const auto extractor = agbot::worldgen::extractor_registry().create("road_import");
    const auto result = extractor->extract({aoi, params});
    expect(result.ok, "manhattan roads import");
    expect(result.features.size() > 500, "manhattan yields > 500 ways");

    const agbot::worldgen::RoadNetwork network =
        agbot::worldgen::RoadNetwork::build(result.features, aoi.center(), {});
    expect(network.nodes().size() > 500, "manhattan graph has > 500 nodes");
    expect(!network.edges().empty(), "manhattan graph has edges");
    const double component_share = static_cast<double>(network.largest_component_size()) /
        static_cast<double>(network.nodes().size());
    std::cout << "INFO manhattan graph: " << network.nodes().size() << " nodes, "
              << network.edges().size() << " directed edges, largest component "
              << component_share * 100.0 << "%\n";
    expect(component_share > 0.8, "largest component covers > 80% of nodes");
}

} // namespace

int main() {
    test_registry();
    test_reason_codes();
    test_overpass_import();
    test_highway_filter_param();
    test_geojson_import();
    test_road_network();
    test_network_determinism();
    test_road_mesh();
    test_manhattan_integration();

    if (failures != 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all road tests passed\n";
    return 0;
}
