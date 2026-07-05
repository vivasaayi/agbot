#include "agbot_vehicles/IVehicleModel.hpp"
#include "agbot_vehicles/KinematicBicycleModel.hpp"
#include "agbot_vehicles/MultirotorModel.hpp"
#include "agbot_vehicles/VehicleTypes.hpp"

#include <cmath>
#include <iostream>
#include <memory>
#include <string>

namespace {

int failures = 0;

void expect(bool condition, const std::string& label) {
    if (condition) {
        std::cout << "PASS " << label << "\n";
    } else {
        std::cout << "FAIL " << label << "\n";
        ++failures;
    }
}

using agbot::vehicles::Actuation;
using agbot::vehicles::EntityState;
using agbot::vehicles::KinematicBicycleModel;
using agbot::vehicles::MultirotorModel;
using agbot::vehicles::VehicleKind;

EntityState state_with_speed(double speed_mps, double yaw_rad = 0.0) {
    EntityState state;
    state.yaw_rad = yaw_rad;
    state.velocity = {speed_mps * std::cos(yaw_rad), 0.0, speed_mps * std::sin(yaw_rad)};
    return state;
}

void test_bicycle_straight_line() {
    KinematicBicycleModel model;
    EntityState state = state_with_speed(5.0);
    const Actuation input{0.0, 0.0};
    for (int i = 0; i < 100; ++i) {
        state = model.step(state, input, 0.02);
    }
    expect(std::abs(state.position.x - 10.0) < 1e-9, "bicycle straight line advances exactly v*t");
    expect(std::abs(state.position.z) < 1e-12, "bicycle straight line stays on axis");
    expect(std::abs(state.yaw_rad) < 1e-12, "bicycle straight line keeps heading");
    expect(std::abs(state.time_s - 2.0) < 1e-9, "bicycle straight line advances time");
}

void test_bicycle_circle_radius() {
    agbot::config::ParamTable params;
    params["wheelbase_m"] = 2.0;
    params["max_steer_rad"] = 0.6;
    params["max_steer_rate_radps"] = 100.0; // effectively instant steer for this test
    params["max_speed_mps"] = 10.0;
    KinematicBicycleModel model(params);

    const double steer = 0.3;
    const double expected_radius = 2.0 / std::tan(steer);
    EntityState state = state_with_speed(5.0);
    const Actuation input{0.0, steer};

    // Warm up half a turn, then estimate the circle center from opposite points.
    const double yaw_rate = 5.0 / 2.0 * std::tan(steer);
    const double period_s = 2.0 * 3.14159265358979323846 / yaw_rate;
    const int steps_per_period = static_cast<int>(period_s / 0.01);
    agbot::flight_sim::Vec3 center{0.0, 0.0, 0.0};
    for (int i = 0; i < steps_per_period; ++i) {
        state = model.step(state, input, 0.01);
        center += state.position;
    }
    center = center / static_cast<double>(steps_per_period);

    double max_err = 0.0;
    EntityState probe = state;
    for (int i = 0; i < steps_per_period; ++i) {
        probe = model.step(probe, input, 0.01);
        const double radius = (probe.position - center).horizontal_length();
        max_err = std::max(max_err, std::abs(radius - expected_radius) / expected_radius);
    }
    expect(max_err < 0.02, "bicycle steady steer tracks radius L/tan(delta) within 2%");
}

void test_bicycle_steer_rate_limit() {
    agbot::config::ParamTable params;
    params["max_steer_rate_radps"] = 0.5;
    params["max_steer_rad"] = 0.6;
    KinematicBicycleModel model(params);
    EntityState state = state_with_speed(2.0);
    state = model.step(state, {0.0, 0.6}, 0.1);
    expect(std::abs(model.current_steer_rad() - 0.05) < 1e-9,
           "steer rate limited to max_steer_rate * dt");
    for (int i = 0; i < 200; ++i) {
        state = model.step(state, {0.0, 5.0}, 0.02);
    }
    expect(std::abs(model.current_steer_rad() - 0.6) < 1e-9,
           "steer saturates at max_steer_rad even for larger commands");
}

void test_bicycle_speed_clamp() {
    agbot::config::ParamTable params;
    params["max_speed_mps"] = 8.0;
    params["max_accel_mps2"] = 4.0;
    params["max_reverse_speed_mps"] = 2.0;
    KinematicBicycleModel model(params);

    EntityState state;
    for (int i = 0; i < 500; ++i) {
        state = model.step(state, {1.0, 0.0}, 0.02);
    }
    expect(std::abs(state.velocity.horizontal_length() - 8.0) < 1e-9,
           "full throttle clamps at max_speed_mps");

    KinematicBicycleModel reverse_model(params);
    EntityState reverse_state;
    for (int i = 0; i < 500; ++i) {
        reverse_state = reverse_model.step(reverse_state, {-1.0, 0.0}, 0.02);
    }
    const double signed_speed = reverse_state.velocity.x * std::cos(reverse_state.yaw_rad)
        + reverse_state.velocity.z * std::sin(reverse_state.yaw_rad);
    expect(std::abs(signed_speed + 2.0) < 1e-9, "reverse clamps at max_reverse_speed_mps");
    expect(reverse_state.position.x < 0.0, "reverse throttle moves the car backwards");
}

void test_bicycle_substep_determinism() {
    KinematicBicycleModel coarse;
    KinematicBicycleModel fine;
    EntityState coarse_state = state_with_speed(3.0);
    EntityState fine_state = coarse_state;
    const Actuation input{0.4, 0.25};

    coarse_state = coarse.step(coarse_state, input, 1.0);
    for (int i = 0; i < 50; ++i) {
        fine_state = fine.step(fine_state, input, 0.02);
    }
    expect(std::abs(coarse_state.position.x - fine_state.position.x) < 1e-9
               && std::abs(coarse_state.position.z - fine_state.position.z) < 1e-9
               && std::abs(coarse_state.yaw_rad - fine_state.yaw_rad) < 1e-9,
           "step(1.0) matches 50 x step(0.02) within 1e-9");
    expect(std::abs(coarse.current_steer_rad() - fine.current_steer_rad()) < 1e-9,
           "substepped internal steer state matches");
}

void test_multirotor_velocity_control() {
    agbot::config::ParamTable params;
    params["max_speed_mps"] = 6.0;
    params["max_accel_mps2"] = 3.0;
    params["hold_altitude_m"] = 10.0;
    MultirotorModel model(params);

    EntityState state;
    state.position = {0.0, 10.0, 0.0};
    for (int i = 0; i < 400; ++i) {
        state = model.step(state, {0.5, 0.0}, 0.02);
    }
    expect(std::abs(state.velocity.x - 3.0) < 1e-6,
           "multirotor throttle 0.5 converges to half max forward speed");
    expect(std::abs(state.position.y - 10.0) < 0.05, "multirotor holds altitude");

    model.set_velocity_setpoint({0.0, 0.0, 2.0});
    for (int i = 0; i < 400; ++i) {
        state = model.step(state, {0.0, 0.0}, 0.02);
    }
    expect(std::abs(state.velocity.z - 2.0) < 1e-6 && std::abs(state.velocity.x) < 1e-6,
           "multirotor follows direct velocity setpoint");
    expect(model.kind() == VehicleKind::Multirotor, "multirotor reports its kind");
}

void test_registry_creation() {
    agbot::config::ParamTable params;
    params["max_speed_mps"] = 4.5;
    params["wheelbase_m"] = 1.2;
    const auto& registry = agbot::vehicles::default_vehicle_registry();
    expect(registry.contains("kinematic_bicycle") && registry.contains("multirotor"),
           "registry lists built-in vehicle models");

    auto car = agbot::vehicles::create_vehicle_model("kinematic_bicycle", params);
    expect(car != nullptr, "registry creates kinematic bicycle");
    if (car != nullptr) {
        expect(car->kind() == VehicleKind::Car, "created model reports Car kind");
        expect(std::abs(car->limits().max_speed_mps - 4.5) < 1e-12
                   && std::abs(car->limits().wheelbase_m - 1.2) < 1e-12,
               "params configure vehicle limits");
    }
    expect(agbot::vehicles::create_vehicle_model("warp_drive", params) == nullptr,
           "unknown vehicle name returns nullptr");
}

} // namespace

int main() {
    test_bicycle_straight_line();
    test_bicycle_circle_radius();
    test_bicycle_steer_rate_limit();
    test_bicycle_speed_clamp();
    test_bicycle_substep_determinism();
    test_multirotor_velocity_control();
    test_registry_creation();

    if (failures > 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all vehicle tests passed\n";
    return 0;
}
