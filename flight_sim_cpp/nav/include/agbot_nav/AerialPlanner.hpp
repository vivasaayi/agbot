#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_nav/Dubins2D.hpp"
#include "agbot_nav/NavTypes.hpp"
#include "agbot_vehicles/ParamRegistry.hpp"

#include <array>
#include <optional>
#include <string>

namespace agbot::nav {

// Pose on the horizontal XZ plane plus altitude. heading_rad uses the repo
// yaw convention: measured from +X (east) toward +Z (north), so a left turn
// (counterclockwise seen from above with +Z up on the page) increases it.
struct AirPose {
    double x = 0.0;
    double z = 0.0;
    double heading_rad = 0.0;
    double altitude_m = 0.0;
};

struct AerialPlanResult {
    bool ok = false;
    std::string reason;
    std::string word;      // chosen Dubins word, e.g. "LSL"
    Path path;             // sampled every ~sample_spacing_m, Vec3{x, altitude, z}
    double length_m = 0.0; // total 3D-projected 2D arc length incl. helix turns
    int helix_turns = 0;   // extra full circles added to respect the gradient
};

class IAerialPlanner {
public:
    virtual ~IAerialPlanner() = default;
    virtual AerialPlanResult plan(const AirPose& start, const AirPose& goal) const = 0;
    [[nodiscard]] virtual std::string name() const = 0;
};

// DubinsWord, kAllDubinsWords and to_string(DubinsWord) live in
// agbot_nav/Dubins2D.hpp (shared with Hybrid-A*'s analytic expansion).

// Minimum turn radius from the coordinated-turn bank limit:
// R = V^2 / (g * tan(phi_max)).
[[nodiscard]] double bank_limited_turn_radius(
    double airspeed_mps,
    double bank_limit_rad,
    double gravity = 9.80665);

// Dubins-airplane planner: the classic 2D Dubins car shortest path (all six
// CSC/CCC words evaluated, shortest picked deterministically; ties break in
// kAllDubinsWords order) with a linear altitude profile along arc length.
// If the required climb/descent exceeds max_gradient * path_length, whole
// extra turn circles (a helix) are appended at the goal so the gradient
// constraint holds. Params: turn_radius_m, max_gradient, sample_spacing_m.
class DubinsAirplanePlanner final : public IAerialPlanner {
public:
    DubinsAirplanePlanner() = default;
    explicit DubinsAirplanePlanner(const agbot::config::ParamTable& params);

    AerialPlanResult plan(const AirPose& start, const AirPose& goal) const override;
    [[nodiscard]] std::string name() const override { return "dubins_airplane"; }

    // 2D length of one specific word (in meters), if that word is feasible.
    // Exposed so tests can verify shortest-word selection.
    [[nodiscard]] std::optional<double> word_length(
        const AirPose& start,
        const AirPose& goal,
        DubinsWord word) const;

    [[nodiscard]] double turn_radius_m() const { return turn_radius_m_; }
    [[nodiscard]] double max_gradient() const { return max_gradient_; }
    [[nodiscard]] double sample_spacing_m() const { return sample_spacing_m_; }

private:
    double turn_radius_m_ = 200.0;
    double max_gradient_ = 0.10;     // max |dh| per meter of arc length
    double sample_spacing_m_ = 20.0;
};

using AerialPlannerRegistry = agbot::vehicles::ParamRegistry<IAerialPlanner>;

// Registry pre-populated with "dubins_airplane".
[[nodiscard]] const AerialPlannerRegistry& default_aerial_planner_registry();

} // namespace agbot::nav
