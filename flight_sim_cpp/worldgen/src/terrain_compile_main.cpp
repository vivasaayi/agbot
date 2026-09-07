#include "agbot_worldgen/WorldCompiler.hpp"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdlib>
#include <cstdint>
#include <filesystem>
#include <iomanip>
#include <iostream>
#include <map>
#include <sstream>
#include <stdexcept>
#include <string>

namespace {

std::string json_escape(const std::string& value) {
    std::string out;
    for (const char character : value) {
        switch (character) {
            case '"': out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n"; break;
            case '\r': out += "\\r"; break;
            case '\t': out += "\\t"; break;
            default: out += character; break;
        }
    }
    return out;
}

std::string toml_escape(const std::string& value) {
    return json_escape(value);
}

std::string number(double value) {
    std::ostringstream out;
    out << std::setprecision(17) << value;
    return out.str();
}

void print_usage() {
    std::cout
        << "Usage: agbot_terrain_compile --dem PATH --output-dir DIR --name NAME "
           "--min-lat N --min-lon N --max-lat N --max-lon N "
           "--vertical-datum DATUM [--resolution N] [--target-gsd-m N] "
           "[--seed N] [--source-id ID] [--source-version VERSION]\n";
}

std::map<std::string, std::string> parse_args(int argc, char** argv) {
    std::map<std::string, std::string> values;
    for (int index = 1; index < argc; ++index) {
        const std::string key = argv[index];
        if (key == "--help" || key == "-h") {
            print_usage();
            std::exit(0);
        }
        if (key.rfind("--", 0) != 0 || index + 1 >= argc) {
            throw std::runtime_error("expected --key value argument");
        }
        values[key.substr(2)] = argv[++index];
    }
    return values;
}

const std::string& required(
    const std::map<std::string, std::string>& values,
    const std::string& key) {
    const auto found = values.find(key);
    if (found == values.end() || found->second.empty()) {
        throw std::runtime_error("missing --" + key);
    }
    return found->second;
}

std::string value_or(
    const std::map<std::string, std::string>& values,
    const std::string& key,
    const std::string& fallback) {
    const auto found = values.find(key);
    return found == values.end() ? fallback : found->second;
}

} // namespace

int main(int argc, char** argv) {
    try {
        const auto args = parse_args(argc, argv);
        const std::filesystem::path dem_path = required(args, "dem");
        const std::filesystem::path output_dir = required(args, "output-dir");
        const std::string name = required(args, "name");
        const double min_lat = std::stod(required(args, "min-lat"));
        const double min_lon = std::stod(required(args, "min-lon"));
        const double max_lat = std::stod(required(args, "max-lat"));
        const double max_lon = std::stod(required(args, "max-lon"));
        const int resolution = std::stoi(value_or(args, "resolution", "128"));
        const double target_gsd_m = std::stod(value_or(args, "target-gsd-m", "30"));
        const std::uint64_t seed = std::stoull(value_or(args, "seed", "0"));
        const std::string vertical_datum = required(args, "vertical-datum");

        if (!std::filesystem::exists(dem_path)) {
            throw std::runtime_error("DEM does not exist: " + dem_path.string());
        }
        if (!std::isfinite(min_lat) || !std::isfinite(min_lon) ||
            !std::isfinite(max_lat) || !std::isfinite(max_lon) ||
            max_lat <= min_lat || max_lon <= min_lon) {
            throw std::runtime_error("AOI bounds are degenerate");
        }
        if (resolution < 2 || resolution > 4096) {
            throw std::runtime_error("resolution must be in [2, 4096]");
        }
        if (!std::isfinite(target_gsd_m) || target_gsd_m <= 0.0) {
            throw std::runtime_error("target GSD must be positive");
        }
        const bool safe_name = name != "." && name != ".." &&
            std::all_of(name.begin(), name.end(), [](const unsigned char character) {
                return std::isalnum(character) != 0 || character == '-' ||
                    character == '_' || character == '.';
            });
        if (!safe_name) {
            throw std::runtime_error("name must be a safe filename component");
        }
        std::filesystem::create_directories(output_dir);
        const std::filesystem::path validation_path =
            output_dir / (name + ".validation.json");

        const std::string terrain_toml =
            "[pipeline]\n"
            "target_gsd_m = " + number(target_gsd_m) + "\n"
            "resolution = " + std::to_string(resolution) + "\n"
            "aoi = { min_lat = " + number(min_lat) +
            ", min_lon = " + number(min_lon) +
            ", max_lat = " + number(max_lat) +
            ", max_lon = " + number(max_lon) + " }\n"
            "[[layer]]\n"
            "algorithm = \"dem_fusion\"\n"
            "weight = 1.0\n"
            "  [layer.params]\n"
            "  source = \"geotiff\"\n"
            "  path = \"" + toml_escape(dem_path.string()) + "\"\n"
            "  resample = \"bilinear\"\n"
            "[fusion]\n"
            "method = \"dem_locked\"\n"
            "[validation]\n"
            "enabled = true\n"
            "reference_layer = 0\n"
            "output_json = \"" + toml_escape(validation_path.string()) + "\"\n";

        agbot::worldgen::WorldCompileSpec spec;
        spec.aoi = {min_lat, min_lon, max_lat, max_lon};
        spec.seed = seed;
        spec.terrain_config_toml = terrain_toml;
        spec.terrain_source_id = value_or(args, "source-id", "catalog:elevation");
        spec.terrain_uri = dem_path.string();
        spec.terrain_version = value_or(args, "source-version", "");
        spec.terrain_vertical_datum = vertical_datum;
        spec.terrain_content_hash = agbot::worldgen::hash_file_bytes(dem_path);
        spec.terrain_authoritative = true;
        spec.allow_terrain_only = true;

        const agbot::worldgen::WorldCompileResult world =
            agbot::worldgen::compile_world(spec);
        if (!world.ok) {
            std::cerr << world.error_code << ": " << world.error_detail << "\n";
            return 2;
        }
        const agbot::worldgen::WorldWriteResult written =
            agbot::worldgen::write_world_artifacts(world, output_dir, name);
        if (!written.ok) {
            std::cerr << written.error_code << ": " << written.error_detail << "\n";
            return 3;
        }

        const auto& quality = world.manifest.quality;
        const auto elevation_state = world.manifest.tiles.empty()
            ? agbot::worldgen::ElevationState::Missing
            : world.manifest.tiles.front().elevation_state;
        std::cout
            << "{\"manifest_path\":\"" << json_escape(written.manifest_path.string())
            << "\",\"scene_path\":\"" << json_escape(written.scene_path.string())
            << "\",\"validation_path\":\"" << json_escape(validation_path.string())
            << "\",\"world_hash\":" << world.manifest.world_hash
            << ",\"elevation_state\":\"" << agbot::worldgen::to_string(elevation_state)
            << "\",\"terrain_min_m\":" << quality.terrain_min_m
            << ",\"terrain_max_m\":" << quality.terrain_max_m
            << ",\"terrain_cell_count\":" << quality.terrain_cell_count
            << ",\"terrain_nodata_cells\":" << quality.terrain_nodata_cells
            << "}\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "invalid_arguments: " << error.what() << "\n";
        return 64;
    }
}
