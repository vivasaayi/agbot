#include "agbot_nav/AerialPlanner.hpp"

#include <algorithm>
#include <cmath>
#include <vector>

// Dubins-airplane planner.
//
// The 2D shortest path between oriented poses with a bounded turn radius is
// the classic Dubins car result (L. E. Dubins, 1957); the closed-form word
// solutions below follow the standard Shkel & Lugo formulation as popularized
// by Andrew Walker's public-domain dubins.c. The airplane extension follows
// Chitsaz & LaValle's "Time-optimal paths for a Dubins airplane": the 2D path
// carries a linear altitude profile, and when the required climb/descent
// exceeds max_gradient * length, whole extra turn circles (a helix) are
// appended so the gradient constraint is met.
//
// Every candidate word is verified by analytically rolling its segments to
// the endpoint; words that fail to reproduce the goal pose are discarded, so
// a formula regression can never produce a wrong path silently.

namespace agbot::nav {

namespace {

constexpr double kPi = 3.14159265358979323846;
constexpr double kTwoPi = 2.0 * kPi;

double mod2pi(double angle) {
    const double wrapped = angle - kTwoPi * std::floor(angle / kTwoPi);
    return (wrapped < 0.0) ? wrapped + kTwoPi : wrapped;
}

enum class SegmentType { Left, Straight, Right };

struct WordSolution {
    bool ok = false;
    double t = 0.0; // first segment, radius units (arc angle or length/R)
    double p = 0.0; // middle segment
    double q = 0.0; // last segment
    [[nodiscard]] double total() const { return t + p + q; }
};

std::array<SegmentType, 3> segment_types(DubinsWord word) {
    switch (word) {
        case DubinsWord::LSL:
            return {SegmentType::Left, SegmentType::Straight, SegmentType::Left};
        case DubinsWord::RSR:
            return {SegmentType::Right, SegmentType::Straight, SegmentType::Right};
        case DubinsWord::LSR:
            return {SegmentType::Left, SegmentType::Straight, SegmentType::Right};
        case DubinsWord::RSL:
            return {SegmentType::Right, SegmentType::Straight, SegmentType::Left};
        case DubinsWord::RLR:
            return {SegmentType::Right, SegmentType::Left, SegmentType::Right};
        case DubinsWord::LRL:
            return {SegmentType::Left, SegmentType::Right, SegmentType::Left};
    }
    return {SegmentType::Left, SegmentType::Straight, SegmentType::Left};
}

// Closed-form word solutions in normalized coordinates: distance d = D / R,
// alpha/beta are start/goal headings relative to the start->goal line.
WordSolution solve_word(DubinsWord word, double alpha, double beta, double d) {
    WordSolution s;
    const double sa = std::sin(alpha);
    const double sb = std::sin(beta);
    const double ca = std::cos(alpha);
    const double cb = std::cos(beta);
    const double cab = std::cos(alpha - beta);

    switch (word) {
        case DubinsWord::LSL: {
            const double tmp0 = d + sa - sb;
            const double p_sq = 2.0 + d * d - 2.0 * cab + 2.0 * d * (sa - sb);
            if (p_sq >= 0.0) {
                const double tmp1 = std::atan2(cb - ca, tmp0);
                s.t = mod2pi(-alpha + tmp1);
                s.p = std::sqrt(p_sq);
                s.q = mod2pi(beta - tmp1);
                s.ok = true;
            }
            break;
        }
        case DubinsWord::RSR: {
            const double tmp0 = d - sa + sb;
            const double p_sq = 2.0 + d * d - 2.0 * cab + 2.0 * d * (sb - sa);
            if (p_sq >= 0.0) {
                const double tmp1 = std::atan2(ca - cb, tmp0);
                s.t = mod2pi(alpha - tmp1);
                s.p = std::sqrt(p_sq);
                s.q = mod2pi(-beta + tmp1);
                s.ok = true;
            }
            break;
        }
        case DubinsWord::LSR: {
            const double p_sq = -2.0 + d * d + 2.0 * cab + 2.0 * d * (sa + sb);
            if (p_sq >= 0.0) {
                const double p = std::sqrt(p_sq);
                const double tmp2 =
                    std::atan2(-ca - cb, d + sa + sb) - std::atan2(-2.0, p);
                s.t = mod2pi(-alpha + tmp2);
                s.p = p;
                s.q = mod2pi(-mod2pi(beta) + tmp2);
                s.ok = true;
            }
            break;
        }
        case DubinsWord::RSL: {
            const double p_sq = d * d - 2.0 + 2.0 * cab - 2.0 * d * (sa + sb);
            if (p_sq >= 0.0) {
                const double p = std::sqrt(p_sq);
                const double tmp2 =
                    std::atan2(ca + cb, d - sa - sb) - std::atan2(2.0, p);
                s.t = mod2pi(alpha - tmp2);
                s.p = p;
                s.q = mod2pi(beta - tmp2);
                s.ok = true;
            }
            break;
        }
        case DubinsWord::RLR: {
            const double tmp = (6.0 - d * d + 2.0 * cab + 2.0 * d * (sa - sb)) / 8.0;
            if (std::abs(tmp) <= 1.0) {
                const double p = mod2pi(kTwoPi - std::acos(tmp));
                s.t = mod2pi(alpha - std::atan2(ca - cb, d - sa + sb) + mod2pi(p / 2.0));
                s.p = p;
                s.q = mod2pi(alpha - beta - s.t + mod2pi(p));
                s.ok = true;
            }
            break;
        }
        case DubinsWord::LRL: {
            const double tmp = (6.0 - d * d + 2.0 * cab + 2.0 * d * (sb - sa)) / 8.0;
            if (std::abs(tmp) <= 1.0) {
                const double p = mod2pi(kTwoPi - std::acos(tmp));
                s.t = mod2pi(-alpha - std::atan2(ca - cb, d + sa - sb) + p / 2.0);
                s.p = p;
                s.q = mod2pi(mod2pi(beta) - alpha - s.t + mod2pi(p));
                s.ok = true;
            }
            break;
        }
    }
    return s;
}

struct PlanarPose {
    double x = 0.0;
    double z = 0.0;
    double heading = 0.0;
};

// Advance a pose along one segment by arc length s_m (left turn increases
// heading, matching the repo yaw convention from +X toward +Z).
PlanarPose advance(const PlanarPose& pose, SegmentType type, double s_m, double radius_m) {
    PlanarPose out = pose;
    switch (type) {
        case SegmentType::Straight:
            out.x += s_m * std::cos(pose.heading);
            out.z += s_m * std::sin(pose.heading);
            break;
        case SegmentType::Left: {
            const double heading = pose.heading + s_m / radius_m;
            out.x += radius_m * (std::sin(heading) - std::sin(pose.heading));
            out.z += radius_m * (std::cos(pose.heading) - std::cos(heading));
            out.heading = heading;
            break;
        }
        case SegmentType::Right: {
            const double heading = pose.heading - s_m / radius_m;
            out.x += radius_m * (std::sin(pose.heading) - std::sin(heading));
            out.z += radius_m * (std::cos(heading) - std::cos(pose.heading));
            out.heading = heading;
            break;
        }
    }
    return out;
}

struct Segment {
    SegmentType type;
    double length_m;
};

PlanarPose pose_at(
    const std::vector<Segment>& segments,
    const PlanarPose& start,
    double s_m,
    double radius_m) {
    PlanarPose pose = start;
    double remaining = s_m;
    for (const Segment& segment : segments) {
        if (remaining <= segment.length_m) {
            return advance(pose, segment.type, remaining, radius_m);
        }
        pose = advance(pose, segment.type, segment.length_m, radius_m);
        remaining -= segment.length_m;
    }
    return pose;
}

double angle_diff(double a, double b) {
    double diff = a - b;
    while (diff > kPi) {
        diff -= kTwoPi;
    }
    while (diff < -kPi) {
        diff += kTwoPi;
    }
    return diff;
}

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
    const double dx = goal.x - start.x;
    const double dz = goal.z - start.z;
    const double distance = std::sqrt(dx * dx + dz * dz);
    const double line = std::atan2(dz, dx);
    const double alpha = mod2pi(start.heading_rad - line);
    const double beta = mod2pi(goal.heading_rad - line);
    const WordSolution solution =
        solve_word(word, alpha, beta, distance / turn_radius_m_);
    if (!solution.ok) {
        return std::nullopt;
    }
    // Verify the closed-form solution actually reaches the goal pose.
    const auto types = segment_types(word);
    PlanarPose pose{start.x, start.z, start.heading_rad};
    const double lengths[3] = {solution.t, solution.p, solution.q};
    for (int i = 0; i < 3; ++i) {
        pose = advance(pose, types[static_cast<std::size_t>(i)],
                       lengths[i] * turn_radius_m_, turn_radius_m_);
    }
    const double pos_err = std::sqrt((pose.x - goal.x) * (pose.x - goal.x)
                                     + (pose.z - goal.z) * (pose.z - goal.z));
    const double heading_err = std::abs(angle_diff(pose.heading, goal.heading_rad));
    if (pos_err > 1e-6 * turn_radius_m_ + 1e-6 || heading_err > 1e-6) {
        return std::nullopt;
    }
    return solution.total() * turn_radius_m_;
}

AerialPlanResult DubinsAirplanePlanner::plan(const AirPose& start, const AirPose& goal) const {
    AerialPlanResult result;
    if (!(turn_radius_m_ > 0.0) || !(max_gradient_ > 0.0) || !(sample_spacing_m_ > 0.0)) {
        result.reason = "invalid planner parameters";
        return result;
    }

    const double dx = goal.x - start.x;
    const double dz = goal.z - start.z;
    const double distance = std::sqrt(dx * dx + dz * dz);
    const double line = std::atan2(dz, dx);
    const double alpha = mod2pi(start.heading_rad - line);
    const double beta = mod2pi(goal.heading_rad - line);
    const double d = distance / turn_radius_m_;

    bool found = false;
    DubinsWord best_word = DubinsWord::LSL;
    WordSolution best;
    for (const DubinsWord word : kAllDubinsWords) {
        const WordSolution candidate = solve_word(word, alpha, beta, d);
        if (!candidate.ok) {
            continue;
        }
        // Endpoint verification (see word_length) keeps bad words out.
        const auto types = segment_types(word);
        PlanarPose pose{start.x, start.z, start.heading_rad};
        const double lengths[3] = {candidate.t, candidate.p, candidate.q};
        for (int i = 0; i < 3; ++i) {
            pose = advance(pose, types[static_cast<std::size_t>(i)],
                           lengths[i] * turn_radius_m_, turn_radius_m_);
        }
        const double pos_err = std::sqrt((pose.x - goal.x) * (pose.x - goal.x)
                                         + (pose.z - goal.z) * (pose.z - goal.z));
        const double heading_err = std::abs(angle_diff(pose.heading, goal.heading_rad));
        if (pos_err > 1e-6 * turn_radius_m_ + 1e-6 || heading_err > 1e-6) {
            continue;
        }
        if (!found || candidate.total() < best.total()) {
            found = true;
            best = candidate;
            best_word = word;
        }
    }
    if (!found) {
        result.reason = "no feasible dubins word";
        return result;
    }

    std::vector<Segment> segments;
    const auto types = segment_types(best_word);
    segments.push_back({types[0], best.t * turn_radius_m_});
    segments.push_back({types[1], best.p * turn_radius_m_});
    segments.push_back({types[2], best.q * turn_radius_m_});
    double total_length = best.total() * turn_radius_m_;

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
    const PlanarPose start_pose{start.x, start.z, start.heading_rad};
    const int samples = std::max(
        1, static_cast<int>(std::ceil(total_length / sample_spacing_m_ - 1e-12)));
    result.path.points.reserve(static_cast<std::size_t>(samples) + 1);
    for (int i = 0; i <= samples; ++i) {
        const double s =
            std::min(total_length, static_cast<double>(i) * sample_spacing_m_);
        const PlanarPose pose = pose_at(segments, start_pose, s, turn_radius_m_);
        const double altitude = (total_length > 1e-9)
            ? start.altitude_m + delta_alt * (s / total_length)
            : goal.altitude_m;
        result.path.points.push_back({pose.x, altitude, pose.z});
        if (s >= total_length) {
            break;
        }
    }

    result.ok = true;
    result.word = to_string(best_word);
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
