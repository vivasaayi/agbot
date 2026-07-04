#include "agbot_render/Atmosphere.hpp"

#include <algorithm>
#include <cmath>

namespace agbot::render {

namespace {

constexpr double kPi = 3.14159265358979323846;

double clampd(double v, double lo, double hi) { return std::max(lo, std::min(hi, v)); }

// Perez distribution F(theta, gamma) = (1 + A e^{B/cos theta})(1 + C e^{D gamma}
// + E cos^2 gamma). cos_theta is clamped away from 0 at the horizon.
double perez(double cos_theta, double gamma, const double c[5]) {
    const double ct = std::max(cos_theta, 0.01);
    const double cg = std::cos(gamma);
    return (1.0 + c[0] * std::exp(c[1] / ct)) *
        (1.0 + c[2] * std::exp(c[3] * gamma) + c[4] * cg * cg);
}

// XYZ -> linear sRGB (Rec.709 primaries, D65).
Rgb xyz_to_rgb(double X, double Y, double Z) {
    Rgb out;
    out.r = static_cast<float>(3.2406 * X - 1.5372 * Y - 0.4986 * Z);
    out.g = static_cast<float>(-0.9689 * X + 1.8758 * Y + 0.0415 * Z);
    out.b = static_cast<float>(0.0557 * X - 0.2040 * Y + 1.0570 * Z);
    out.r = std::max(0.0F, out.r);
    out.g = std::max(0.0F, out.g);
    out.b = std::max(0.0F, out.b);
    return out;
}

} // namespace

Vec3f sun_direction(const agbot::flight_sim::SolarPosition& sun) {
    const double el = sun.elevation_rad;
    const double az = sun.azimuth_rad; // clockwise from north (+Z)
    const double ch = std::cos(el);
    return Vec3f{
        static_cast<float>(ch * std::sin(az)), // east  (+X)
        static_cast<float>(std::sin(el)),      // up    (+Y)
        static_cast<float>(ch * std::cos(az)), // north (+Z)
    };
}

Rgb preetham_sky(const Vec3f& view_dir, const Vec3f& sun_dir, double turbidity) {
    // Night: the analytic clear-sky model is only valid with the sun up.
    if (sun_dir.y <= 0.0F) {
        return Rgb{0.02F, 0.03F, 0.06F};
    }
    const double T = clampd(turbidity, 1.8, 10.0);

    // Perez coefficients as linear functions of turbidity (Preetham 1999).
    const double cy[5] = {0.1787 * T - 1.4630, -0.3554 * T + 0.4275, -0.0227 * T + 5.3251,
                          0.1206 * T - 2.5771, -0.0670 * T + 0.3703};
    const double cx[5] = {-0.0193 * T - 0.2592, -0.0665 * T + 0.0008, -0.0004 * T + 0.2125,
                          -0.0641 * T - 0.8989, -0.0033 * T + 0.0452};
    const double cxy[5] = {-0.0167 * T - 0.2608, -0.0950 * T + 0.0092, -0.0079 * T + 0.2102,
                           -0.0441 * T - 1.6537, -0.0109 * T + 0.0529};

    // View / sun geometry. y is up; theta is measured from the zenith.
    const double cos_theta = clampd(view_dir.y, 0.0, 1.0);
    const double cos_gamma =
        clampd(view_dir.x * sun_dir.x + view_dir.y * sun_dir.y + view_dir.z * sun_dir.z, -1.0, 1.0);
    const double gamma = std::acos(cos_gamma);
    const double cos_thetas = clampd(sun_dir.y, 0.0, 1.0);
    const double thetas = std::acos(cos_thetas);
    const double gamma_s = 0.0; // sun-to-sun angle

    // Zenith chromaticity (Preetham polynomials in turbidity and sun zenith).
    const double ts = thetas;
    const double ts2 = ts * ts;
    const double ts3 = ts2 * ts;
    const double T2 = T * T;
    const double xz =
        (0.00166 * ts3 - 0.00375 * ts2 + 0.00209 * ts) * T2 +
        (-0.02903 * ts3 + 0.06377 * ts2 - 0.03202 * ts + 0.00394) * T +
        (0.11693 * ts3 - 0.21196 * ts2 + 0.06052 * ts + 0.25886);
    const double yz =
        (0.00275 * ts3 - 0.00610 * ts2 + 0.00317 * ts) * T2 +
        (-0.04214 * ts3 + 0.08970 * ts2 - 0.04153 * ts + 0.00516) * T +
        (0.15346 * ts3 - 0.26756 * ts2 + 0.06670 * ts + 0.26688);

    // Relative luminance via the Perez ratio (zenith-normalized); absolute
    // zenith luminance is folded into a fixed exposure below.
    const double fy_view = perez(cos_theta, gamma, cy);
    const double fy_zenith = perez(1.0, thetas, cy);
    const double Yrel = fy_view / std::max(fy_zenith, 1e-4);

    const double x = xz * perez(cos_theta, gamma, cx) / std::max(perez(1.0, gamma_s, cx), 1e-4);
    const double y = yz * perez(cos_theta, gamma, cxy) / std::max(perez(1.0, gamma_s, cxy), 1e-4);

    // xyY -> XYZ with a fixed exposure so relative output sits in a sane range.
    const double Y = Yrel * 0.18;
    const double yy = std::max(y, 1e-4);
    const double X = x / yy * Y;
    const double Z = (1.0 - x - y) / yy * Y;
    return xyz_to_rgb(X, Y, Z);
}

Rgb aerial_perspective(const Rgb& surface, const Rgb& haze, double distance_m,
                       double visibility_m) {
    const double vis = std::max(visibility_m, 1.0);
    const double t = clampd(1.0 - std::exp(-3.0 * std::max(distance_m, 0.0) / vis), 0.0, 1.0);
    const float tf = static_cast<float>(t);
    return Rgb{
        surface.r * (1.0F - tf) + haze.r * tf,
        surface.g * (1.0F - tf) + haze.g * tf,
        surface.b * (1.0F - tf) + haze.b * tf,
    };
}

LightingState lighting_from_preset(const agbot::flight_sim::WeatherPreset& preset) {
    const agbot::flight_sim::SolarPosition sun = agbot::flight_sim::solar_position(preset);
    LightingState s;
    s.sun_dir = sun_direction(sun);
    const double sin_el = std::sin(sun.elevation_rad);
    s.sun_intensity = std::max(0.0, sin_el);
    s.ambient = 0.03 + 0.35 * std::max(0.0, sin_el); // twilight floor .. day fill
    // Turbidity proxy: clearer air (higher visibility) -> lower turbidity.
    s.turbidity = clampd(2.0 + 60000.0 / std::max(preset.visibility_m, 1000.0) * 0.15, 2.0, 10.0);
    // Civil dusk/dawn: artificial lighting from ~6 deg above the horizon down.
    s.artificial_lights_on = sun.elevation_rad < (6.0 * kPi / 180.0);
    return s;
}

std::vector<PointLight> night_lights_from_roads(const std::vector<std::vector<Vec3f>>& roads,
                                                const std::vector<double>& importance,
                                                const NightLightingParams& params) {
    std::vector<PointLight> lights;
    for (std::size_t ri = 0; ri < roads.size(); ++ri) {
        const std::vector<Vec3f>& road = roads[ri];
        if (road.size() < 2) {
            continue;
        }
        const double imp = ri < importance.size() ? clampd(importance[ri], 0.0, 1.0) : 0.5;
        // Major roads (higher importance) are lit denser and brighter.
        const double spacing = std::max(5.0, params.spacing_m / (0.5 + imp));
        const float intensity = static_cast<float>(params.intensity * (0.6 + 0.4 * imp));

        double dist_since = spacing; // place a lamp at the first vertex
        for (std::size_t i = 0; i + 1 < road.size(); ++i) {
            const Vec3f a = road[i];
            const Vec3f b = road[i + 1];
            const double dx = b.x - a.x;
            const double dy = b.y - a.y;
            const double dz = b.z - a.z;
            const double seg = std::sqrt(dx * dx + dy * dy + dz * dz);
            if (seg < 1e-6) {
                continue;
            }
            double s = 0.0;
            while (dist_since + (seg - s) >= spacing) {
                s += spacing - dist_since;
                dist_since = 0.0;
                const double t = s / seg;
                PointLight lamp;
                lamp.position = Vec3f{
                    static_cast<float>(a.x + dx * t),
                    static_cast<float>(a.y + dy * t + params.height_m),
                    static_cast<float>(a.z + dz * t),
                };
                lamp.color = params.color;
                lamp.intensity = intensity;
                lights.push_back(lamp);
            }
            dist_since += seg - s;
        }
    }
    return lights;
}

} // namespace agbot::render
