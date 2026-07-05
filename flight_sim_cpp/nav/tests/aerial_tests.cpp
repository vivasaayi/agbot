#include "agbot_nav/AerialPlanner.hpp"
#include "agbot_vehicles/FixedWingAutopilot.hpp"
#include "agbot_vehicles/FixedWingModel.hpp"

#include <algorithm>
#include <cmath>
#include <iostream>
#include <limits>
#include <string>
#include <vector>

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

constexpr double kPi = 3.14159265358979323846;
constexpr double kDt = 0.02;

using agbot::nav::AerialPlanResult;
using agbot::nav::AirPose;
using agbot::nav::DubinsAirplanePlanner;
using agbot::nav::DubinsWord;
using agbot::nav::Path;
using agbot::nav::Vec3;
using agbot::vehicles::Actuation;
using agbot::vehicles::AutopilotCommand;
using agbot::vehicles::EntityState;
using agbot::vehicles::FixedWingAutopilot;
using agbot::vehicles::FixedWingControls;
using agbot::vehicles::FixedWingModel;

DubinsAirplanePlanner make_planner(double radius, double gradient, double spacing) {
    agbot::config::ParamTable params;
    params["turn_radius_m"] = radius;
    params["max_gradient"] = gradient;
    params["sample_spacing_m"] = spacing;
    return DubinsAirplanePlanner(params);
}

double max_sample_gradient(const Path& path) {
    double worst = 0.0;
    for (std::size_t i = 1; i < path.points.size(); ++i) {
        const Vec3 delta = path.points[i] - path.points[i - 1];
        const double run = delta.horizontal_length();
        if (run > 1e-6) {
            worst = std::max(worst, std::abs(delta.y) / run);
        }
    }
    return worst;
}

// ---------------------------------------------------------------------------
// Dubins geometry
// ---------------------------------------------------------------------------

void test_dubins_straight_line() {
    const DubinsAirplanePlanner planner = make_planner(200.0, 0.1, 20.0);
    const AirPose start{0.0, 0.0, 0.0, 300.0};
    const AirPose goal{1000.0, 0.0, 0.0, 300.0};
    const AerialPlanResult result = planner.plan(start, goal);
    expect(result.ok, "straight-line case plans");
    expect(std::abs(result.length_m - 1000.0) < 1e-6,
           "straight-line length equals the euclidean distance");
    bool on_axis = true;
    for (const Vec3& point : result.path.points) {
        on_axis = on_axis && std::abs(point.z) < 1e-6 && std::abs(point.y - 300.0) < 1e-9;
    }
    expect(on_axis, "straight-line samples stay on the axis at constant altitude");
    const Vec3& last = result.path.points.back();
    expect(std::abs(last.x - 1000.0) < 1e-6, "straight-line path ends exactly at the goal");
}

void test_dubins_u_turn() {
    const double radius = 200.0;
    const DubinsAirplanePlanner planner = make_planner(radius, 0.1, 20.0);
    // Fly +X for 500 m, then a half circle to come back parallel: total
    // length must be separation + pi * R.
    const AirPose start{0.0, 0.0, 0.0, 300.0};
    const AirPose goal{500.0, 2.0 * radius, kPi, 300.0};
    const AerialPlanResult result = planner.plan(start, goal);
    expect(result.ok, "u-turn case plans");
    const double expected = 500.0 + kPi * radius;
    std::cout << "  [dubins] u-turn length=" << result.length_m << " expected=" << expected
              << " word=" << result.word << "\n";
    expect(std::abs(result.length_m - expected) < 1e-6,
           "u-turn length equals separation + pi*R");
}

void test_dubins_shortest_word_selection() {
    const DubinsAirplanePlanner planner = make_planner(150.0, 0.1, 10.0);
    const AirPose start{0.0, 0.0, 0.0, 300.0};
    const AirPose goal{300.0, 120.0, 2.4, 300.0};
    const AerialPlanResult result = planner.plan(start, goal);
    expect(result.ok, "known mixed case plans");

    double best = std::numeric_limits<double>::infinity();
    std::string best_word;
    int feasible = 0;
    for (const DubinsWord word : agbot::nav::kAllDubinsWords) {
        const auto length = planner.word_length(start, goal, word);
        if (length.has_value()) {
            ++feasible;
            std::cout << "  [dubins] word " << agbot::nav::to_string(word) << " length="
                      << *length << "\n";
            if (*length < best) {
                best = *length;
                best_word = agbot::nav::to_string(word);
            }
        }
    }
    expect(feasible >= 4, "several words are feasible for the mixed case");
    expect(result.word == best_word, "planner picks the shortest word");
    expect(std::abs(result.length_m - best) < 1e-6,
           "planned length equals the best word length");

    // The endpoint must reproduce the goal pose.
    const Vec3& last = result.path.points.back();
    expect(std::abs(last.x - goal.x) < 1e-3 && std::abs(last.z - goal.z) < 1e-3,
           "shortest path ends at the goal position");
}

