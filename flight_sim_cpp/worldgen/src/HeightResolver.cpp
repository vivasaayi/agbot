#include "agbot_worldgen/HeightResolver.hpp"

#include <cmath>
#include <cstdlib>

namespace agbot::worldgen {

const char* to_string(HeightSource source) {
    switch (source) {
        case HeightSource::Attribute:
            return "attr";
        case HeightSource::Levels:
            return "levels";
        case HeightSource::Default:
            return "default";
    }
    return "default";
}

HeightResolution resolve_height(
    const std::optional<double>& height_attr_value,
    const std::optional<double>& levels_value,
    const HeightResolverParams& params) {
    if (height_attr_value.has_value()) {
        const double height = *height_attr_value * params.attr_unit_scale_m;
        if (std::isfinite(height) && height > 0.0) {
            return {height, HeightSource::Attribute};
        }
    }
    if (levels_value.has_value()) {
        const double height = *levels_value * params.default_level_height_m;
        if (std::isfinite(height) && height > 0.0) {
            return {height, HeightSource::Levels};
        }
    }
    return {params.default_height_m, HeightSource::Default};
}

std::optional<double> parse_numeric_attribute(const std::string& raw) {
    if (raw.empty()) {
        return std::nullopt;
    }
    const char* begin = raw.c_str();
    char* end = nullptr;
    const double value = std::strtod(begin, &end);
    if (end == begin || !std::isfinite(value)) {
        return std::nullopt;
    }
    // Reject trailing garbage beyond whitespace.
    while (*end == ' ' || *end == '\t') {
        ++end;
    }
    if (*end != '\0') {
        return std::nullopt;
    }
    return value;
}

} // namespace agbot::worldgen
