#include "agbot_flight_sim/LabeledSyntheticDataset.hpp"

#include "agbot_flight_sim/TwinContractV1.hpp"

#include <algorithm>
#include <cmath>
#include <iomanip>
#include <limits>
#include <map>
#include <optional>
#include <sstream>
#include <string_view>
#include <utility>

namespace agbot::flight_sim {
namespace {

std::string escape_json(std::string_view value) {
    std::ostringstream output;
    for (const char c : value) {
        switch (c) {
            case '"':
                output << "\\\"";
                break;
            case '\\':
                output << "\\\\";
                break;
            case '\n':
                output << "\\n";
                break;
            case '\r':
                output << "\\r";
                break;
            case '\t':
                output << "\\t";
                break;
            default:
                output << c;
                break;
        }
    }
    return output.str();
}

void write_double(std::ostringstream& output, double value, int precision = 3) {
    output << std::fixed << std::setprecision(precision) << value;
}

void write_vec3_json(std::ostringstream& output, const Vec3& value) {
    output << "{"
           << "\"x\":";
    write_double(output, value.x);
    output << ",\"y\":";
    write_double(output, value.y);
    output << ",\"z\":";
    write_double(output, value.z);
    output << "}";
}

bool finite_pose(const Vec3& pose) {
    return std::isfinite(pose.x) && std::isfinite(pose.y) && std::isfinite(pose.z);
}

Vec3 centroid_for(const SceneObject& object) {
    if (object.footprint_local_m.empty()) {
        return {};
    }
    Vec3 sum;
    std::size_t count = object.footprint_local_m.size();
    if (count > 1 && object.footprint_local_m.front().x == object.footprint_local_m.back().x
        && object.footprint_local_m.front().z == object.footprint_local_m.back().z) {
        --count;
    }
    for (std::size_t index = 0; index < count; ++index) {
        sum = sum + object.footprint_local_m[index];
    }
    const double divisor = static_cast<double>(std::max<std::size_t>(1, count));
    return {sum.x / divisor, object.height_m, sum.z / divisor};
}

std::map<std::string, const SceneObject*> scene_objects_by_id(const SceneSynthesisManifest& scene) {
    std::map<std::string, const SceneObject*> objects;
    for (const SceneObject& object : scene.objects) {
        objects.emplace(object.object_id, &object);
    }
    return objects;
}

std::vector<DatasetClassLabel> class_legend_for_scene(const SceneSynthesisManifest& scene) {
    std::vector<std::string> classes {"terrain", "no_coverage", "range_exceeded"};
    for (const SceneObject& object : scene.objects) {
        if (!object.class_name.empty()
            && std::find(classes.begin(), classes.end(), object.class_name) == classes.end()) {
            classes.push_back(object.class_name);
        }
    }
    std::sort(classes.begin() + 3, classes.end());

    std::vector<DatasetClassLabel> legend;
    legend.reserve(classes.size());
    for (std::size_t index = 0; index < classes.size(); ++index) {
        legend.push_back({static_cast<std::uint16_t>(index), classes[index]});
    }
    return legend;
}

std::optional<std::uint16_t> class_id_for(
    const std::vector<DatasetClassLabel>& legend,
    const std::string& class_name) {
    const auto found = std::find_if(legend.begin(), legend.end(), [&](const DatasetClassLabel& label) {
        return label.class_name == class_name;
    });
    if (found == legend.end()) {
        return std::nullopt;
    }
    return found->class_id;
}

std::string label_frame_json_without_hash(const LabeledDatasetFrame& frame) {
    std::ostringstream output;
    output << "{"
           << "\"frame_index\":" << frame.frame_index
           << ",\"frame_hash\":\"" << escape_json(frame.frame_hash) << "\""
           << ",\"timestamp_s\":";
    write_double(output, frame.timestamp_s);
    output << ",\"camera_pose_m\":";
    write_vec3_json(output, frame.camera_pose_m);
    output << ",\"scenario_hash\":\"" << escape_json(frame.scenario_hash) << "\""
           << ",\"scene_hash\":\"" << escape_json(frame.scene_hash) << "\""
           << ",\"width\":" << frame.width
           << ",\"height\":" << frame.height
           << ",\"class_mask\":[";
    for (std::size_t index = 0; index < frame.class_mask.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        output << frame.class_mask[index];
    }
    output << "],\"depth_m\":[";
    for (std::size_t index = 0; index < frame.depth_m.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        write_double(output, frame.depth_m[index]);
    }
    output << "],\"object_ids\":[";
    for (std::size_t index = 0; index < frame.object_ids.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        output << "\"" << escape_json(frame.object_ids[index]) << "\"";
    }
    output << "]}";
    return output.str();
}

std::string dataset_json_without_hash(const LabeledSyntheticDatasetManifest& manifest) {
    std::ostringstream output;
    output << "{"
           << "\"contract_version\":\"" << escape_json(manifest.contract_version) << "\""
           << ",\"scenario_hash\":\"" << escape_json(manifest.scenario_hash) << "\""
           << ",\"scene_hash\":\"" << escape_json(manifest.scene_hash) << "\""
           << ",\"seed\":" << manifest.seed
           << ",\"exported_frame_count\":" << manifest.exported_frame_count()
           << ",\"excluded_frame_count\":" << manifest.excluded_frame_count()
           << ",\"class_legend\":[";
    for (std::size_t index = 0; index < manifest.class_legend.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        const DatasetClassLabel& label = manifest.class_legend[index];
        output << "{\"class_id\":" << label.class_id
               << ",\"class_name\":\"" << escape_json(label.class_name) << "\"}";
    }
    output << "],\"object_poses\":[";
    for (std::size_t index = 0; index < manifest.object_poses.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        const DatasetObjectPose& pose = manifest.object_poses[index];
        output << "{\"object_id\":\"" << escape_json(pose.object_id) << "\""
               << ",\"class_name\":\"" << escape_json(pose.class_name) << "\""
               << ",\"centroid_m\":";
        write_vec3_json(output, pose.centroid_m);
        output << ",\"height_m\":";
        write_double(output, pose.height_m);
        output << ",\"placement_seed\":" << pose.placement_seed << "}";
    }
    output << "],\"frames\":[";
    for (std::size_t index = 0; index < manifest.frames.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        std::string frame_json = label_frame_json_without_hash(manifest.frames[index]);
        frame_json.pop_back();
        output << frame_json
               << ",\"label_hash\":\"" << escape_json(manifest.frames[index].label_hash) << "\"}";
    }
    output << "],\"exclusions\":[";
    for (std::size_t index = 0; index < manifest.exclusions.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        const ExcludedDatasetFrame& exclusion = manifest.exclusions[index];
        output << "{\"frame_index\":" << exclusion.frame_index
               << ",\"frame_hash\":\"" << escape_json(exclusion.frame_hash) << "\""
               << ",\"reason\":\"" << escape_json(exclusion.reason) << "\"}";
    }
    output << "]}";
    return output.str();
}

std::optional<std::string> exclusion_reason(
    const RayTracedFrame& frame,
    const SceneSynthesisManifest& scene,
    const std::string& scenario_hash) {
    if (scenario_hash.empty()) {
        return "missing_scenario_linkage";
    }
    if (!finite_pose(frame.pose)) {
        return "missing_pose";
    }
    if (frame.scene_hash.empty()) {
        return "missing_scene_linkage";
    }
    if (frame.scene_hash != scene.scene_hash) {
        return "scene_hash_mismatch";
    }
    if (frame.status != "ok") {
        return "frame_not_ok";
    }
    if (frame.frame_hash.empty()
        || frame.width <= 0
        || frame.height <= 0
        || frame.pixels.size() != static_cast<std::size_t>(frame.width * frame.height)) {
        return "missing_frame_pixels";
    }
    return std::nullopt;
}

} // namespace

std::size_t LabeledSyntheticDatasetManifest::exported_frame_count() const {
    return frames.size();
}

std::size_t LabeledSyntheticDatasetManifest::excluded_frame_count() const {
    return exclusions.size();
}

std::uint16_t LabeledSyntheticDatasetManifest::class_id_for(const std::string& class_name) const {
    const auto id = agbot::flight_sim::class_id_for(class_legend, class_name);
    return id.value_or(std::numeric_limits<std::uint16_t>::max());
}

std::string LabeledSyntheticDatasetManifest::to_json() const {
    std::string json = dataset_json_without_hash(*this);
    json.pop_back();
    json += ",\"dataset_id\":\"" + escape_json(dataset_id) + "\"}";
    return json;
}

LabeledSyntheticDatasetManifest export_labeled_synthetic_dataset(
    const std::vector<RayTracedFrame>& frames,
    const SceneSynthesisManifest& scene,
    std::string scenario_hash,
    std::uint64_t seed) {
    LabeledSyntheticDatasetManifest manifest;
    manifest.contract_version = twin_contract_v1_schema().version;
    manifest.scenario_hash = std::move(scenario_hash);
    manifest.scene_hash = scene.scene_hash;
    manifest.seed = seed;
    manifest.class_legend = class_legend_for_scene(scene);

    const auto objects = scene_objects_by_id(scene);
    for (const SceneObject& object : scene.objects) {
        manifest.object_poses.push_back({
            object.object_id,
            object.class_name,
            centroid_for(object),
            object.height_m,
            object.placement_seed,
        });
    }

    for (std::size_t frame_index = 0; frame_index < frames.size(); ++frame_index) {
        const RayTracedFrame& source = frames[frame_index];
        if (const auto reason = exclusion_reason(source, scene, manifest.scenario_hash)) {
            manifest.exclusions.push_back({frame_index, source.frame_hash, *reason});
            continue;
        }

        LabeledDatasetFrame frame;
        frame.frame_index = frame_index;
        frame.frame_hash = source.frame_hash;
        frame.timestamp_s = source.timestamp_s;
        frame.camera_pose_m = source.pose;
        frame.scenario_hash = manifest.scenario_hash;
        frame.scene_hash = source.scene_hash;
        frame.width = source.width;
        frame.height = source.height;
        frame.class_mask.reserve(source.pixels.size());
        frame.depth_m.reserve(source.pixels.size());
        frame.object_ids.reserve(source.pixels.size());

        bool label_mismatch = false;
        for (const RayTracedPixel& pixel : source.pixels) {
            const auto class_id = agbot::flight_sim::class_id_for(manifest.class_legend, pixel.class_name);
            if (!class_id.has_value()) {
                label_mismatch = true;
                break;
            }
            if (!pixel.object_id.empty()) {
                const auto object = objects.find(pixel.object_id);
                if (object == objects.end()
                    || object->second->class_name != pixel.class_name
                    || object->second->placement_seed != pixel.object_seed) {
                    label_mismatch = true;
                    break;
                }
            }
            frame.class_mask.push_back(*class_id);
            frame.depth_m.push_back(pixel.depth_m);
            frame.object_ids.push_back(pixel.object_id);
        }

        if (label_mismatch) {
            manifest.exclusions.push_back({frame_index, source.frame_hash, "label_scene_mismatch"});
            continue;
        }

        frame.label_hash = sha256_hex(label_frame_json_without_hash(frame));
        manifest.frames.push_back(std::move(frame));
    }

    manifest.dataset_id = sha256_hex(dataset_json_without_hash(manifest));
    return manifest;
}

} // namespace agbot::flight_sim