void test_dubins_altitude_gradient() {
    const DubinsAirplanePlanner planner = make_planner(200.0, 0.08, 20.0);
    const AirPose start{0.0, 0.0, 0.0, 300.0};
    const AirPose goal{2000.0, 0.0, 0.0, 340.0};
    const AerialPlanResult result = planner.plan(start, goal);
    expect(result.ok, "gentle climb case plans");
    expect(result.helix_turns == 0, "gentle climb needs no helix");
    expect(max_sample_gradient(result.path) <= 0.08 + 1e-9,
           "sampled slope never exceeds the gradient limit");
    expect(std::abs(result.path.points.back().y - 340.0) < 1e-9,
           "path reaches the goal altitude exactly");
}

void test_dubins_helix_extension() {
    const double radius = 200.0;
    const DubinsAirplanePlanner planner = make_planner(radius, 0.08, 20.0);
    const AirPose start{0.0, 0.0, 0.0, 300.0};
    const AirPose goal{400.0, 0.0, 0.0, 600.0}; // +300 m over a 400 m 2D path
    const AerialPlanResult result = planner.plan(start, goal);
    expect(result.ok, "steep climb case plans");
    // Required length 300/0.08 = 3750 m; base 400 m; circle 2*pi*200 = 1256.6 m
    // -> 3 helix turns.
    std::cout << "  [dubins] helix turns=" << result.helix_turns
              << " total length=" << result.length_m << "\n";
    expect(result.helix_turns == 3, "helix adds the minimal number of full turns");
    expect(max_sample_gradient(result.path) <= 0.08 + 1e-9,
           "helix path respects the gradient limit");
    const Vec3& last = result.path.points.back();
    expect(std::abs(last.x - 400.0) < 1e-3 && std::abs(last.z) < 1e-3
               && std::abs(last.y - 600.0) < 1e-9,
           "helix path still ends at the goal pose and altitude");
}

void test_dubins_determinism_and_registry() {
    const DubinsAirplanePlanner planner = make_planner(180.0, 0.1, 15.0);
    const AirPose start{12.5, -40.0, 0.7, 250.0};
    const AirPose goal{800.0, 400.0, -1.9, 320.0};
    const AerialPlanResult a = planner.plan(start, goal);
    const AerialPlanResult b = planner.plan(start, goal);
    bool identical = a.ok && b.ok && a.length_m == b.length_m && a.word == b.word
        && a.path.points.size() == b.path.points.size();
    if (identical) {
        for (std::size_t i = 0; i < a.path.points.size(); ++i) {
            identical = identical && a.path.points[i].x == b.path.points[i].x
                && a.path.points[i].y == b.path.points[i].y
                && a.path.points[i].z == b.path.points[i].z;
        }
    }
    expect(identical, "two identical plans are bit-identical");

    const auto& registry = agbot::nav::default_aerial_planner_registry();
    expect(registry.contains("dubins_airplane"), "registry lists dubins_airplane");
    agbot::config::ParamTable params;
    params["turn_radius_m"] = 250.0;
    auto created = registry.create("dubins_airplane", params);
    expect(created != nullptr && created->name() == "dubins_airplane",
           "registry creates the aerial planner");

    const double r = agbot::nav::bank_limited_turn_radius(40.0, 0.5236);
    expect(std::abs(r - 40.0 * 40.0 / (9.80665 * std::tan(0.5236))) < 1e-9,
           "bank_limited_turn_radius matches V^2/(g tan phi)");
}

// ---------------------------------------------------------------------------
// Circuit integration: Cessna + autopilot follow a Dubins-planned square
// ---------------------------------------------------------------------------

double wrap_pi(double angle) {
    while (angle > kPi) {
        angle -= 2.0 * kPi;
    }
    while (angle < -kPi) {
        angle += 2.0 * kPi;
    }
    return angle;
}

double point_to_segment_distance(const Vec3& p, const Vec3& a, const Vec3& b) {
    const double abx = b.x - a.x;
    const double abz = b.z - a.z;
    const double len_sq = abx * abx + abz * abz;
    double t = 0.0;
    if (len_sq > 1e-12) {
        t = ((p.x - a.x) * abx + (p.z - a.z) * abz) / len_sq;
        t = std::max(0.0, std::min(1.0, t));
    }
    const double dx = p.x - (a.x + t * abx);
    const double dz = p.z - (a.z + t * abz);
    return std::sqrt(dx * dx + dz * dz);
}

