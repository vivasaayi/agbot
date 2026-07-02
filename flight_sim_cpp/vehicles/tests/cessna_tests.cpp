#include "agbot_config/Toml.hpp"
#include "agbot_vehicles/FixedWingAutopilot.hpp"
#include "agbot_vehicles/FixedWingModel.hpp"
#include "agbot_vehicles/IVehicleModel.hpp"

#include <algorithm>
#include <cmath>
#include <iostream>
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
constexpr double kGravity = 9.80665;

using agbot::vehicles::Actuation;
using agbot::vehicles::AutopilotCommand;
using agbot::vehicles::EntityState;
using agbot::vehicles::FixedWingAutopilot;
using agbot::vehicles::FixedWingControls;
using agbot::vehicles::FixedWingModel;
using agbot::vehicles::Vec3;
using agbot::vehicles::VehicleKind;

double wrap_pi(double angle) {
    while (angle > kPi) {
        angle -= 2.0 * kPi;
    }
    while (angle < -kPi) {
        angle += 2.0 * kPi;
    }
    return angle;
}

// Run the closed autopilot loop for the given duration; the callback (if
// any) observes every post-step state.
template <typename Callback>
EntityState run_autopilot(
    FixedWingModel& model,
    FixedWingAutopilot& autopilot,
    EntityState state,
    const AutopilotCommand& command,
    double duration_s,
    Callback&& callback) {
    const int steps = static_cast<int>(std::round(duration_s / kDt));
    for (int i = 0; i < steps; ++i) {
        const FixedWingControls controls =
            autopilot.update(state, model.body_rates(), command, kDt);
        model.set_controls(controls);
        state = model.step(state, Actuation{}, kDt);
        callback(state);
    }
    return state;
}

// ---------------------------------------------------------------------------
// Frame mapping
// ---------------------------------------------------------------------------

void test_frame_mapping() {
    FixedWingModel north_model;
    const EntityState north_trim = north_model.set_initial_trim(300.0, 55.0, kPi / 2.0);
    expect(std::abs(north_trim.velocity.z - 55.0) < 0.5
               && std::abs(north_trim.velocity.x) < 1e-6,
           "trim heading pi/2 (north) gives velocity along +Z");
    expect(std::abs(north_trim.yaw_rad - kPi / 2.0) < 1e-9,
           "trim reports the commanded heading");
    expect(std::abs(north_trim.position.y - 300.0) < 1e-9, "trim altitude maps to +Y");

    EntityState state = north_trim;
    state = north_model.step(state, Actuation{}, 1.0);
    expect(state.position.z > 40.0 && std::abs(state.position.x) < 2.0,
           "flying north advances +Z, not +X");

    FixedWingModel east_model;
    EntityState east = east_model.set_initial_trim(300.0, 55.0, 0.0);
    expect(std::abs(east.velocity.x - 55.0) < 0.5 && std::abs(east.velocity.z) < 1e-6,
           "trim heading 0 (east) gives velocity along +X");
    east = east_model.step(east, Actuation{}, 1.0);
    expect(east.position.x > 40.0, "flying east advances +X");

    // Pull up (negative elevator = nose-up) must climb (+Y).
    FixedWingModel climb_model;
    EntityState climb = climb_model.set_initial_trim(300.0, 55.0, 0.0);
    FixedWingControls controls = climb_model.trim_controls();
    controls.elevator -= 0.2;
    controls.throttle = 1.0;
    climb_model.set_controls(controls);
    for (int i = 0; i < 150; ++i) {
        climb = climb_model.step(climb, Actuation{}, kDt);
    }
    expect(climb.position.y > 300.0 && climb.pitch_rad > 0.02,
           "nose-up elevator pitches up and climbs (+Y)");

    // Positive aileron rolls right (positive roll_rad).
    FixedWingModel roll_model;
    EntityState roll = roll_model.set_initial_trim(300.0, 55.0, 0.0);
    controls = roll_model.trim_controls();
    controls.aileron = 0.3;
    roll_model.set_controls(controls);
    for (int i = 0; i < 50; ++i) {
        roll = roll_model.step(roll, Actuation{}, kDt);
    }
    expect(roll.roll_rad > 0.05, "positive aileron rolls right (positive roll)");
    // A right bank turns from north toward east: repo yaw decreases.
    for (int i = 0; i < 200; ++i) {
        roll = roll_model.step(roll, Actuation{}, kDt);
    }
    expect(roll.yaw_rad < 0.0, "right bank decreases repo yaw (turns east->south)");
}

