#include "agbot_nav/Tracking.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <map>
#include <memory>

namespace agbot::nav {

namespace {

const char* class_name_of(std::uint32_t class_id) {
    switch (class_id) {
    case kClassPedestrian:
        return "pedestrian";
    case kClassVehicle:
        return "vehicle";
    default:
        return "obstacle";
    }
}

struct ClusterAccumulator {
    double sum_x = 0.0;
    double sum_z = 0.0;
    std::size_t count = 0;
    std::uint32_t class_id = kClassObstacle;
    std::uint32_t object_id = 0;
    std::vector<std::size_t> members;

    [[nodiscard]] double centroid_x() const { return sum_x / static_cast<double>(count); }
    [[nodiscard]] double centroid_z() const { return sum_z / static_cast<double>(count); }
};

Detection finish_cluster(const ClusterAccumulator& cluster, const PointCloud& cloud) {
    Detection detection;
    detection.position = {cluster.centroid_x(), 0.0, cluster.centroid_z()};
    detection.class_id = cluster.class_id;
    detection.object_id = cluster.object_id;
    detection.point_count = cluster.count;
    double max_extent = 0.0;
    for (const std::size_t index : cluster.members) {
        const double dx = cloud.points[index].x - detection.position.x;
        const double dz = cloud.points[index].z - detection.position.z;
        max_extent = std::max(max_extent, std::sqrt(dx * dx + dz * dz));
    }
    detection.radius_m = std::max(0.15, max_extent);
    return detection;
}

} // namespace

std::vector<Detection> cluster_dynamic_detections(
    const PointCloud& cloud,
    bool use_object_ids,
    double cluster_distance_m) {
    std::vector<Detection> detections;
    if (cloud.classes.size() != cloud.points.size()) {
        return detections;
    }
    const bool ids_available = cloud.object_ids.size() == cloud.points.size();

    if (use_object_ids && ids_available) {
        // Ground-truth association path: group by sensor object id (ordered
        // map keeps the output deterministic and sorted by id).
        std::map<std::uint32_t, ClusterAccumulator> clusters;
        for (std::size_t i = 0; i < cloud.points.size(); ++i) {
            if (!is_dynamic_class(cloud.classes[i])) {
                continue;
            }
            ClusterAccumulator& cluster = clusters[cloud.object_ids[i]];
            cluster.sum_x += cloud.points[i].x;
            cluster.sum_z += cloud.points[i].z;
            ++cluster.count;
            cluster.class_id = cloud.classes[i];
            cluster.object_id = cloud.object_ids[i];
            cluster.members.push_back(i);
        }
        detections.reserve(clusters.size());
        for (const auto& [object_id, cluster] : clusters) {
            (void)object_id;
            detections.push_back(finish_cluster(cluster, cloud));
        }
        return detections;
    }

    // Fallback: greedy distance clustering in cloud order. A point joins the
    // first cluster whose running centroid lies within cluster_distance_m.
    const double join_distance = std::max(1e-3, cluster_distance_m);
    std::vector<ClusterAccumulator> clusters;
    for (std::size_t i = 0; i < cloud.points.size(); ++i) {
        if (!is_dynamic_class(cloud.classes[i])) {
            continue;
        }
        const double px = cloud.points[i].x;
        const double pz = cloud.points[i].z;
        ClusterAccumulator* home = nullptr;
        for (ClusterAccumulator& cluster : clusters) {
            const double dx = px - cluster.centroid_x();
            const double dz = pz - cluster.centroid_z();
            if (std::sqrt(dx * dx + dz * dz) <= join_distance) {
                home = &cluster;
                break;
            }
        }
        if (home == nullptr) {
            clusters.emplace_back();
            home = &clusters.back();
            home->class_id = cloud.classes[i];
        }
        home->sum_x += px;
        home->sum_z += pz;
        ++home->count;
        home->members.push_back(i);
    }
    detections.reserve(clusters.size());
    for (const ClusterAccumulator& cluster : clusters) {
        detections.push_back(finish_cluster(cluster, cloud));
    }
    return detections;
}

GreedyNnTracker::GreedyNnTracker(const agbot::config::ParamTable& params) {
    using agbot::config::double_or;
    using agbot::config::integer_or;
    gate_m_ = double_or(params, "gate_m", gate_m_);
    min_hits_ = static_cast<int>(std::clamp<std::int64_t>(
        integer_or(params, "min_hits", min_hits_), 1, 1000));
    max_missed_ = static_cast<int>(std::clamp<std::int64_t>(
        integer_or(params, "max_missed", max_missed_), 0, 100000));
    q_pos_ = double_or(params, "q_pos", q_pos_);
    q_vel_ = double_or(params, "q_vel", q_vel_);
    r_pos_ = double_or(params, "r_pos", r_pos_);
    init_vel_sigma_ = double_or(params, "init_vel_sigma", init_vel_sigma_);
}

std::vector<TrackedObject> GreedyNnTracker::update(
    const std::vector<Detection>& detections, double time_s) {
    const double dt = has_time_ ? std::max(0.0, time_s - last_time_s_) : 0.0;
    last_time_s_ = time_s;
    has_time_ = true;

    // Predict: constant-velocity propagation of every track.
    // x' = F x, P' = F P F^T + Q with F = [I, dt*I; 0, I].
    if (dt > 0.0) {
        for (Track& track : tracks_) {
            track.x[0] += track.x[2] * dt;
            track.x[1] += track.x[3] * dt;
            std::array<double, 16>& P = track.P;
            // Closed-form F P F^T for the block CV structure (indices:
            // row-major, position rows 0/1 couple to velocity rows 2/3).
            for (int axis = 0; axis < 2; ++axis) {
                const int p = axis;      // position index
                const int v = axis + 2;  // velocity index
                const double Ppp = P[static_cast<std::size_t>(p * 4 + p)];
                const double Ppv = P[static_cast<std::size_t>(p * 4 + v)];
                const double Pvp = P[static_cast<std::size_t>(v * 4 + p)];
                const double Pvv = P[static_cast<std::size_t>(v * 4 + v)];
                P[static_cast<std::size_t>(p * 4 + p)] =
                    Ppp + dt * (Ppv + Pvp) + dt * dt * Pvv + q_pos_ * dt;
                P[static_cast<std::size_t>(p * 4 + v)] = Ppv + dt * Pvv;
                P[static_cast<std::size_t>(v * 4 + p)] = Pvp + dt * Pvv;
                P[static_cast<std::size_t>(v * 4 + v)] = Pvv + q_vel_ * dt;
            }
        }
    }

    // Associate: greedy nearest neighbor over all gated pairs, deterministic
    // ordering (distance, then track id, then detection index).
    struct Pair {
        double distance;
        std::size_t track_index;
        std::size_t detection_index;
    };
    std::vector<Pair> pairs;
    for (std::size_t ti = 0; ti < tracks_.size(); ++ti) {
        for (std::size_t di = 0; di < detections.size(); ++di) {
            const double dx = detections[di].position.x - tracks_[ti].x[0];
            const double dz = detections[di].position.z - tracks_[ti].x[1];
            const double distance = std::sqrt(dx * dx + dz * dz);
            if (distance <= gate_m_) {
                pairs.push_back({distance, ti, di});
            }
        }
    }
    std::sort(pairs.begin(), pairs.end(), [this](const Pair& a, const Pair& b) {
        if (a.distance != b.distance) {
            return a.distance < b.distance;
        }
        if (tracks_[a.track_index].id != tracks_[b.track_index].id) {
            return tracks_[a.track_index].id < tracks_[b.track_index].id;
        }
        return a.detection_index < b.detection_index;
    });

    std::vector<bool> track_used(tracks_.size(), false);
    std::vector<bool> detection_used(detections.size(), false);
    for (const Pair& pair : pairs) {
        if (track_used[pair.track_index] || detection_used[pair.detection_index]) {
            continue;
        }
        track_used[pair.track_index] = true;
        detection_used[pair.detection_index] = true;

        Track& track = tracks_[pair.track_index];
        const Detection& detection = detections[pair.detection_index];

        // KF position measurement update (H = [I 0], R = r^2 I, per axis:
        // the two axes are only coupled through P, handled jointly below).
        std::array<double, 16>& P = track.P;
        const double r2 = r_pos_ * r_pos_;
        // Innovation covariance S (2x2) over the position block.
        const double S00 = P[0] + r2;
        const double S01 = P[1];
        const double S10 = P[4];
        const double S11 = P[5] + r2;
        const double det = S00 * S11 - S01 * S10;
        if (std::abs(det) > 1e-12) {
            const double i00 = S11 / det;
            const double i01 = -S01 / det;
            const double i10 = -S10 / det;
            const double i11 = S00 / det;
            const double yx = detection.position.x - track.x[0];
            const double yz = detection.position.z - track.x[1];
            // K = P H^T S^-1 (4x2); columns of P H^T are P[:,0] and P[:,1].
            std::array<double, 8> K{};
            for (int row = 0; row < 4; ++row) {
                const double c0 = P[static_cast<std::size_t>(row * 4 + 0)];
                const double c1 = P[static_cast<std::size_t>(row * 4 + 1)];
                K[static_cast<std::size_t>(row * 2 + 0)] = c0 * i00 + c1 * i10;
                K[static_cast<std::size_t>(row * 2 + 1)] = c0 * i01 + c1 * i11;
            }
            for (int row = 0; row < 4; ++row) {
                track.x[static_cast<std::size_t>(row)] +=
                    K[static_cast<std::size_t>(row * 2 + 0)] * yx
                    + K[static_cast<std::size_t>(row * 2 + 1)] * yz;
            }
            // P = (I - K H) P; K H has nonzero columns 0 and 1 only.
            std::array<double, 16> updated = P;
            for (int row = 0; row < 4; ++row) {
                for (int col = 0; col < 4; ++col) {
                    updated[static_cast<std::size_t>(row * 4 + col)] -=
                        K[static_cast<std::size_t>(row * 2 + 0)]
                            * P[static_cast<std::size_t>(0 * 4 + col)]
                        + K[static_cast<std::size_t>(row * 2 + 1)]
                            * P[static_cast<std::size_t>(1 * 4 + col)];
                }
            }
            P = updated;
        }

        track.radius_m = 0.7 * track.radius_m + 0.3 * detection.radius_m;
        track.class_id = detection.class_id;
        ++track.hits;
        track.missed = 0;
        ++track.age;
    }

    // Coast unassigned tracks; drop the ones missed too long.
    for (std::size_t ti = 0; ti < tracks_.size(); ++ti) {
        if (!track_used[ti]) {
            ++tracks_[ti].missed;
            ++tracks_[ti].age;
        }
    }
    tracks_.erase(
        std::remove_if(
            tracks_.begin(), tracks_.end(),
            [this](const Track& track) { return track.missed > max_missed_; }),
        tracks_.end());

    // Birth: one tentative track per unassigned detection.
    for (std::size_t di = 0; di < detections.size(); ++di) {
        if (detection_used[di]) {
            continue;
        }
        Track track;
        track.id = next_id_++;
        track.x = {detections[di].position.x, detections[di].position.z, 0.0, 0.0};
        track.P.fill(0.0);
        track.P[0] = r_pos_ * r_pos_;
        track.P[5] = r_pos_ * r_pos_;
        track.P[10] = init_vel_sigma_ * init_vel_sigma_;
        track.P[15] = init_vel_sigma_ * init_vel_sigma_;
        track.radius_m = detections[di].radius_m;
        track.class_id = detections[di].class_id;
        track.hits = 1;
        track.missed = 0;
        track.age = 1;
        tracks_.push_back(track);
    }

    // Report confirmed tracks only (birth after min_hits).
    std::vector<TrackedObject> confirmed;
    confirmed.reserve(tracks_.size());
    for (const Track& track : tracks_) {
        if (track.hits < min_hits_) {
            continue;
        }
        TrackedObject object;
        object.id = track.id;
        object.position = {track.x[0], 0.0, track.x[1]};
        object.velocity = {track.x[2], 0.0, track.x[3]};
        object.radius_m = track.radius_m;
        object.class_name = class_name_of(track.class_id);
        object.age = track.age;
        object.missed = track.missed;
        confirmed.push_back(object);
    }
    return confirmed;
}

const TrackerRegistry& default_tracker_registry() {
    static const TrackerRegistry registry = [] {
        TrackerRegistry built;
        built.register_factory(
            "greedy_nn",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<ITracker> {
                return std::make_unique<GreedyNnTracker>(params);
            });
        return built;
    }();
    return registry;
}

} // namespace agbot::nav
