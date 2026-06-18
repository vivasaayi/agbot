#pragma once

#include "agbot_flight_sim/RayTracedCamera.hpp"
#include "agbot_flight_sim/SceneSynthesis.hpp"
#include "agbot_flight_sim/Vec3.hpp"

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

namespace agbot::flight_sim {

struct DatasetClassLabel {
    std::uint16_t class_id = 0;
    std::string class_name;
};

struct DatasetObjectPose {
    std::string object_id;
    std::string class_name;
    Vec3 centroid_m;
    double height_m = 0.0;
    std::uint64_t placement_seed = 0;
};

struct LabeledDatasetFrame {
    std::size_t frame_index = 0;
    std::string frame_hash;
    double timestamp_s = 0.0;
    Vec3 camera_pose_m;
    std::string scenario_hash;
    std::string scene_hash;
    int width = 0;
    int height = 0;
    std::vector<std::uint16_t> class_mask;
    std::vector<double> depth_m;
    std::vector<std::string> object_ids;
    std::string label_hash;
};

struct ExcludedDatasetFrame {
    std::size_t frame_index = 0;
    std::string frame_hash;
    std::string reason;
};

struct LabeledSyntheticDatasetManifest {
    std::string contract_version;
    std::string scenario_hash;
    std::string scene_hash;
    std::uint64_t seed = 0;
    std::vector<DatasetClassLabel> class_legend;
    std::vector<DatasetObjectPose> object_poses;
    std::vector<LabeledDatasetFrame> frames;
    std::vector<ExcludedDatasetFrame> exclusions;
    std::string dataset_id;

    [[nodiscard]] std::size_t exported_frame_count() const;
    [[nodiscard]] std::size_t excluded_frame_count() const;
    [[nodiscard]] std::uint16_t class_id_for(const std::string& class_name) const;
    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] LabeledSyntheticDatasetManifest export_labeled_synthetic_dataset(
    const std::vector<RayTracedFrame>& frames,
    const SceneSynthesisManifest& scene,
    std::string scenario_hash,
    std::uint64_t seed);

} // namespace agbot::flight_sim