// ---------------------------------------------------------------------------
// Trim cruise hold
// ---------------------------------------------------------------------------

void test_trim_cruise_hold() {
    FixedWingModel model;
    FixedWingAutopilot autopilot;
    EntityState state = model.set_initial_trim(300.0, 55.0, kPi / 2.0);
    autopilot.reset(model.trim_controls());
    const AutopilotCommand command{kPi / 2.0, 300.0, 55.0};

    state = run_autopilot(model, autopilot, state, command, 60.0, [](const EntityState&) {});

    const double alt_err = std::abs(state.position.y - 300.0);
    const double speed_err = std::abs(state.velocity.length() - 55.0);
    const double heading_err = std::abs(wrap_pi(state.yaw_rad - kPi / 2.0));
    std::cout << "  [trim] alt_err=" << alt_err << " m, speed_err=" << speed_err
              << " m/s, heading_err=" << heading_err * 180.0 / kPi << " deg\n";
    expect(alt_err < 15.0, "trim hold keeps altitude within +/-15 m over 60 s");
    expect(speed_err < 3.0, "trim hold keeps airspeed within +/-3 m/s over 60 s");
    expect(heading_err < 5.0 * kPi / 180.0, "trim hold keeps heading within +/-5 deg");
}

// ---------------------------------------------------------------------------
// Climb performance
// ---------------------------------------------------------------------------

void test_climb() {
    FixedWingModel model;
    agbot::config::ParamTable ap_params;
    ap_params["climb_rate_limit_mps"] = 3.0;
    FixedWingAutopilot autopilot(ap_params);
    EntityState state = model.set_initial_trim(300.0, 40.0, 0.0);
    autopilot.reset(model.trim_controls());
    const AutopilotCommand command{0.0, 500.0, 40.0};

    double t_at_320 = -1.0;
    double t_at_480 = -1.0;
    state = run_autopilot(model, autopilot, state, command, 120.0, [&](const EntityState& s) {
        if (t_at_320 < 0.0 && s.position.y >= 320.0) {
            t_at_320 = s.time_s;
        }
        if (t_at_480 < 0.0 && s.position.y >= 480.0) {
            t_at_480 = s.time_s;
        }
    });

    expect(std::abs(state.position.y - 500.0) < 15.0, "climb reaches +200 m altitude target");
    expect(t_at_320 > 0.0 && t_at_480 > t_at_320, "climb passes through the measured band");
    if (t_at_320 > 0.0 && t_at_480 > t_at_320) {
        const double climb_rate = 160.0 / (t_at_480 - t_at_320);
        std::cout << "  [climb] steady climb rate=" << climb_rate << " m/s\n";
        expect(climb_rate > 2.0 && climb_rate < 4.0,
               "steady climb rate is plausible for a C172 (2-4 m/s)");
    }
}

// ---------------------------------------------------------------------------
// Banked turn radius vs theory
// ---------------------------------------------------------------------------

void test_banked_turn() {
    FixedWingModel model;
    FixedWingAutopilot autopilot;
    EntityState state = model.set_initial_trim(300.0, 55.0, kPi / 2.0);
    autopilot.reset(model.trim_controls());
    const AutopilotCommand command{kPi, 300.0, 55.0}; // +90 deg heading step

    const double bank_limit = 0.5236;
    double prev_yaw = state.yaw_rad;
    double yaw_acc = 0.0;
    double time_acc = 0.0;
    double speed_acc = 0.0;
    double bank_acc = 0.0;
    int n = 0;

    state = run_autopilot(model, autopilot, state, command, 40.0, [&](const EntityState& s) {
        const double dyaw = wrap_pi(s.yaw_rad - prev_yaw);
        prev_yaw = s.yaw_rad;
        // Steady-turn window: bank held near the limit.
        if (std::abs(s.roll_rad) > 0.9 * bank_limit) {
            yaw_acc += dyaw;
            time_acc += kDt;
            speed_acc += s.velocity.length();
            bank_acc += std::abs(s.roll_rad);
            ++n;
        }
    });

    const double final_heading_err = std::abs(wrap_pi(state.yaw_rad - kPi));
    expect(final_heading_err < 5.0 * kPi / 180.0, "heading step +90 deg converges within 5 deg");
    expect(n > 100, "turn holds the bank limit for a measurable window");
    if (n > 100) {
        const double yaw_rate = std::abs(yaw_acc) / time_acc;
        const double v_avg = speed_acc / n;
        const double bank_avg = bank_acc / n;
        const double r_measured = v_avg / yaw_rate;
        const double r_theory = v_avg * v_avg / (kGravity * std::tan(bank_avg));
        std::cout << "  [turn] V=" << v_avg << " m/s, bank=" << bank_avg * 180.0 / kPi
                  << " deg, R_measured=" << r_measured << " m, R_theory=" << r_theory
                  << " m\n";
        expect(std::abs(r_measured - r_theory) / r_theory < 0.20,
               "steady turn radius matches V^2/(g tan phi) within 20%");
    }
}

