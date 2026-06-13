#include "agbot_flight_sim/TwinContractV1.hpp"

#include "agbot_flight_sim/DeterministicRunner.hpp"

#include <algorithm>
#include <array>
#include <charconv>
#include <cstddef>
#include <iomanip>
#include <sstream>
#include <system_error>

namespace agbot::flight_sim {
namespace {

std::string escape_json(std::string_view value) {
    std::ostringstream output;
    for (const char c : value) {
        switch (c) {
            case '"':
                output << "\\\"";
                break;
            case '\\':
                output << "\\\\";
                break;
            case '\n':
                output << "\\n";
                break;
            case '\r':
                output << "\\r";
                break;
            case '\t':
                output << "\\t";
                break;
            default:
                output << c;
                break;
        }
    }
    return output.str();
}

std::uint32_t rotate_right(std::uint32_t value, std::uint32_t amount) {
    return (value >> amount) | (value << (32U - amount));
}

std::string bytes_to_hex(const std::array<std::uint8_t, 32>& bytes) {
    std::ostringstream output;
    output << std::hex << std::setfill('0');
    for (const auto byte : bytes) {
        output << std::setw(2) << static_cast<unsigned int>(byte);
    }
    return output.str();
}

std::string schema_json_without_hash(const TwinContractSchemaV1& schema) {
    std::ostringstream stream;
    stream << "{\"name\":\"" << escape_json(schema.name) << "\""
           << ",\"version\":\"" << escape_json(schema.version) << "\""
           << ",\"types\":[";
    for (std::size_t type_index = 0; type_index < schema.types.size(); ++type_index) {
        if (type_index > 0) {
            stream << ",";
        }
        const auto& type = schema.types[type_index];
        stream << "{\"name\":\"" << escape_json(type.name) << "\",\"required_fields\":[";
        for (std::size_t field_index = 0; field_index < type.required_fields.size(); ++field_index) {
            if (field_index > 0) {
                stream << ",";
            }
            stream << "\"" << escape_json(type.required_fields[field_index]) << "\"";
        }
        stream << "]}";
    }
    stream << "],\"capabilities\":[";
    for (std::size_t index = 0; index < schema.capabilities.size(); ++index) {
        if (index > 0) {
            stream << ",";
        }
        stream << "\"" << escape_json(schema.capabilities[index]) << "\"";
    }
    stream << "]}";
    return stream.str();
}

bool parse_semver(std::string_view value, int& major, int& minor, int& patch) {
    const std::size_t first_dot = value.find('.');
    const std::size_t second_dot = first_dot == std::string_view::npos
        ? std::string_view::npos
        : value.find('.', first_dot + 1);
    if (first_dot == std::string_view::npos || second_dot == std::string_view::npos) {
        return false;
    }
    const auto parse_part = [](std::string_view text, int& out) {
        if (text.empty()) {
            return false;
        }
        const char* begin = text.data();
        const char* end = text.data() + text.size();
        const auto [ptr, ec] = std::from_chars(begin, end, out);
        return ec == std::errc {} && ptr == end;
    };
    return parse_part(value.substr(0, first_dot), major)
        && parse_part(value.substr(first_dot + 1, second_dot - first_dot - 1), minor)
        && parse_part(value.substr(second_dot + 1), patch);
}

TwinContractSchemaV1 build_schema() {
    TwinContractSchemaV1 schema;
    schema.name = "TwinContractV1";
    schema.version = kTwinContractVersion;
    schema.types = {
        {
            "FlightCommandV1",
            {
                "contract_version",
                "command_id",
                "command_type",
                "issued_at_unix_ms",
                "payload",
                "ack_timeout_ms",
            },
        },
        {
            "TelemetryV1",
            {
                "contract_version",
                "command_id",
                "time_s",
                "mode",
                "position",
                "velocity",
                "attitude",
                "battery_percent",
                "target_waypoint_index",
                "armed",
            },
        },
        {
            "SimulationTraceV1",
            {
                "contract_version",
                "trace_schema",
                "samples",
                "sample_count",
                "output_hash",
            },
        },
        {
            "LidarPointV1",
            {
                "timestamp",
                "angle",
                "distance",
                "quality",
            },
        },
        {
            "LidarScanV1",
            {
                "timestamp",
                "points",
                "scan_id",
            },
        },
        {
            "ScenarioManifestV1",
            {
                "simulator_version",
                "contract_version",
                "contract_schema_hash",
                "run_id",
                "seed",
                "timestep_s",
                "record_interval_s",
                "mission_name",
                "mission_hash",
                "terrain_tiles",
                "terrain_tiles_hash",
                "weather_config",
                "weather_config_hash",
                "sensor_config",
                "sensor_config_hash",
                "lidar_config",
                "lidar_config_hash",
                "lidar_scan_count",
                "lidar_output_hash",
                "safety_config",
                "safety_config_hash",
                "trace_retention_keep",
                "trace_retention_deleted",
                "faults",
                "faults_hash",
                "fault_events",
                "fault_events_hash",
                "output_hash",
                "completed",
            },
        },
        {
            "TwinErrorV1",
            {
                "contract_version",
                "code",
                "message",
                "retryable",
            },
        },
        {
            "TwinCommandAckV1",
            {
                "contract_version",
                "command_id",
                "accepted",
                "error",
                "telemetry",
            },
        },
        {
            "TwinCapabilitiesV1",
            {
                "contract_version",
                "capabilities",
                "simulator_version",
            },
        },
    };
    schema.capabilities = {
        "deterministic_runner",
        "golden_trace",
        "scenario_manifest",
        "trace_diff",
        "safety_parity",
        "terrain_no_data_state",
        "simulation_health",
        "trace_retention",
        "tile_cache_control",
        "fault_injection",
        "terrain_crs_extent_assertions",
        "wind_field",
        "sensor_noise_calibration",
        "lidar_raycast",
        "twin_backend_api",
        "shared_command_telemetry_contract",
    };
    schema.schema_hash = sha256_hex(schema_json_without_hash(schema));
    return schema;
}

} // namespace

std::string sha256_hex(std::string_view bytes) {
    static constexpr std::array<std::uint32_t, 64> k {
        0x428a2f98U, 0x71374491U, 0xb5c0fbcfU, 0xe9b5dba5U,
        0x3956c25bU, 0x59f111f1U, 0x923f82a4U, 0xab1c5ed5U,
        0xd807aa98U, 0x12835b01U, 0x243185beU, 0x550c7dc3U,
        0x72be5d74U, 0x80deb1feU, 0x9bdc06a7U, 0xc19bf174U,
        0xe49b69c1U, 0xefbe4786U, 0x0fc19dc6U, 0x240ca1ccU,
        0x2de92c6fU, 0x4a7484aaU, 0x5cb0a9dcU, 0x76f988daU,
        0x983e5152U, 0xa831c66dU, 0xb00327c8U, 0xbf597fc7U,
        0xc6e00bf3U, 0xd5a79147U, 0x06ca6351U, 0x14292967U,
        0x27b70a85U, 0x2e1b2138U, 0x4d2c6dfcU, 0x53380d13U,
        0x650a7354U, 0x766a0abbU, 0x81c2c92eU, 0x92722c85U,
        0xa2bfe8a1U, 0xa81a664bU, 0xc24b8b70U, 0xc76c51a3U,
        0xd192e819U, 0xd6990624U, 0xf40e3585U, 0x106aa070U,
        0x19a4c116U, 0x1e376c08U, 0x2748774cU, 0x34b0bcb5U,
        0x391c0cb3U, 0x4ed8aa4aU, 0x5b9cca4fU, 0x682e6ff3U,
        0x748f82eeU, 0x78a5636fU, 0x84c87814U, 0x8cc70208U,
        0x90befffaU, 0xa4506cebU, 0xbef9a3f7U, 0xc67178f2U,
    };

    std::vector<std::uint8_t> message(bytes.begin(), bytes.end());
    const std::uint64_t bit_length = static_cast<std::uint64_t>(message.size()) * 8ULL;
    message.push_back(0x80U);
    while ((message.size() % 64U) != 56U) {
        message.push_back(0U);
    }
    for (int shift = 56; shift >= 0; shift -= 8) {
        message.push_back(static_cast<std::uint8_t>((bit_length >> shift) & 0xffU));
    }

    std::uint32_t h0 = 0x6a09e667U;
    std::uint32_t h1 = 0xbb67ae85U;
    std::uint32_t h2 = 0x3c6ef372U;
    std::uint32_t h3 = 0xa54ff53aU;
    std::uint32_t h4 = 0x510e527fU;
    std::uint32_t h5 = 0x9b05688cU;
    std::uint32_t h6 = 0x1f83d9abU;
    std::uint32_t h7 = 0x5be0cd19U;

    for (std::size_t chunk = 0; chunk < message.size(); chunk += 64U) {
        std::array<std::uint32_t, 64> w {};
        for (std::size_t index = 0; index < 16U; ++index) {
            const std::size_t offset = chunk + index * 4U;
            w[index] = (static_cast<std::uint32_t>(message[offset]) << 24U)
                | (static_cast<std::uint32_t>(message[offset + 1U]) << 16U)
                | (static_cast<std::uint32_t>(message[offset + 2U]) << 8U)
                | static_cast<std::uint32_t>(message[offset + 3U]);
        }
        for (std::size_t index = 16U; index < 64U; ++index) {
            const std::uint32_t s0 = rotate_right(w[index - 15U], 7U)
                ^ rotate_right(w[index - 15U], 18U)
                ^ (w[index - 15U] >> 3U);
            const std::uint32_t s1 = rotate_right(w[index - 2U], 17U)
                ^ rotate_right(w[index - 2U], 19U)
                ^ (w[index - 2U] >> 10U);
            w[index] = w[index - 16U] + s0 + w[index - 7U] + s1;
        }

        std::uint32_t a = h0;
        std::uint32_t b = h1;
        std::uint32_t c = h2;
        std::uint32_t d = h3;
        std::uint32_t e = h4;
        std::uint32_t f = h5;
        std::uint32_t g = h6;
        std::uint32_t h = h7;

        for (std::size_t index = 0; index < 64U; ++index) {
            const std::uint32_t s1 = rotate_right(e, 6U) ^ rotate_right(e, 11U) ^ rotate_right(e, 25U);
            const std::uint32_t ch = (e & f) ^ ((~e) & g);
            const std::uint32_t temp1 = h + s1 + ch + k[index] + w[index];
            const std::uint32_t s0 = rotate_right(a, 2U) ^ rotate_right(a, 13U) ^ rotate_right(a, 22U);
            const std::uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
            const std::uint32_t temp2 = s0 + maj;
            h = g;
            g = f;
            f = e;
            e = d + temp1;
            d = c;
            c = b;
            b = a;
            a = temp1 + temp2;
        }

        h0 += a;
        h1 += b;
        h2 += c;
        h3 += d;
        h4 += e;
        h5 += f;
        h6 += g;
        h7 += h;
    }

    std::array<std::uint8_t, 32> digest {};
    const std::array<std::uint32_t, 8> words {h0, h1, h2, h3, h4, h5, h6, h7};
    for (std::size_t index = 0; index < words.size(); ++index) {
        digest[index * 4U] = static_cast<std::uint8_t>((words[index] >> 24U) & 0xffU);
        digest[index * 4U + 1U] = static_cast<std::uint8_t>((words[index] >> 16U) & 0xffU);
        digest[index * 4U + 2U] = static_cast<std::uint8_t>((words[index] >> 8U) & 0xffU);
        digest[index * 4U + 3U] = static_cast<std::uint8_t>(words[index] & 0xffU);
    }
    return bytes_to_hex(digest);
}

bool TwinContractSchemaV1::has_type(std::string_view type_name) const {
    return std::any_of(types.begin(), types.end(), [&](const auto& type) {
        return type.name == type_name;
    });
}

bool TwinContractSchemaV1::type_has_field(std::string_view type_name, std::string_view field_name) const {
    const auto type_iter = std::find_if(types.begin(), types.end(), [&](const auto& type) {
        return type.name == type_name;
    });
    if (type_iter == types.end()) {
        return false;
    }
    return std::any_of(type_iter->required_fields.begin(), type_iter->required_fields.end(), [&](const auto& field) {
        return field == field_name;
    });
}

bool TwinContractSchemaV1::has_capability(std::string_view capability) const {
    return std::any_of(capabilities.begin(), capabilities.end(), [&](const auto& item) {
        return item == capability;
    });
}

std::string TwinContractSchemaV1::to_json() const {
    std::string json = schema_json_without_hash(*this);
    json.pop_back();
    json += ",\"schema_hash\":\"" + escape_json(schema_hash) + "\"}";
    return json;
}

const TwinContractSchemaV1& twin_contract_v1_schema() {
    static const TwinContractSchemaV1 schema = build_schema();
    return schema;
}

bool is_compatible_contract_version(std::string_view consumer_version, std::string_view producer_version) {
    int consumer_major = 0;
    int consumer_minor = 0;
    int consumer_patch = 0;
    int producer_major = 0;
    int producer_minor = 0;
    int producer_patch = 0;
    if (!parse_semver(consumer_version, consumer_major, consumer_minor, consumer_patch)
        || !parse_semver(producer_version, producer_major, producer_minor, producer_patch)) {
        return false;
    }
    if (consumer_major != producer_major) {
        return false;
    }
    if (producer_minor < consumer_minor) {
        return false;
    }
    if (producer_minor == consumer_minor && producer_patch < consumer_patch) {
        return false;
    }
    return true;
}

} // namespace agbot::flight_sim
