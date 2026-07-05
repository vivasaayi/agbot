#include "agbot_nav/AerialPlanner.hpp"

#include "agbot_nav/Dubins2D.hpp"

#include <algorithm>
#include <cmath>
#include <vector>

// Dubins-airplane planner.
//
// The 2D Dubins core (closed-form word solutions, segment rolling, endpoint
// verification) lives in agbot_nav/Dubins2D.hpp and is shared with the
// Hybrid-A* analytic expansion. The airplane extension follows Chitsaz &
// LaValle's "Time-optimal paths for a Dubins airplane": the 2D path carries a
// linear altitude profile, and when the required climb/descent exceeds
// max_gradient * length, whole extra turn circles (a helix) are appended so
// the gradient constraint is met.

namespace agbot::nav {

namespace {

using dubins2d::kTwoPi;
using dubins2d::PlanarPose;
using dubins2d::Segment;
using dubins2d::SegmentType;

} // namespace

const char* to_string(DubinsWord word) {
    switch (word) {
        case DubinsWord::LSL:
            return "LSL";
        case DubinsWord::RSR:
            return "RSR";
        case DubinsWord::LSR:
            return "LSR";
        case DubinsWord::RSL:
            return "RSL";
        case DubinsWord::RLR:
            return "RLR";
        case DubinsWord::LRL:
            return "LRL";
    }
    return "unknown";
}

double bank_limited_turn_radius(double airspeed_mps, double bank_limit_rad, double gravity) {
    return airspeed_mps * airspeed_mps / (gravity * std::tan(bank_limit_rad));
}

DubinsAirplanePlanner::DubinsAirplanePlanner(const agbot::config::ParamTable& params) {
    using agbot::config::double_or;
    turn_radius_m_ = double_or(params, "turn_radius_m", turn_radius_m_);
    max_gradient_ = double_or(params, "max_gradient", max_gradient_);
    sample_spacing_m_ = double_or(params, "sample_spacing_m", sample_spacing_m_);
}

std::optional<double> DubinsAirplanePlanner::word_length(
    const AirPose& start,
    const AirPose& goal,
    DubinsWord word) const {
    const PlanarPose start_pose{start.x, start.z, start.heading_rad};
    const PlanarPose goal_pose{goal.x, goal.z, goal.heading_rad};
    const std::optional<std::array<Segment, 3>> segments =
        dubins2d::solve_word_verified(start_pose, goal_pose, turn_radius_m_, word);
    if (!segments.has_value()) {
        return std::nullopt;
    }
    return (*segments)[0].length_m + (*segments)[1].length_m + (*segments)[2].length_m;
}

AerialPlanResult DubinsAirplanePlanner::plan(const AirPose& start, const AirPose& goal) const {
    AerialPlanResult result;
    if (!(turn_radius_m_ > 0.0) || !(max_gradient_ > 0.0) || !(sample_spacing_m_ > 0.0)) {
        result.reason = "invalid planner parameters";
        return result;
    }

    const PlanarPose start_pose{start.x, start.z, start.heading_rad};
    const PlanarPose goal_pose{goal.x, goal.z, goal.heading_rad};
    const dubins2d::ShortestPath best =
        dubins2d::shortest_path(start_pose, goal_pose, turn_radius_m_);
    if (!best.ok) {
        result.reason = "no feasible dubins word";
        return result;
    }

    std::vector<Segment> segments(best.segments.begin(), best.segments.end());
    double total_length = best.length_m;

    // Altitude feasibility: append full helix circles at the goal when the
    // climb/descent cannot fit within max_gradient over the 2D length.
    const double delta_alt = goal.altitude_m - start.altitude_m;
    const double required_length = std::abs(delta_alt) / max_gradient_;
    int helix_turns = 0;
    if (required_length > total_length) {
        const double circle_length = kTwoPi * turn_radius_m_;
        helix_turns = static_cast<int>(
            std::ceil((required_length - total_length) / circle_length - 1e-12));
        for (int i = 0; i < helix_turns; ++i) {
            segments.push_back({SegmentType::Left, circle_length});
            total_length += circle_length;
        }
    }

    // Sample every sample_spacing_m plus the exact endpoint; altitude is
    // linear in arc length so the slope never exceeds max_gradient.
    const int samples = std::max(
        1, static_cast<int>(std::ceil(total_length / sample_spacing_m_ - 1e-12)));
    result.path.points.reserve(static_cast<std::size_t>(samples) + 1);
    for (int i = 0; i <= samples; ++i) {
        const double s =
            std::min(total_length, static_cast<double>(i) * sample_spacing_m_);
        const PlanarPose pose = dubins2d::pose_at(segments, start_pose, s, turn_radius_m_);
        const double altitude = (total_length > 1e-9)
            ? start.altitude_m + delta_alt * (s / total_length)
            : goal.altitude_m;
        result.path.points.push_back({pose.x, altitude, pose.z});
        if (s >= total_length) {
            break;
        }
    }

    result.ok = true;
    result.word = to_string(best.word);
    result.length_m = total_length;
    result.helix_turns = helix_turns;
    return result;
}

const AerialPlannerRegistry& default_aerial_planner_registry() {
    static const AerialPlannerRegistry registry = [] {
        AerialPlannerRegistry built;
        built.register_factory(
            "dubins_airplane",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<IAerialPlanner> {
                return std::make_unique<DubinsAirplanePlanner>(params);
            });
        return built;
    }();
    return registry;
}

} // namespace agbot::nav
