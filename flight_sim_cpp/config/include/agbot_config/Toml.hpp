#pragma once

#include "agbot_config/Params.hpp"

#include <filesystem>
#include <string>

namespace agbot::config {

struct TomlParseResult {
    bool ok = false;
    std::string error;   // human-readable, includes line number on failure
    ParamTable root;
};

// Minimal TOML subset parser sufficient for engine configuration files:
//   - comments (#), blank lines
//   - [table] and nested [table.subtable] headers
//   - [[array-of-tables]] headers
//   - key = value with basic strings "...", integers, floats, booleans
//   - arrays of scalars [1, 2.5, "x", true]
//   - inline tables { key = value, ... }
//   - dotted keys inside headers only (not in key positions)
// Unsupported TOML syntax fails with a clear error instead of silently misparsing.
TomlParseResult parse_toml(const std::string& text);
TomlParseResult parse_toml_file(const std::filesystem::path& path);

} // namespace agbot::config
