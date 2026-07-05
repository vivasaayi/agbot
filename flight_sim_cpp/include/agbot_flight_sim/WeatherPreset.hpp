#pragma once

#include "agbot_flight_sim/Vec3.hpp"

#include <cstdint>
#include <string>
#include <vector>

// M7 batch 2 — deterministic weather presets.
//
// A recorded weather preset snapshots every meteorological input needed to
// drive flight dynamics (wind ground+aloft) and, later, atmosphere rendering
// (sun position, visibility, clouds, precipitation, wetness). Presets are pure
// data with no wall clock: the UTC timestamp is explicit, so sun position and
// everything derived from it are reproducible. Live METAR/TAF assimilation is
// deferred; the same schema is the target for it.
namespace agbot::flight_sim {

// Solar position for a site + instant. Elevation is measured above the horizon
// (negative below); azimuth is clockwise from true north in [0, 2pi).
struct SolarPosition {
    double elevation_rad = 0.0;
    double azimuth_rad = 0.0;
};

// A cloud deck. coverage is the sky fraction in [0, 1] (oktas/8).
struct CloudLayer {
    double base_alt_m = 0.0;
    double top_alt_m = 0.0;
    double coverage = 0.0;
};

enum class PrecipClass { None = 0, Rain = 1, Snow = 2 };

// The recorded weather state. Wind vectors are in the repo world frame (X east,
// Y up, Z north) as "blowing-toward" vectors in m/s, matching
// FixedWingModel::set_wind.
struct WeatherPreset {
    std::string name;

    // Explicit UTC timestamp (no clock dependency).
    int year = 2024;
    int month = 6;
    int day = 21;
    int hour = 17;
    int minute = 0;
    double second = 0.0;

    // Site used for the solar-position calculation.
    double site_lat_deg = 40.71;
    double site_lon_deg = -74.0;

    // Wind: ground value and the value at aloft_ref_alt_m; linearly blended in
    // between, constant above.
    Vec3 ground_wind_mps{0.0, 0.0, 0.0};
    Vec3 aloft_wind_mps{0.0, 0.0, 0.0};
    double aloft_ref_alt_m = 1000.0;

    // Atmosphere / surface.
    double visibility_m = 20000.0;
    std::vector<CloudLayer> cloud_layers;
    PrecipClass precip_class = PrecipClass::None;
    double precip_rate_mmph = 0.0;
    double temperature_c = 15.0;
    double pressure_hpa = 1013.25;
    double surface_wetness = 0.0; // 0 dry .. 1 saturated (roads/roofs)
};

// NOAA solar-position algorithm (deterministic). Latitude/longitude in degrees
// (east-positive longitude).
[[nodiscard]] SolarPosition solar_position_utc(int year, int month, int day, int hour, int minute,
                                               double second, double lat_deg, double lon_deg);
[[nodiscard]] SolarPosition solar_position(const WeatherPreset& preset);

// Wind in the world frame at a given altitude (m), blending ground->aloft.
[[nodiscard]] Vec3 wind_at_altitude(const WeatherPreset& preset, double altitude_m);

// Deterministic JSON (fixed key order + float formatting) and a content hash.
[[nodiscard]] std::string weather_preset_to_json(const WeatherPreset& preset);
[[nodiscard]] std::uint64_t weather_preset_hash(const WeatherPreset& preset);

// Standard recorded presets for acceptance / demo use.
[[nodiscard]] WeatherPreset preset_clear_noon();
[[nodiscard]] WeatherPreset preset_hazy_afternoon();
[[nodiscard]] WeatherPreset preset_overcast_dusk();
[[nodiscard]] WeatherPreset preset_clear_night();

} // namespace agbot::flight_sim
