// M7 batch 2 — deterministic weather presets: NOAA solar position, wind-aloft
// profile, deterministic serialization, and wiring into the flight model.

#include "agbot_flight_sim/WeatherPreset.hpp"
#include "agbot_vehicles/FixedWingModel.hpp"

#include <cmath>
#include <iostream>
#include <string>

namespace {

int failures = 0;

void expect(bool condition, const std::string& label) {
    std::cout << (condition ? "PASS " : "FAIL ") << label << "\n";
    if (!condition) {
        ++failures;
    }
}

constexpr double kPi = 3.14159265358979323846;
double deg(double r) { return r * 180.0 / kPi; }
bool near(double a, double b, double tol) { return std::abs(a - b) <= tol; }

using namespace agbot::flight_sim;

void test_solar_position_summer_noon() {
    // NYC, 2024-06-21 (summer solstice) near solar noon (~16:56 UTC). Max
    // elevation = 90 - (lat - decl) = 90 - (40.71 - 23.44) ~= 72.7 deg.
    const SolarPosition noon = solar_position_utc(2024, 6, 21, 17, 0, 0.0, 40.71, -74.0);
    std::cout << "  summer noon elevation " << deg(noon.elevation_rad) << " deg, azimuth "
              << deg(noon.azimuth_rad) << " deg\n";
    expect(deg(noon.elevation_rad) > 70.0 && deg(noon.elevation_rad) < 74.0,
           "summer solar noon elevation ~72.7 deg");
    expect(deg(noon.azimuth_rad) > 165.0 && deg(noon.azimuth_rad) < 200.0,
           "summer solar noon azimuth near south");

    // Three hours later the sun is lower and further west.
    const SolarPosition later = solar_position_utc(2024, 6, 21, 20, 0, 0.0, 40.71, -74.0);
    expect(later.elevation_rad < noon.elevation_rad, "sun descends after solar noon");
    expect(deg(later.azimuth_rad) > deg(noon.azimuth_rad),
           "azimuth advances westward through the afternoon");
}

void test_solar_position_night() {
    // NYC, 2024-01-10 05:00 UTC = midnight local; the sun is below the horizon.
    const SolarPosition night = solar_position_utc(2024, 1, 10, 5, 0, 0.0, 40.71, -74.0);
    std::cout << "  winter midnight elevation " << deg(night.elevation_rad) << " deg\n";
    expect(night.elevation_rad < 0.0, "winter local midnight sun is below the horizon");
}

void test_preset_solar_presets() {
    expect(solar_position(preset_clear_noon()).elevation_rad > 0.0, "clear_noon sun is up");
    expect(solar_position(preset_clear_night()).elevation_rad < 0.0, "clear_night sun is down");
}

void test_wind_profile() {
    WeatherPreset p = preset_clear_noon(); // ground {3,0,0}, aloft {8,0,2} @1000m
    const Vec3 w0 = wind_at_altitude(p, 0.0);
    const Vec3 w500 = wind_at_altitude(p, 500.0);
    const Vec3 w2000 = wind_at_altitude(p, 2000.0);
    expect(near(w0.x, 3.0, 1e-9) && near(w0.z, 0.0, 1e-9), "wind at ground equals ground wind");
    expect(near(w500.x, 5.5, 1e-9) && near(w500.z, 1.0, 1e-9),
           "wind at half the reference altitude blends ground->aloft");
    expect(near(w2000.x, 8.0, 1e-9) && near(w2000.z, 2.0, 1e-9),
           "wind above the reference altitude clamps to the aloft value");
}

void test_determinism() {
    const WeatherPreset a = preset_overcast_dusk();
    const WeatherPreset b = preset_overcast_dusk();
    expect(weather_preset_hash(a) == weather_preset_hash(b) && weather_preset_hash(a) != 0,
           "weather preset hash is deterministic");
    expect(weather_preset_to_json(a) == weather_preset_to_json(b),
           "weather preset JSON is byte-identical across calls");
    expect(weather_preset_hash(preset_clear_noon()) != weather_preset_hash(preset_clear_night()),
           "distinct presets hash differently");
    expect(weather_preset_to_json(a).find("\"surface_wetness\": 0.800") != std::string::npos,
           "overcast_dusk records wet surfaces");
}

void test_preset_drives_flight_model() {
    const WeatherPreset p = preset_overcast_dusk();
    const Vec3 w = wind_at_altitude(p, 300.0);
    agbot::vehicles::FixedWingModel model;
    model.set_initial_trim(300.0, 55.0, 0.0);
    model.set_wind(w);
    const agbot::vehicles::Vec3 got = model.wind();
    expect(near(got.x, w.x, 1e-9) && near(got.y, w.y, 1e-9) && near(got.z, w.z, 1e-9),
           "preset wind at altitude feeds the flight model");
    // A crosswind component induces sideslip within a few seconds of flight.
    double max_beta = 0.0;
    auto state = model.set_initial_trim(300.0, 55.0, 0.0);
    model.set_wind(w);
    for (int i = 0; i < 200; ++i) {
        state = model.step(state, agbot::vehicles::Actuation{}, 0.02);
        max_beta = std::max(max_beta, std::abs(model.aero_debug().beta_rad));
    }
    expect(max_beta > 1.0 * kPi / 180.0, "preset crosswind induces sideslip in the model");
}

} // namespace

int main() {
    test_solar_position_summer_noon();
    test_solar_position_night();
    test_preset_solar_presets();
    test_wind_profile();
    test_determinism();
    test_preset_drives_flight_model();

    if (failures > 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all weather preset tests passed\n";
    return 0;
}