// ---------------------------------------------------------------------------
// Stall behavior
// ---------------------------------------------------------------------------

void test_stall() {
    FixedWingModel model;
    EntityState state = model.set_initial_trim(1000.0, 55.0, 0.0);
    FixedWingControls controls = model.trim_controls();
    controls.throttle = 0.0;   // idle
    controls.elevator = -0.22; // firm nose-up, held through the stall
    controls.aileron = 0.0;
    controls.rudder = 0.0;
    model.set_controls(controls);

    double min_airspeed = 1e9;
    double max_alpha = 0.0;
    bool cl_cap_seen = false;
    double sink_before_stall = 0.0;
    double max_sink_after_stall = 0.0;
    double min_altitude = 1000.0;
    bool stalled = false;
    const auto& p = model.params();

    for (int i = 0; i < 3000; ++i) { // 60 s
        state = model.step(state, Actuation{}, kDt);
        const auto& dbg = model.aero_debug();
        min_airspeed = std::min(min_airspeed, dbg.airspeed_mps);
        max_alpha = std::max(max_alpha, dbg.alpha_rad);
        if (dbg.alpha_rad > p.alpha_stall_rad) {
            const double cl_linear = p.cl0 + p.cl_alpha * dbg.alpha_rad;
            if (dbg.cl < cl_linear - 0.05) {
                cl_cap_seen = true;
            }
            stalled = true;
        }
        if (!stalled) {
            sink_before_stall = std::max(sink_before_stall, -state.velocity.y);
        } else {
            max_sink_after_stall = std::max(max_sink_after_stall, -state.velocity.y);
        }
        min_altitude = std::min(min_altitude, state.position.y);
        if (state.position.y <= 0.0) {
            break;
        }
    }

    std::cout << "  [stall] min_airspeed=" << min_airspeed
              << " m/s, max_alpha=" << max_alpha * 180.0 / kPi
              << " deg, max_sink_after=" << max_sink_after_stall << " m/s\n";
    expect(min_airspeed < 28.0, "airspeed decays below ~28 m/s at idle with nose held up");
    expect(stalled && max_alpha > p.alpha_stall_rad, "angle of attack exceeds stall onset");
    expect(cl_cap_seen, "CL cap engages beyond stall (blended below the linear curve)");
    expect(max_sink_after_stall > 5.0 && max_sink_after_stall > sink_before_stall + 3.0,
           "sink rate increases sharply after stall onset");
    expect(min_altitude < 1000.0 - 100.0, "altitude drops rapidly after the stall");
}

// ---------------------------------------------------------------------------
// Glide ratio
// ---------------------------------------------------------------------------

