#pragma once

#include <optional>
#include <string>

namespace agbot::worldgen {

// Where a resolved building height came from, in precedence order.
enum class HeightSource {
    Attribute,
    Levels,
    Default,
};

[[nodiscard]] const char* to_string(HeightSource source);

struct HeightResolution {
    double height_m = 0.0;
    HeightSource source = HeightSource::Default;
};

struct HeightResolverParams {
    // Multiplier applied to the raw height attribute (0.3048 for feet).
    double attr_unit_scale_m = 1.0;
    double default_level_height_m = 3.0;
    double default_height_m = 3.0;
};

// Resolves a height with precedence: explicit attribute > levels x storey
// height > configured default. Non-finite or non-positive candidates fall
// through to the next source.
[[nodiscard]] HeightResolution resolve_height(
    const std::optional<double>& height_attr_value,
    const std::optional<double>& levels_value,
    const HeightResolverParams& params);

// Parses a numeric property that Socrata-style GeoJSON may serialize as a
// string ("33.49") or a number. Returns nullopt for absent/non-numeric input.
[[nodiscard]] std::optional<double> parse_numeric_attribute(const std::string& raw);

} // namespace agbot::worldgen