void test_circuit_integration() {
    const double side = 2000.0;
    const double altitude = 300.0;
    const double airspeed = 40.0;
    const DubinsAirplanePlanner planner = make_planner(300.0, 0.08, 20.0);

    // 4 km-scale square circuit: corner poses headed along the departing leg.
    const AirPose corners[5] = {
        {0.0, 0.0, 0.0, altitude},
        {side, 0.0, kPi / 2.0, altitude},
        {side, side, kPi, altitude},
        {0.0, side, -kPi / 2.0, altitude},
        {0.0, 0.0, 0.0, altitude},
    };

    std::vector<Vec3> route;
    for (int leg = 0; leg < 4; ++leg) {
        const AerialPlanResult plan = planner.plan(corners[leg], corners[leg + 1]);
        expect(plan.ok, "circuit leg " + std::to_string(leg) + " plans");
        if (!plan.ok) {
            return;
        }
        const std::size_t first = route.empty() ? 0 : 1; // drop duplicated joint
        for (std::size_t i = first; i < plan.path.points.size(); ++i) {
            route.push_back(plan.path.points[i]);
        }
    }
    expect(route.size() > 300, "circuit route has a dense sample set");

    FixedWingModel model;
    FixedWingAutopilot autopilot;
    EntityState state = model.set_initial_trim(altitude, airspeed, 0.0, 0.0, 0.0);
    autopilot.reset(model.trim_controls());

    const std::size_t lookahead_points = 10; // ~200 m at 20 m spacing
    std::size_t nearest = 0;
    double max_cross_track = 0.0;
    double max_alt_err = 0.0;
    bool completed = false;
    const double max_time_s = 500.0;

    for (double t = 0.0; t < max_time_s; t += kDt) {
        // Monotonic nearest-point tracking with a bounded forward window.
        double best = std::numeric_limits<double>::infinity();
        std::size_t best_idx = nearest;
        const std::size_t window_end = std::min(route.size(), nearest + 40);
        for (std::size_t i = nearest; i < window_end; ++i) {
            const double dx = route[i].x - state.position.x;
            const double dz = route[i].z - state.position.z;
            const double dist = dx * dx + dz * dz;
            if (dist < best) {
                best = dist;
                best_idx = i;
            }
        }
        nearest = best_idx;
        if (nearest + 2 >= route.size()) {
            completed = true;
            break;
        }

        // Cross-track: distance to the segments around the nearest sample.
        double cross = std::numeric_limits<double>::infinity();
        const std::size_t seg_lo = (nearest >= 2) ? nearest - 2 : 0;
        const std::size_t seg_hi = std::min(route.size() - 1, nearest + 2);
        for (std::size_t i = seg_lo; i < seg_hi; ++i) {
            cross = std::min(cross,
                             point_to_segment_distance(state.position, route[i], route[i + 1]));
        }
        if (t > 15.0) {
            max_cross_track = std::max(max_cross_track, cross);
            max_alt_err = std::max(max_alt_err, std::abs(state.position.y - altitude));
        }

        const std::size_t target =
            std::min(route.size() - 1, nearest + lookahead_points);
        const double heading_cmd = std::atan2(route[target].z - state.position.z,
                                              route[target].x - state.position.x);
        const AutopilotCommand command{wrap_pi(heading_cmd), altitude, airspeed};
        const FixedWingControls controls =
            autopilot.update(state, model.body_rates(), command, kDt);
        model.set_controls(controls);
        state = model.step(state, Actuation{}, kDt);
    }

    std::cout << "  [circuit] completed=" << (completed ? "yes" : "no")
              << " max_cross_track=" << max_cross_track << " m, max_alt_err="
              << max_alt_err << " m, t_end=" << state.time_s << " s\n";
    expect(completed, "cessna completes the 4-leg dubins circuit");
    expect(max_cross_track < 60.0, "cross-track error stays below 60 m");
    expect(max_alt_err < 25.0, "altitude held within +/-25 m around the circuit");
}

} // namespace

int main() {
    test_dubins_straight_line();
    test_dubins_u_turn();
    test_dubins_shortest_word_selection();
    test_dubins_altitude_gradient();
    test_dubins_helix_extension();
    test_dubins_determinism_and_registry();
    test_circuit_integration();

    if (failures > 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all aerial tests passed\n";
    return 0;
}