void test_glide() {
    FixedWingModel model;
    EntityState state = model.set_initial_trim(1000.0, 55.0, 0.0);
    FixedWingControls controls;
    controls.throttle = 0.0;
    controls.elevator = -0.06; // near-neutral trim toward best-glide alpha
    controls.aileron = 0.0;
    controls.rudder = 0.0;
    model.set_controls(controls);

    // Let the phugoid settle for 20 s, then measure over 80 s.
    for (int i = 0; i < 1000; ++i) {
        state = model.step(state, Actuation{}, kDt);
    }
    const Vec3 start_pos = state.position;
    for (int i = 0; i < 4000; ++i) {
        state = model.step(state, Actuation{}, kDt);
    }
    const double horizontal =
        (state.position - start_pos).horizontal_length();
    const double altitude_lost = start_pos.y - state.position.y;
    const double glide_ratio = horizontal / altitude_lost;
    std::cout << "  [glide] horizontal=" << horizontal << " m, lost=" << altitude_lost
              << " m, ratio=" << glide_ratio << "\n";
    expect(altitude_lost > 50.0, "glide descends steadily with the throttle at idle");
    expect(glide_ratio > 7.0 && glide_ratio < 12.0,
           "glide ratio is C172-plausible (7-12, book ~9)");
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

void test_determinism() {
    // Two identical closed-loop runs must be bit-identical.
    FixedWingModel model_a;
    FixedWingModel model_b;
    FixedWingAutopilot ap_a;
    FixedWingAutopilot ap_b;
    EntityState sa = model_a.set_initial_trim(300.0, 55.0, 1.0);
    EntityState sb = model_b.set_initial_trim(300.0, 55.0, 1.0);
    ap_a.reset(model_a.trim_controls());
    ap_b.reset(model_b.trim_controls());
    const AutopilotCommand command{2.0, 350.0, 50.0};
    sa = run_autopilot(model_a, ap_a, sa, command, 20.0, [](const EntityState&) {});
    sb = run_autopilot(model_b, ap_b, sb, command, 20.0, [](const EntityState&) {});
    expect(sa.position.x == sb.position.x && sa.position.y == sb.position.y
               && sa.position.z == sb.position.z && sa.velocity.x == sb.velocity.x
               && sa.velocity.y == sb.velocity.y && sa.velocity.z == sb.velocity.z
               && sa.yaw_rad == sb.yaw_rad && sa.pitch_rad == sb.pitch_rad
               && sa.roll_rad == sb.roll_rad && sa.time_s == sb.time_s,
           "two identical closed-loop runs are bit-identical");

    // step(1.0) must equal 50 x step(0.02) exactly (fixed controls).
    FixedWingModel coarse;
    FixedWingModel fine;
    EntityState cs = coarse.set_initial_trim(300.0, 55.0, 0.3);
    EntityState fs = fine.set_initial_trim(300.0, 55.0, 0.3);
    FixedWingControls controls = coarse.trim_controls();
    controls.aileron = 0.1;
    coarse.set_controls(controls);
    fine.set_controls(controls);
    cs = coarse.step(cs, Actuation{}, 1.0);
    for (int i = 0; i < 50; ++i) {
        fs = fine.step(fs, Actuation{}, kDt);
    }
    expect(cs.position.x == fs.position.x && cs.position.y == fs.position.y
               && cs.position.z == fs.position.z && cs.yaw_rad == fs.yaw_rad
               && cs.pitch_rad == fs.pitch_rad && cs.roll_rad == fs.roll_rad
               && cs.time_s == fs.time_s,
           "step(1.0) is bit-identical to 50 x step(0.02)");
}

// ---------------------------------------------------------------------------
// Registry + config loading
// ---------------------------------------------------------------------------

void test_registry_and_config() {
    const auto& registry = agbot::vehicles::default_vehicle_registry();
    expect(registry.contains("fixed_wing"), "registry lists fixed_wing");
    agbot::config::ParamTable params;
    params["mass_kg"] = 900.0;
    auto model = agbot::vehicles::create_vehicle_model("fixed_wing", params);
    expect(model != nullptr && model->kind() == VehicleKind::FixedWing,
           "registry creates a FixedWing model");

    const auto parsed = agbot::config::parse_toml_file(
        std::string(AGBOT_VEHICLES_CONFIG_DIR) + "/cessna172.toml");
    expect(parsed.ok, "cessna172.toml parses");
    if (!parsed.ok) {
        std::cout << "  toml error: " << parsed.error << "\n";
        return;
    }
    const auto* wing = agbot::config::find_table(parsed.root, "fixed_wing");
    const auto* autopilot_params = agbot::config::find_table(parsed.root, "autopilot");
    expect(wing != nullptr && autopilot_params != nullptr,
           "config exposes fixed_wing and autopilot tables");
    if (wing == nullptr || autopilot_params == nullptr) {
        return;
    }

    FixedWingModel cessna(*wing);
    expect(std::abs(cessna.params().mass_kg - 1043.0) < 1e-9
               && std::abs(cessna.params().t_max_n - 2200.0) < 1e-9,
           "config parameters reach the model");

    FixedWingAutopilot autopilot(*autopilot_params);
    EntityState state = cessna.set_initial_trim(300.0, 55.0, 0.0);
    autopilot.reset(cessna.trim_controls());
    const AutopilotCommand command{0.0, 300.0, 55.0};
    state = run_autopilot(cessna, autopilot, state, command, 20.0, [](const EntityState&) {});
    expect(std::abs(state.position.y - 300.0) < 15.0,
           "config-built model + autopilot hold trim");
}

} // namespace

int main() {
    test_frame_mapping();
    test_trim_cruise_hold();
    test_climb();
    test_banked_turn();
    test_stall();
    test_glide();
    test_determinism();
    test_registry_and_config();

    if (failures > 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all cessna tests passed\n";
    return 0;
}
