#pragma once

#include <array>
#include <cmath>
#include <cstddef>
#include <optional>
#include <vector>

// Shared 2D Dubins-car core used by both the Dubins-airplane aerial planner
// and the Hybrid-A* analytic expansion.
//
// The shortest path between oriented planar poses with a bounded turn radius
// is the classic Dubins result (L. E. Dubins, 1957); the closed-form word
// solutions follow the standard Shkel & Lugo formulation as popularized by
// Andrew Walker's public-domain dubins.c. Every candidate word is verified by
// analytically rolling its segments to the endpoint, so a formula regression
// can never produce a wrong path silently.
//
// Convention: heading is measured on the XZ plane from +X toward +Z, so a
// left turn increases heading (matches the repo yaw convention).

namespace agbot::nav {

enum class DubinsWord {
    LSL,
    RSR,
    LSR,
    RSL,
    RLR,
    LRL,
};

inline constexpr std::array<DubinsWord, 6> kAllDubinsWords = {
    DubinsWord::LSL, DubinsWord::RSR, DubinsWord::LSR,
    DubinsWord::RSL, DubinsWord::RLR, DubinsWord::LRL,
};

[[nodiscard]] const char* to_string(DubinsWord word);

namespace dubins2d {

inline constexpr double kPi = 3.14159265358979323846;
inline constexpr double kTwoPi = 2.0 * kPi;

[[nodiscard]] inline double mod2pi(double angle) {
    const double wrapped = angle - kTwoPi * std::floor(angle / kTwoPi);
    return (wrapped < 0.0) ? wrapped + kTwoPi : wrapped;
}

[[nodiscard]] inline double angle_diff(double a, double b) {
    double diff = a - b;
    while (diff > kPi) {
        diff -= kTwoPi;
    }
    while (diff < -kPi) {
        diff += kTwoPi;
    }
    return diff;
}

enum class SegmentType { Left, Straight, Right };

struct WordSolution {
    bool ok = false;
    double t = 0.0; // first segment, radius units (arc angle or length/R)
    double p = 0.0; // middle segment
    double q = 0.0; // last segment
    [[nodiscard]] double total() const { return t + p + q; }
};

[[nodiscard]] inline std::array<SegmentType, 3> segment_types(DubinsWord word) {
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
[[nodiscard]] inline WordSolution solve_word(DubinsWord word, double alpha, double beta, double d) {
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
[[nodiscard]] inline PlanarPose advance(
    const PlanarPose& pose,
    SegmentType type,
    double s_m,
    double radius_m) {
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

// Pose at arc length s_m along a segment chain that starts at `start`.
[[nodiscard]] inline PlanarPose pose_at(
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

// Solve one word between world poses and verify the endpoint analytically.
// Returns the three segments in meters, or nullopt if the word is infeasible
// or fails endpoint verification.
[[nodiscard]] inline std::optional<std::array<Segment, 3>> solve_word_verified(
    const PlanarPose& start,
    const PlanarPose& goal,
    double radius_m,
    DubinsWord word) {
    const double dx = goal.x - start.x;
    const double dz = goal.z - start.z;
    const double distance = std::sqrt(dx * dx + dz * dz);
    const double line = std::atan2(dz, dx);
    const double alpha = mod2pi(start.heading - line);
    const double beta = mod2pi(goal.heading - line);
    const WordSolution solution = solve_word(word, alpha, beta, distance / radius_m);
    if (!solution.ok) {
        return std::nullopt;
    }
    const std::array<SegmentType, 3> types = segment_types(word);
    const double lengths[3] = {solution.t, solution.p, solution.q};
    PlanarPose pose = start;
    for (int i = 0; i < 3; ++i) {
        pose = advance(pose, types[static_cast<std::size_t>(i)],
                       lengths[i] * radius_m, radius_m);
    }
    const double pos_err = std::sqrt((pose.x - goal.x) * (pose.x - goal.x)
                                     + (pose.z - goal.z) * (pose.z - goal.z));
    const double heading_err = std::abs(angle_diff(pose.heading, goal.heading));
    if (pos_err > 1e-6 * radius_m + 1e-6 || heading_err > 1e-6) {
        return std::nullopt;
    }
    return std::array<Segment, 3>{
        Segment{types[0], solution.t * radius_m},
        Segment{types[1], solution.p * radius_m},
        Segment{types[2], solution.q * radius_m},
    };
}

struct ShortestPath {
    bool ok = false;
    DubinsWord word = DubinsWord::LSL;
    std::array<Segment, 3> segments{};
    double length_m = 0.0;
};

// Shortest verified Dubins path over all six words; ties break in
// kAllDubinsWords order (deterministic).
[[nodiscard]] inline ShortestPath shortest_path(
    const PlanarPose& start,
    const PlanarPose& goal,
    double radius_m) {
    ShortestPath best;
    for (const DubinsWord word : kAllDubinsWords) {
        const std::optional<std::array<Segment, 3>> segments =
            solve_word_verified(start, goal, radius_m, word);
        if (!segments.has_value()) {
            continue;
        }
        const double length =
            (*segments)[0].length_m + (*segments)[1].length_m + (*segments)[2].length_m;
        if (!best.ok || length < best.length_m) {
            best.ok = true;
            best.word = word;
            best.segments = *segments;
            best.length_m = length;
        }
    }
    return best;
}

} // namespace dubins2d
} // namespace agbot::nav
