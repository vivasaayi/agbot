#include "agbot_worldgen/Feature.hpp"
#include "agbot_worldgen/FeatureExtractor.hpp"
#include "agbot_worldgen/Polygonize.hpp"
#include "agbot_worldgen/SegTiler.hpp"
#include "agbot_worldgen/extractors/ClassicalIndex.hpp"
#include "agbot_worldgen/extractors/OnnxSemSeg.hpp"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <memory>
#include <set>
#include <sstream>
#include <string>
#include <vector>

namespace {

namespace wg = agbot::worldgen;
namespace fs = agbot::flight_sim;

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

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

fs::GeoBounds test_aoi() {
    // ~111 m x ~111 m box at the equator.
    return {0.0, 0.0, 0.001, 0.001};
}

// '.' = 0, '#' = 1, 'X' = 2. Each string is one raster row (row 0 = north).
wg::ClassMask mask_from_rows(const std::vector<std::string>& rows) {
    wg::ClassMask mask;
    mask.height = static_cast<int>(rows.size());
    mask.width = rows.empty() ? 0 : static_cast<int>(rows.front().size());
    for (const std::string& row : rows) {
        for (const char c : row) {
            mask.classes.push_back(c == '.' ? 0 : (c == '#' ? 1 : 2));
        }
    }
    return mask;
}

// Cell dimensions in meters via the same local frame Polygonize uses.
void cell_size_m(
    const fs::GeoBounds& aoi, int width, int height, double& cell_w, double& cell_h) {
    const fs::GeoCoordinate origin = aoi.center();
    const fs::Vec3 low =
        fs::local_from_geo({aoi.min_latitude, aoi.min_longitude, 0.0}, origin);
    const fs::Vec3 high =
        fs::local_from_geo({aoi.max_latitude, aoi.max_longitude, 0.0}, origin);
    cell_w = std::abs(high.x - low.x) / width;
    cell_h = std::abs(high.z - low.z) / height;
}

double signed_area_local(
    const std::vector<fs::GeoCoordinate>& ring, const fs::GeoCoordinate& origin) {
    if (ring.size() < 3) {
        return 0.0;
    }
    double doubled = 0.0;
    fs::Vec3 previous = fs::local_from_geo(ring.back(), origin);
    for (const fs::GeoCoordinate& point : ring) {
        const fs::Vec3 current = fs::local_from_geo(point, origin);
        doubled += previous.x * current.z - current.x * previous.z;
        previous = current;
    }
    return doubled * 0.5;
}

std::string serialize_features(const std::vector<wg::ExtractedFeature>& features) {
    std::ostringstream out;
    out << std::setprecision(12);
    for (const wg::ExtractedFeature& feature : features) {
        out << feature.class_name << "|" << feature.source_id << "|"
            << feature.confidence << "|";
        for (const fs::GeoCoordinate& point : feature.exterior) {
            out << point.latitude << "," << point.longitude << ";";
        }
        for (const auto& hole : feature.holes) {
            out << "H:";
            for (const fs::GeoCoordinate& point : hole) {
                out << point.latitude << "," << point.longitude << ";";
            }
        }
        out << "\n";
    }
    return out.str();
}

bool ring_within_aoi(
    const std::vector<fs::GeoCoordinate>& ring, const fs::GeoBounds& aoi, double eps) {
    for (const fs::GeoCoordinate& point : ring) {
        if (point.latitude < aoi.min_latitude - eps || point.latitude > aoi.max_latitude + eps ||
            point.longitude < aoi.min_longitude - eps ||
            point.longitude > aoi.max_longitude + eps) {
            return false;
        }
    }
    return true;
}

// ---------------------------------------------------------------------------
// Minimal PNG writer (RGB8, filter 0, stored-deflate) for synthetic imagery.
// Same idiom as terrain_engine/tests; the worldgen extractors decode it with
// the shared agbot_terrain PNG decoder.
// ---------------------------------------------------------------------------

std::uint32_t crc32_bytes(const std::uint8_t* data, std::size_t size, std::uint32_t crc) {
    crc = ~crc;
    for (std::size_t i = 0; i < size; ++i) {
        crc ^= data[i];
        for (int bit = 0; bit < 8; ++bit) {
            crc = (crc >> 1) ^ (0xEDB88320u & (~(crc & 1u) + 1u));
        }
    }
    return ~crc;
}

void append_u32_be(std::vector<std::uint8_t>& out, std::uint32_t value) {
    out.push_back(static_cast<std::uint8_t>(value >> 24));
    out.push_back(static_cast<std::uint8_t>(value >> 16));
    out.push_back(static_cast<std::uint8_t>(value >> 8));
    out.push_back(static_cast<std::uint8_t>(value));
}

void append_chunk(
    std::vector<std::uint8_t>& out, const char* type, const std::vector<std::uint8_t>& payload) {
    append_u32_be(out, static_cast<std::uint32_t>(payload.size()));
    std::vector<std::uint8_t> body(type, type + 4);
    body.insert(body.end(), payload.begin(), payload.end());
    out.insert(out.end(), body.begin(), body.end());
    append_u32_be(out, crc32_bytes(body.data(), body.size(), 0));
}

bool write_test_png_rgb(
    const std::filesystem::path& path, int width, int height,
    const std::vector<std::uint8_t>& rgb) {
    std::vector<std::uint8_t> raw;
    raw.reserve(static_cast<std::size_t>(height) * (1 + static_cast<std::size_t>(width) * 3));
    for (int row = 0; row < height; ++row) {
        raw.push_back(0);
        const std::size_t offset =
            static_cast<std::size_t>(row) * static_cast<std::size_t>(width) * 3;
        raw.insert(raw.end(), rgb.begin() + static_cast<std::ptrdiff_t>(offset),
                   rgb.begin() + static_cast<std::ptrdiff_t>(offset +
                       static_cast<std::size_t>(width) * 3));
    }
    std::vector<std::uint8_t> zlib = {0x78, 0x01};
    std::size_t position = 0;
    while (position < raw.size()) {
        const std::size_t block = std::min<std::size_t>(65535, raw.size() - position);
        const bool final_block = position + block == raw.size();
        zlib.push_back(final_block ? 0x01 : 0x00);
        zlib.push_back(static_cast<std::uint8_t>(block & 0xFF));
        zlib.push_back(static_cast<std::uint8_t>(block >> 8));
        zlib.push_back(static_cast<std::uint8_t>(~block & 0xFF));
        zlib.push_back(static_cast<std::uint8_t>((~block >> 8) & 0xFF));
        zlib.insert(zlib.end(), raw.begin() + static_cast<std::ptrdiff_t>(position),
                    raw.begin() + static_cast<std::ptrdiff_t>(position + block));
        position += block;
    }
    std::uint32_t s1 = 1;
    std::uint32_t s2 = 0;
    for (const std::uint8_t byte : raw) {
        s1 = (s1 + byte) % 65521u;
        s2 = (s2 + s1) % 65521u;
    }
    append_u32_be(zlib, (s2 << 16) | s1);

    std::vector<std::uint8_t> file = {0x89, 'P', 'N', 'G', 0x0D, 0x0A, 0x1A, 0x0A};
    std::vector<std::uint8_t> ihdr;
    append_u32_be(ihdr, static_cast<std::uint32_t>(width));
    append_u32_be(ihdr, static_cast<std::uint32_t>(height));
    ihdr.push_back(8); // bit depth
    ihdr.push_back(2); // color type RGB
    ihdr.push_back(0);
    ihdr.push_back(0);
    ihdr.push_back(0);
    append_chunk(file, "IHDR", ihdr);
    append_chunk(file, "IDAT", zlib);
    append_chunk(file, "IEND", {});

    std::filesystem::create_directories(path.parent_path());
    std::ofstream out(path, std::ios::binary | std::ios::trunc);
    if (!out) {
        return false;
    }
    out.write(reinterpret_cast<const char*>(file.data()),
              static_cast<std::streamsize>(file.size()));
    return static_cast<bool>(out);
}

std::filesystem::path scratch_dir() {
    return std::filesystem::temp_directory_path() / "agbot_worldgen_seg_tests";
}

// ---------------------------------------------------------------------------
// Polygonize
// ---------------------------------------------------------------------------

void test_polygonize_single_blob() {
    const fs::GeoBounds aoi = test_aoi();
    const wg::ClassMask mask = mask_from_rows({
        "........",
        "........",
        "..###...",
        "..###...",
        "..###...",
        "........",
        "........",
        "........",
    });
    std::vector<std::int32_t> labels;
    const auto polygons = wg::polygonize_class(mask, aoi, 1, {}, &labels);
    expect(polygons.size() == 1, "single blob yields one polygon");
    if (polygons.size() != 1) {
        return;
    }
    expect(polygons[0].holes.empty(), "single blob has no holes");
    expect(polygons[0].exterior.size() == 4,
           "square blob exterior collapses to 4 corners");
    expect(polygons[0].cell_count == 9, "single blob counts 9 cells");

    double cell_w = 0.0;
    double cell_h = 0.0;
    cell_size_m(aoi, mask.width, mask.height, cell_w, cell_h);
    const double expected = 9.0 * cell_w * cell_h;
    expect(near(polygons[0].area_m2, expected, expected * 1e-6),
           "square blob area matches 9 cells (" + std::to_string(polygons[0].area_m2) +
               " vs " + std::to_string(expected) + " m2)");
    expect(signed_area_local(polygons[0].exterior, aoi.center()) > 0.0,
           "exterior ring winds CCW");

    expect(labels.size() == 64, "label map covers all cells");
    int labeled = 0;
    for (const std::int32_t label : labels) {
        if (label == 0) {
            ++labeled;
        }
    }
    expect(labeled == 9, "label map marks exactly the blob cells");
}

void test_polygonize_hole() {
    const fs::GeoBounds aoi = test_aoi();
    const wg::ClassMask mask = mask_from_rows({
        ".....",
        ".###.",
        ".#.#.",
        ".###.",
        ".....",
    });
    const auto polygons = wg::polygonize_class(mask, aoi, 1, {});
    expect(polygons.size() == 1, "donut yields one polygon");
    if (polygons.size() != 1) {
        return;
    }
    expect(polygons[0].holes.size() == 1, "donut has exactly one hole");
    expect(polygons[0].exterior.size() == 4, "donut exterior is the outer square");
    if (polygons[0].holes.size() == 1) {
        expect(polygons[0].holes[0].size() == 4, "donut hole is the inner square");
        expect(signed_area_local(polygons[0].holes[0], aoi.center()) < 0.0,
               "hole ring winds CW");
    }
    double cell_w = 0.0;
    double cell_h = 0.0;
    cell_size_m(aoi, mask.width, mask.height, cell_w, cell_h);
    const double expected = 8.0 * cell_w * cell_h;
    expect(near(polygons[0].area_m2, expected, expected * 1e-6),
           "donut area is 8 cells (hole subtracted)");
}

void test_polygonize_two_blobs() {
    const fs::GeoBounds aoi = test_aoi();
    const wg::ClassMask mask = mask_from_rows({
        "##....",
        "##....",
        "......",
        "....##",
        "....##",
        "......",
    });
    const auto polygons = wg::polygonize_class(mask, aoi, 1, {});
    expect(polygons.size() == 2, "two blobs yield two polygons");
    if (polygons.size() == 2) {
        expect(polygons[0].component_label == 0 && polygons[1].component_label == 1,
               "blobs appear in scanline discovery order");
        // First blob is the north-west one: larger latitudes, smaller longitudes.
        double max_lat_0 = -1e9;
        for (const auto& p : polygons[0].exterior) {
            max_lat_0 = std::max(max_lat_0, p.latitude);
        }
        double max_lat_1 = -1e9;
        for (const auto& p : polygons[1].exterior) {
            max_lat_1 = std::max(max_lat_1, p.latitude);
        }
        expect(max_lat_0 > max_lat_1, "first discovered blob is the northern one");
    }
}

void test_polygonize_diagonal_saddle() {
    const fs::GeoBounds aoi = test_aoi();
    const wg::ClassMask mask = mask_from_rows({
        "#.",
        ".#",
    });
    const auto polygons = wg::polygonize_class(mask, aoi, 1, {});
    expect(polygons.size() == 2, "diagonal-touching cells stay separate (4-connectivity)");
    for (const auto& polygon : polygons) {
        expect(polygon.exterior.size() == 4 && polygon.holes.empty(),
               "saddle blob is a plain unit square");
    }
}

void test_polygonize_min_area_filter() {
    const fs::GeoBounds aoi = test_aoi();
    const wg::ClassMask mask = mask_from_rows({
        "....",
        ".#..",
        "....",
        "....",
    });
    double cell_w = 0.0;
    double cell_h = 0.0;
    cell_size_m(aoi, mask.width, mask.height, cell_w, cell_h);
    wg::PolygonizeOptions options;
    options.min_area_m2 = cell_w * cell_h * 1.5;
    expect(wg::polygonize_class(mask, aoi, 1, options).empty(),
           "tiny blob filtered by min_area_m2");
    options.min_area_m2 = cell_w * cell_h * 0.5;
    expect(wg::polygonize_class(mask, aoi, 1, options).size() == 1,
           "tiny blob kept when under min_area_m2");
}

void test_polygonize_simplification() {
    const fs::GeoBounds aoi = test_aoi();
    // Rectangle with a one-cell notch in the south edge.
    const wg::ClassMask mask = mask_from_rows({
        "######",
        "######",
        "###.##",
    });
    const auto raw = wg::polygonize_class(mask, aoi, 1, {});
    expect(raw.size() == 1 && raw[0].exterior.size() == 8,
           "notched rectangle has 8 boundary corners unsimplified");

    double cell_w = 0.0;
    double cell_h = 0.0;
    cell_size_m(aoi, mask.width, mask.height, cell_w, cell_h);
    wg::PolygonizeOptions options;
    options.simplify_tol_m = 2.0 * std::max(cell_w, cell_h);
    const auto simplified = wg::polygonize_class(mask, aoi, 1, options);
    expect(simplified.size() == 1 && !simplified[0].exterior.empty() &&
               simplified[0].exterior.size() < raw[0].exterior.size(),
           "Douglas-Peucker removes the notch corners");
}

void test_polygonize_determinism() {
    const fs::GeoBounds aoi = test_aoi();
    const wg::ClassMask mask = mask_from_rows({
        "##..##..",
        "##..##..",
        "...###..",
        "..##.##.",
        "..#####.",
        "........",
    });
    const auto a = wg::polygonize_class(mask, aoi, 1, {});
    const auto b = wg::polygonize_class(mask, aoi, 1, {});
    bool same = a.size() == b.size();
    for (std::size_t i = 0; same && i < a.size(); ++i) {
        same = a[i].component_label == b[i].component_label &&
            a[i].exterior.size() == b[i].exterior.size() &&
            a[i].holes.size() == b[i].holes.size() && a[i].area_m2 == b[i].area_m2;
    }
    expect(same, "polygonize is deterministic across runs");
}

// ---------------------------------------------------------------------------
// SegTiler
// ---------------------------------------------------------------------------

bool tiles_cover_image(const std::vector<wg::TileRect>& tiles, int width, int height) {
    std::vector<std::uint8_t> covered(
        static_cast<std::size_t>(width) * static_cast<std::size_t>(height), 0);
    for (const wg::TileRect& tile : tiles) {
        if (tile.x0 < 0 || tile.y0 < 0 || tile.x0 + tile.width > width ||
            tile.y0 + tile.height > height) {
            return false;
        }
        for (int y = tile.y0; y < tile.y0 + tile.height; ++y) {
            for (int x = tile.x0; x < tile.x0 + tile.width; ++x) {
                covered[static_cast<std::size_t>(y) * width + x] = 1;
            }
        }
    }
    return std::all_of(covered.begin(), covered.end(),
                       [](std::uint8_t v) { return v != 0; });
}

void test_tile_planning() {
    const auto tiles = wg::plan_tiles(1024, 1024, 512, 64);
    expect(tiles.size() == 9, "1024^2 at 512/64 plans a 3x3 tile grid");
    expect(tiles_cover_image(tiles, 1024, 1024), "1024^2 tiling covers every pixel");
    bool sized = true;
    for (const wg::TileRect& tile : tiles) {
        sized = sized && tile.width == 512 && tile.height == 512;
    }
    expect(sized, "all planned tiles are full model-input size");
    if (tiles.size() == 9) {
        expect(tiles[0].x0 == 0 && tiles[1].x0 == 448 && tiles[2].x0 == 512,
               "tile columns overlap and clamp to the image edge");
        expect(tiles[1].x0 + 512 - tiles[2].x0 >= 64,
               "consecutive tiles keep at least the requested overlap");
    }

    const auto small = wg::plan_tiles(300, 200, 512, 64);
    expect(small.size() == 1 && small[0].width == 300 && small[0].height == 200,
           "image smaller than tile size becomes a single content tile");
    expect(wg::plan_tiles(0, 100, 512, 64).empty(), "degenerate image plans no tiles");

    const auto map_tile = wg::plan_tiles(256, 256, 128, 32);
    expect(map_tile.size() == 9 && tiles_cover_image(map_tile, 256, 256),
           "256^2 at 128/32 plans a covering 3x3 grid");
}

void test_tile_stitching() {
    wg::TileStitcher stitcher(12, 8);
    const wg::TileRect left{0, 0, 8, 8};
    const wg::TileRect right{4, 0, 8, 8};
    const std::vector<std::uint8_t> ones(64, 1);
    const std::vector<std::uint8_t> twos(64, 2);
    const std::vector<float> low(64, 0.25f);
    const std::vector<float> high(64, 0.75f);
    stitcher.commit(left, ones, low);
    stitcher.commit(right, twos, high);

    const auto& classes = stitcher.classes();
    // Pixel (5,3): depth 2 in the left tile vs 1 in the right -> left wins.
    expect(classes[3 * 12 + 5] == 1, "overlap pixel closer to left center keeps left tile");
    // Pixel (6,3): depth 1 in the left tile vs 2 in the right -> right wins.
    expect(classes[3 * 12 + 6] == 2, "overlap pixel closer to right center keeps right tile");
    expect(classes[3 * 12 + 1] == 1 && classes[3 * 12 + 10] == 2,
           "non-overlap pixels keep their own tile");
    expect(near(stitcher.confidence()[3 * 12 + 10], 0.75, 1e-6),
           "confidence stitched alongside classes");
}

// ---------------------------------------------------------------------------
// classical_index
// ---------------------------------------------------------------------------

// 64x64 synthetic scene: gray ground, 16x16 green patch at (8,8)..(23,23),
// 16x16 blue lake at (40,40)..(55,55), plus 3 isolated green salt pixels.
constexpr int kSceneSize = 64;

std::filesystem::path write_synthetic_scene() {
    std::vector<std::uint8_t> rgb(
        static_cast<std::size_t>(kSceneSize) * kSceneSize * 3, 0);
    const auto set_pixel = [&](int x, int y, std::uint8_t r, std::uint8_t g, std::uint8_t b) {
        const std::size_t index =
            (static_cast<std::size_t>(y) * kSceneSize + static_cast<std::size_t>(x)) * 3;
        rgb[index] = r;
        rgb[index + 1] = g;
        rgb[index + 2] = b;
    };
    for (int y = 0; y < kSceneSize; ++y) {
        for (int x = 0; x < kSceneSize; ++x) {
            set_pixel(x, y, 128, 128, 128); // gray ground
        }
    }
    for (int y = 8; y < 24; ++y) {
        for (int x = 8; x < 24; ++x) {
            set_pixel(x, y, 40, 180, 40); // vegetation patch
        }
    }
    for (int y = 40; y < 56; ++y) {
        for (int x = 40; x < 56; ++x) {
            set_pixel(x, y, 30, 60, 200); // lake
        }
    }
    set_pixel(40, 10, 40, 180, 40); // salt noise (isolated green pixels)
    set_pixel(52, 20, 40, 180, 40);
    set_pixel(10, 50, 40, 180, 40);

    const std::filesystem::path path = scratch_dir() / "classical_scene.png";
    if (!write_test_png_rgb(path, kSceneSize, kSceneSize, rgb)) {
        return {};
    }
    return path;
}

agbot::config::ParamTable classical_params(const std::filesystem::path& imagery) {
    agbot::config::ParamTable params;
    params["imagery_path"] = imagery.string();
    params["veg_method"] = "exg";
    params["veg_thresh"] = 0.08;
    params["water_method"] = "blueness";
    params["water_thresh"] = 0.05;
    params["morph_open_px"] = 1;
    params["min_area_m2"] = 0.0;
    params["simplify_tol_m"] = 0.0;
    params["confidence"] = 0.6;
    return params;
}

void test_classical_error_paths() {
    const auto extractor = wg::extractor_registry().create("classical_index");
    expect(extractor != nullptr, "classical_index registered");
    if (!extractor) {
        return;
    }
    agbot::config::ParamTable params;
    const wg::ExtractionContext context{test_aoi(), params};
    const auto missing = extractor->extract(context);
    expect(!missing.ok && missing.error_code == "params_missing_imagery_path",
           "classical_index requires imagery_path");

    params["imagery_path"] = "/nonexistent/scene.png";
    const auto absent = extractor->extract({test_aoi(), params});
    expect(!absent.ok && absent.error_code == "imagery_not_found",
           "classical_index reports imagery_not_found");

    params = classical_params(write_synthetic_scene());
    params["veg_method"] = "ndvi";
    const auto bad_method = extractor->extract({test_aoi(), params});
    expect(!bad_method.ok && bad_method.error_code == "unknown_veg_method",
           "classical_index rejects unknown veg_method");
}

void test_classical_extraction() {
    const std::filesystem::path imagery = write_synthetic_scene();
    expect(!imagery.empty(), "synthetic scene PNG written");
    if (imagery.empty()) {
        return;
    }
    const fs::GeoBounds aoi = test_aoi();
    const auto extractor = wg::extractor_registry().create("classical_index");
    const auto params = classical_params(imagery);
    const auto result = extractor->extract({aoi, params});
    expect(result.ok, "classical extraction succeeds");
    if (!result.ok) {
        return;
    }

    std::size_t veg_count = 0;
    std::size_t water_count = 0;
    const wg::ExtractedFeature* veg = nullptr;
    const wg::ExtractedFeature* water = nullptr;
    for (const wg::ExtractedFeature& feature : result.features) {
        if (feature.cls == wg::FeatureClass::Vegetation) {
            ++veg_count;
            veg = &feature;
        } else if (feature.cls == wg::FeatureClass::Water) {
            ++water_count;
            water = &feature;
        }
    }
    expect(veg_count == 1 && water_count == 1,
           "morph opening leaves one vegetation and one water blob (got " +
               std::to_string(veg_count) + "/" + std::to_string(water_count) + ")");
    if (veg != nullptr) {
        // Patch pixels [8,24) of 64 -> lon in [min + 8/64 dlon, min + 24/64 dlon],
        // lat in [max - 24/64 dlat, max - 8/64 dlat].
        const double dlat = aoi.max_latitude - aoi.min_latitude;
        const double dlon = aoi.max_longitude - aoi.min_longitude;
        double min_lat = 1e9;
        double max_lat = -1e9;
        double min_lon = 1e9;
        double max_lon = -1e9;
        for (const fs::GeoCoordinate& point : veg->exterior) {
            min_lat = std::min(min_lat, point.latitude);
            max_lat = std::max(max_lat, point.latitude);
            min_lon = std::min(min_lon, point.longitude);
            max_lon = std::max(max_lon, point.longitude);
        }
        const double eps = 1e-9;
        expect(near(min_lon, aoi.min_longitude + dlon * 8.0 / 64.0, eps) &&
                   near(max_lon, aoi.min_longitude + dlon * 24.0 / 64.0, eps) &&
                   near(max_lat, aoi.max_latitude - dlat * 8.0 / 64.0, eps) &&
                   near(min_lat, aoi.max_latitude - dlat * 24.0 / 64.0, eps),
               "vegetation patch georeferenced to the expected AOI window");
        expect(near(veg->confidence, 0.6, 1e-9), "classical confidence uses the param");
        expect(veg->exterior.size() == 4, "clean vegetation patch is a rectangle");
    }
    if (water != nullptr) {
        expect(ring_within_aoi(water->exterior, aoi, 1e-9), "water blob inside the AOI");
    }

    // Without morphological opening the three salt pixels survive.
    auto noisy_params = params;
    noisy_params["morph_open_px"] = 0;
    const auto noisy = extractor->extract({aoi, noisy_params});
    std::size_t noisy_veg = 0;
    for (const wg::ExtractedFeature& feature : noisy.features) {
        if (feature.cls == wg::FeatureClass::Vegetation) {
            ++noisy_veg;
        }
    }
    expect(noisy.ok && noisy_veg == 4,
           "without morph opening the salt pixels appear as blobs (got " +
               std::to_string(noisy_veg) + ")");

    const auto again = extractor->extract({aoi, params});
    expect(again.ok &&
               serialize_features(again.features) == serialize_features(result.features),
           "classical extraction deterministic across runs");
}

// ---------------------------------------------------------------------------
// onnx_semseg
// ---------------------------------------------------------------------------

#if defined(AGBOT_WORLDGEN_HAS_ONNX)

std::string seg_model_path() {
    return std::string(AGBOT_FLIGHT_SIM_SOURCE_DIR) +
        "/data/models/segformer_b0_ade20k_512.onnx";
}

// Deterministically pick a cached rendered map tile (first in sorted order).
std::filesystem::path first_map_tile() {
    const std::filesystem::path root =
        std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "map_tiles";
    if (!std::filesystem::exists(root)) {
        return {};
    }
    std::vector<std::filesystem::path> tiles;
    for (const auto& entry : std::filesystem::recursive_directory_iterator(root)) {
        if (entry.is_regular_file() && entry.path().extension() == ".png") {
            tiles.push_back(entry.path());
        }
    }
    std::sort(tiles.begin(), tiles.end());
    return tiles.empty() ? std::filesystem::path{} : tiles.front();
}

void test_onnx_error_paths() {
    const auto extractor = wg::extractor_registry().create("onnx_semseg");
    expect(extractor != nullptr, "onnx_semseg registered");
    if (!extractor) {
        return;
    }
    agbot::config::ParamTable params;
    const auto missing_imagery = extractor->extract({test_aoi(), params});
    expect(!missing_imagery.ok && missing_imagery.error_code == "params_missing_imagery_path",
           "onnx_semseg requires imagery_path");

    params["imagery_path"] = write_synthetic_scene().string();
    params["model_path"] = "/nonexistent/segmodel.onnx";
    const auto missing_model = extractor->extract({test_aoi(), params});
    expect(!missing_model.ok && missing_model.error_code == "model_missing",
           "onnx_semseg reports model_missing for bogus path");
}

void test_onnx_real_tile() {
    if (!std::filesystem::exists(seg_model_path())) {
        std::cout << "SKIP onnx_semseg real-tile run (model absent: " << seg_model_path()
                  << "; run worldgen/tools/fetch_seg_model.sh)\n";
        return;
    }
    const std::filesystem::path tile = first_map_tile();
    if (tile.empty()) {
        std::cout << "SKIP onnx_semseg real-tile run (no cached map tiles)\n";
        return;
    }

    const fs::GeoBounds aoi = test_aoi();
    const auto extractor = wg::extractor_registry().create("onnx_semseg");

    // Map every model class to a feature class so rendered map art (which the
    // ADE20K model was never trained on) is guaranteed to produce blobs; the
    // machinery under test is tiling, stitching, and vectorization.
    agbot::config::ParamTable identity_map;
    for (int c = 0; c < 150; ++c) {
        identity_map[std::to_string(c)] = "unknown";
    }
    agbot::config::ParamTable params;
    params["imagery_path"] = tile.string();
    params["input_size"] = 128; // exercises the 3x3 tile grid on a 256px tile
    params["overlap_px"] = 32;
    params["min_area_m2"] = 0.0;
    params["class_map"] = identity_map;

    const auto result = extractor->extract({aoi, params});
    expect(result.ok, "onnx_semseg real-tile extraction succeeds (" +
                          result.error_code + " " + result.error_detail + ")");
    if (!result.ok) {
        return;
    }
    expect(!result.features.empty(), "identity class_map yields features");
    bool rings_valid = true;
    bool confidence_valid = true;
    std::set<std::string> model_classes;
    for (const wg::ExtractedFeature& feature : result.features) {
        rings_valid = rings_valid && feature.exterior.size() >= 3 &&
            ring_within_aoi(feature.exterior, aoi, 1e-9);
        for (const auto& hole : feature.holes) {
            rings_valid = rings_valid && hole.size() >= 3 && ring_within_aoi(hole, aoi, 1e-9);
        }
        confidence_valid = confidence_valid &&
            feature.confidence > 0.0 && feature.confidence <= 1.0;
        const auto it = feature.attributes.find("model_class");
        if (it != feature.attributes.end()) {
            model_classes.insert(it->second);
        }
    }
    expect(rings_valid, "all rings valid and inside the AOI");
    expect(confidence_valid, "blob confidences are softmax probabilities in (0,1]");

    const auto again = extractor->extract({aoi, params});
    expect(again.ok &&
               serialize_features(again.features) == serialize_features(result.features),
           "onnx_semseg deterministic across two runs");

    // class_map filtering: restrict to the first feature's model class.
    const std::string keep_class = result.features.front().attributes.at("model_class");
    agbot::config::ParamTable filter_map;
    filter_map[keep_class] = "building";
    auto filtered_params = params;
    filtered_params["class_map"] = filter_map;
    const auto filtered = extractor->extract({aoi, filtered_params});
    bool only_kept = filtered.ok && !filtered.features.empty();
    for (const wg::ExtractedFeature& feature : filtered.features) {
        only_kept = only_kept && feature.class_name == "building" &&
            feature.attributes.at("model_class") == keep_class;
    }
    expect(only_kept, "class_map filters and renames model classes");

    const auto tiles = wg::plan_tiles(256, 256, 128, 32);
    std::cout << "  onnx_semseg: tile=" << tile.filename().string() << " tiles processed="
              << tiles.size() << " distinct model classes=" << model_classes.size()
              << " features=" << result.features.size()
              << " filtered(model_class=" << keep_class << ")="
              << filtered.features.size() << "\n";
}

#else // !AGBOT_WORLDGEN_HAS_ONNX

void test_onnx_stub() {
    const auto extractor = wg::extractor_registry().create("onnx_semseg");
    expect(extractor != nullptr, "onnx_semseg stub registered");
    if (!extractor) {
        return;
    }
    agbot::config::ParamTable params;
    params["imagery_path"] = "unused.png";
    const auto result = extractor->extract({test_aoi(), params});
    expect(!result.ok && result.error_code == "onnx_runtime_unavailable",
           "onnx_semseg stub reports onnx_runtime_unavailable");
    std::cout << "SKIP onnx_semseg inference tests (built without ONNX Runtime)\n";
}

#endif // AGBOT_WORLDGEN_HAS_ONNX

} // namespace

int main() {
    test_polygonize_single_blob();
    test_polygonize_hole();
    test_polygonize_two_blobs();
    test_polygonize_diagonal_saddle();
    test_polygonize_min_area_filter();
    test_polygonize_simplification();
    test_polygonize_determinism();
    test_tile_planning();
    test_tile_stitching();
    test_classical_error_paths();
    test_classical_extraction();
#if defined(AGBOT_WORLDGEN_HAS_ONNX)
    test_onnx_error_paths();
    test_onnx_real_tile();
#else
    test_onnx_stub();
#endif

    if (failures > 0) {
        std::cout << failures << " test(s) failed\n";
        return 1;
    }
    std::cout << "all worldgen segmentation tests passed\n";
    return 0;
}
