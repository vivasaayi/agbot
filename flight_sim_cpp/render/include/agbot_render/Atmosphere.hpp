#pragma once

#include "agbot_flight_sim/WeatherPreset.hpp"
#include "agbot_render/Mat4.hpp"

#include <vector>

// M7 batch 3 — deterministic atmosphere & lighting (v1).
//
// Analytic sky (Preetham), aerial-perspective haze, and a day/night lighting
// state, all driven by a recorded WeatherPreset (its solar position + visibility
// + clouds). Plus data-driven night lighting placed along road polylines by
// hierarchy. Everything here is a pure, deterministic CPU function — no GPU, no
// clock — so it can be gated like the rest of the compiler.
namespace agbot::render {

// Linear (un-tonemapped) RGB radiance, relative units.
struct Rgb {
    float r = 0.0F;
    float g = 0.0F;
    float b = 0.0F;
};

// Unit direction toward the sun in the repo world frame (X east, Y up, Z north)
// from a solar position. Below-horizon positions have a negative y.
[[nodiscard]] Vec3f sun_direction(const agbot::flight_sim::SolarPosition& sun);

// Preetham analytic clear-sky radiance for a view direction, given the sun
// direction and atmospheric turbidity (~2 clear .. ~10 hazy). Returns relative
// linear RGB; when the sun is below the horizon a dim night sky is returned.
[[nodiscard]] Rgb preetham_sky(const Vec3f& view_dir, const Vec3f& sun_dir, double turbidity);

// Exponential aerial-perspective haze: blend a surface colour toward the haze
// colour as distance grows relative to visibility (~95% haze at one visibility).
[[nodiscard]] Rgb aerial_perspective(const Rgb& surface, const Rgb& haze, double distance_m,
                                     double visibility_m);

// Day/night lighting summary derived from the preset's solar elevation.
struct LightingState {
    Vec3f sun_dir;              // toward the sun (world frame)
    double sun_intensity = 0.0; // 0 at/below horizon -> ~1 at zenith
    double ambient = 0.0;       // sky-fill floor (never fully black)
    double turbidity = 2.5;     // haze proxy from visibility
    bool artificial_lights_on = false; // true from civil dusk through dawn
};

[[nodiscard]] LightingState lighting_from_preset(const agbot::flight_sim::WeatherPreset& preset);

// A placed emissive light (street lamp / façade glow).
struct PointLight {
    Vec3f position;
    Rgb color;
    float intensity = 1.0F;
};

struct NightLightingParams {
    double spacing_m = 40.0; // base lamp spacing along a road centerline
    double height_m = 8.0;   // lamp height above the polyline
    Rgb color{1.0F, 0.85F, 0.6F}; // warm sodium/LED street light
    double intensity = 1.0;
};

// Place street lamps along road polylines by hierarchy. `importance` (parallel
// to `roads`, values in [0,1]; empty => all 0.5) makes major roads denser and
// brighter. Lamps are spaced by arc length and raised to height_m. Deterministic.
[[nodiscard]] std::vector<PointLight> night_lights_from_roads(
    const std::vector<std::vector<Vec3f>>& roads,
    const std::vector<double>& importance = {},
    const NightLightingParams& params = {});

} // namespace agbot::render
