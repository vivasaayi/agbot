#include "agbot_flight_sim/WeatherPreset.hpp"

#include <algorithm>
#include <cmath>
#include <sstream>

namespace agbot::flight_sim {

namespace {

constexpr double kPi = 3.14159265358979323846;

double rad(double deg) { return deg * kPi / 180.0; }
double deg(double r) { return r * 180.0 / kPi; }

// Julian Day Number at 12:00 UT for a Gregorian date (Fliegel-Van Flandern).
long julian_day_number(int y, int m, int d) {
    const long a = (14 - m) / 12;
    const long yy = y + 4800 - a;
    const long mm = m + 12 * a - 3;
    return d + (153 * mm + 2) / 5 + 365 * yy + yy / 4 - yy / 100 + yy / 400 - 32045;
}

double clamp(double v, double lo, double hi) { return std::max(lo, std::min(hi, v)); }

} // namespace

SolarPosition solar_position_utc(int year, int month, int day, int hour, int minute, double second,
                                 double lat_deg, double lon_deg) {
    // NOAA solar-position equations (see the NOAA Solar Calculator spreadsheet).
    const double jdn = static_cast<double>(julian_day_number(year, month, day));
    // JDN is at noon UT; shift by the UT fraction relative to noon.
    const double jd = jdn + (hour - 12) / 24.0 + minute / 1440.0 + second / 86400.0;
    const double T = (jd - 2451545.0) / 36525.0;

    const double L0 = std::fmod(280.46646 + T * (36000.76983 + T * 0.0003032), 360.0);
    const double M = 357.52911 + T * (35999.05029 - 0.0001537 * T);
    const double e = 0.016708634 - T * (0.000042037 + 0.0000001267 * T);
    const double Mr = rad(M);
    const double C = std::sin(Mr) * (1.914602 - T * (0.004817 + 0.000014 * T)) +
        std::sin(2 * Mr) * (0.019993 - 0.000101 * T) + std::sin(3 * Mr) * 0.000289;
    const double true_long = L0 + C;
    const double omega = 125.04 - 1934.136 * T;
    const double app_long = true_long - 0.00569 - 0.00478 * std::sin(rad(omega));
    const double e0 =
        23.0 + (26.0 + (21.448 - T * (46.815 + T * (0.00059 - T * 0.001813))) / 60.0) / 60.0;
    const double e_corr = e0 + 0.00256 * std::cos(rad(omega));
    const double decl = deg(std::asin(std::sin(rad(e_corr)) * std::sin(rad(app_long))));

    // Equation of time (minutes).
    double vy = std::tan(rad(e_corr / 2.0));
    vy *= vy;
    const double L0r = rad(L0);
    const double eot = 4.0 *
        deg(vy * std::sin(2 * L0r) - 2 * e * std::sin(Mr) +
            4 * e * vy * std::sin(Mr) * std::cos(2 * L0r) - 0.5 * vy * vy * std::sin(4 * L0r) -
            1.25 * e * e * std::sin(2 * Mr));

    // True solar time (minutes) at the site, then the hour angle.
    const double minutes = hour * 60.0 + minute + second / 60.0;
    const double true_solar_time = std::fmod(minutes + eot + 4.0 * lon_deg + 1440.0, 1440.0);
    double ha = true_solar_time / 4.0 - 180.0; // degrees
    if (ha < -180.0) {
        ha += 360.0;
    }

    const double lat_r = rad(lat_deg);
    const double decl_r = rad(decl);
    const double ha_r = rad(ha);
    const double cos_zenith =
        clamp(std::sin(lat_r) * std::sin(decl_r) + std::cos(lat_r) * std::cos(decl_r) *
                                                       std::cos(ha_r),
              -1.0, 1.0);
    const double zenith = std::acos(cos_zenith);
    const double elevation = kPi / 2.0 - zenith;

    // Azimuth (clockwise from north).
    double azimuth = 0.0;
    const double az_denom = std::cos(lat_r) * std::sin(zenith);
    if (std::abs(az_denom) > 1e-9) {
        double az_arg = (std::sin(lat_r) * std::cos(zenith) - std::sin(decl_r)) / az_denom;
        az_arg = clamp(az_arg, -1.0, 1.0);
        const double az = std::acos(az_arg); // from north, before hemisphere fix
        azimuth = (ha > 0.0) ? std::fmod(deg(az) + 180.0, 360.0)
                             : std::fmod(540.0 - deg(az), 360.0);
    } else {
        azimuth = (lat_deg > 0.0) ? 180.0 : 0.0;
    }

    return {elevation, rad(azimuth)};
}

SolarPosition solar_position(const WeatherPreset& p) {
    return solar_position_utc(p.year, p.month, p.day, p.hour, p.minute, p.second, p.site_lat_deg,
                              p.site_lon_deg);
}

Vec3 wind_at_altitude(const WeatherPreset& p, double altitude_m) {
    if (p.aloft_ref_alt_m <= 0.0 || altitude_m <= 0.0) {
        return p.ground_wind_mps;
    }
    const double t = std::min(1.0, altitude_m / p.aloft_ref_alt_m);
    return {
        p.ground_wind_mps.x + (p.aloft_wind_mps.x - p.ground_wind_mps.x) * t,
        p.ground_wind_mps.y + (p.aloft_wind_mps.y - p.ground_wind_mps.y) * t,
        p.ground_wind_mps.z + (p.aloft_wind_mps.z - p.ground_wind_mps.z) * t,
    };
}

namespace {

std::string fmt(double v, int prec) {
    std::ostringstream out;
    out.setf(std::ios::fixed);
    out.precision(prec);
    out << v;
    return out.str();
}

} // namespace

std::string weather_preset_to_json(const WeatherPreset& p) {
    std::ostringstream out;
    out << "{\n";
    out << "  \"name\": \"" << p.name << "\",\n";
    out << "  \"utc\": \"" << p.year << "-" << p.month << "-" << p.day << "T" << p.hour << ":"
        << p.minute << ":" << fmt(p.second, 1) << "Z\",\n";
    out << "  \"site\": {\"lat\": " << fmt(p.site_lat_deg, 6) << ", \"lon\": "
        << fmt(p.site_lon_deg, 6) << "},\n";
    out << "  \"ground_wind_mps\": [" << fmt(p.ground_wind_mps.x, 3) << ", "
        << fmt(p.ground_wind_mps.y, 3) << ", " << fmt(p.ground_wind_mps.z, 3) << "],\n";
    out << "  \"aloft_wind_mps\": [" << fmt(p.aloft_wind_mps.x, 3) << ", "
        << fmt(p.aloft_wind_mps.y, 3) << ", " << fmt(p.aloft_wind_mps.z, 3)
        << "], \"aloft_ref_alt_m\": " << fmt(p.aloft_ref_alt_m, 1) << ",\n";
    out << "  \"visibility_m\": " << fmt(p.visibility_m, 1) << ",\n";
    out << "  \"cloud_layers\": [";
    for (std::size_t i = 0; i < p.cloud_layers.size(); ++i) {
        const CloudLayer& c = p.cloud_layers[i];
        out << (i == 0 ? "" : ", ") << "{\"base_alt_m\": " << fmt(c.base_alt_m, 1)
            << ", \"top_alt_m\": " << fmt(c.top_alt_m, 1) << ", \"coverage\": "
            << fmt(c.coverage, 3) << "}";
    }
    out << "],\n";
    out << "  \"precip\": {\"class\": " << static_cast<int>(p.precip_class) << ", \"rate_mmph\": "
        << fmt(p.precip_rate_mmph, 3) << "},\n";
    out << "  \"temperature_c\": " << fmt(p.temperature_c, 2) << ", \"pressure_hpa\": "
        << fmt(p.pressure_hpa, 2) << ", \"surface_wetness\": " << fmt(p.surface_wetness, 3) << "\n";
    out << "}\n";
    return out.str();
}

std::uint64_t weather_preset_hash(const WeatherPreset& p) {
    std::uint64_t acc = 1469598103934665603ULL;
    const auto fold = [&acc](std::int64_t v) {
        const auto bits = static_cast<std::uint64_t>(v);
        for (int i = 0; i < 8; ++i) {
            acc ^= (bits >> (i * 8)) & 0xFFu;
            acc *= 1099511628211ULL;
        }
    };
    const auto q = [](double v) { return static_cast<std::int64_t>(std::llround(v * 1000.0)); };
    for (char ch : p.name) {
        fold(static_cast<std::int64_t>(ch));
    }
    fold(p.year);
    fold(p.month);
    fold(p.day);
    fold(p.hour);
    fold(p.minute);
    fold(q(p.second));
    fold(q(p.site_lat_deg));
    fold(q(p.site_lon_deg));
    for (double v : {p.ground_wind_mps.x, p.ground_wind_mps.y, p.ground_wind_mps.z,
                     p.aloft_wind_mps.x, p.aloft_wind_mps.y, p.aloft_wind_mps.z, p.aloft_ref_alt_m,
                     p.visibility_m, p.precip_rate_mmph, p.temperature_c, p.pressure_hpa,
                     p.surface_wetness}) {
        fold(q(v));
    }
    fold(static_cast<std::int64_t>(p.precip_class));
    for (const CloudLayer& c : p.cloud_layers) {
        fold(q(c.base_alt_m));
        fold(q(c.top_alt_m));
        fold(q(c.coverage));
    }
    return acc;
}

WeatherPreset preset_clear_noon() {
    WeatherPreset p;
    p.name = "clear_noon";
    p.year = 2024;
    p.month = 6;
    p.day = 21;
    p.hour = 17; // ~solar noon over NYC in June
    p.minute = 0;
    p.ground_wind_mps = {3.0, 0.0, 0.0}; // light easterly-blowing breeze
    p.aloft_wind_mps = {8.0, 0.0, 2.0};
    p.visibility_m = 30000.0;
    p.temperature_c = 26.0;
    p.surface_wetness = 0.0;
    return p;
}

WeatherPreset preset_hazy_afternoon() {
    WeatherPreset p;
    p.name = "hazy_afternoon";
    p.year = 2024;
    p.month = 8;
    p.day = 15;
    p.hour = 20;
    p.minute = 30;
    p.ground_wind_mps = {1.0, 0.0, 2.0};
    p.aloft_wind_mps = {4.0, 0.0, 6.0};
    p.visibility_m = 6000.0; // haze
    p.cloud_layers = {{2500.0, 3200.0, 0.4}};
    p.temperature_c = 31.0;
    p.surface_wetness = 0.0;
    return p;
}

WeatherPreset preset_overcast_dusk() {
    WeatherPreset p;
    p.name = "overcast_dusk";
    p.year = 2024;
    p.month = 11;
    p.day = 5;
    p.hour = 22;
    p.minute = 15;
    p.ground_wind_mps = {-4.0, 0.0, 3.0};
    p.aloft_wind_mps = {-10.0, 0.0, 6.0};
    p.visibility_m = 8000.0;
    p.cloud_layers = {{600.0, 1800.0, 0.9}};
    p.precip_class = PrecipClass::Rain;
    p.precip_rate_mmph = 1.5;
    p.temperature_c = 9.0;
    p.pressure_hpa = 1004.0;
    p.surface_wetness = 0.8;
    return p;
}

WeatherPreset preset_clear_night() {
    WeatherPreset p;
    p.name = "clear_night";
    p.year = 2024;
    p.month = 1;
    p.day = 10;
    p.hour = 5; // ~midnight local
    p.minute = 0;
    p.ground_wind_mps = {0.5, 0.0, 0.5};
    p.aloft_wind_mps = {2.0, 0.0, 3.0};
    p.visibility_m = 25000.0;
    p.temperature_c = -3.0;
    p.surface_wetness = 0.0;
    return p;
}

} // namespace agbot::flight_sim
