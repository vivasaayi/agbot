#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_config/Toml.hpp"
#include "agbot_nav/Controller.hpp"
#include "agbot_nav/GlobalPlanner.hpp"
#include "agbot_nav/LocalPlanner.hpp"
#include "agbot_nav/Localization.hpp"
#include "agbot_nav/Mapping.hpp"
#include "agbot_nav/Perception.hpp"
#include "agbot_nav/Sensing.hpp"
#include "agbot_vehicles/IVehicleModel.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <vector>

namespace agbot::nav {

struct NavigationPipelineConfig {
    bool ok = false;
    std::string error;
    std::uint64_t param_hash = 0; // hash of the full config table
    agbot::config::ParamTable root;
};

// Parse a pipeline configuration from TOML text. Expected sections, each with
// an `algorithm` key plus algorithm params: [vehicle], [sensing],
// [perception], [mapping], [global], [local], [control]; plus [pipeline] with
// stage periods (sensor_period_s, map_period_s, plan_period_s,
// local_period_s, goal_tolerance_m). The optional [localization] section
// selects the pose source (`algorithm = "ground_truth"` by default, or
// "ekf_2d") plus filter params; for non-ground-truth localizers the pipeline
// also synthesizes deterministic noisy GPS/compass/odometry measurements from
// the true state (gps_sigma_m, gps_period_s, compass_sigma_rad,
// compass_period_s, odom_v_sigma, odom_yaw_rate_sigma, noise_seed) and plans
// from the estimated pose instead of the raw EntityState.
[[nodiscard]] NavigationPipelineConfig parse_pipeline_config(const std::string& toml_text);

// Staged navigation pipeline: sense -> perceive -> map/inflate -> global plan
// -> local plan -> control -> vehicle integration, each stage hot-swappable
// via the strategy registries and running at its configured rate.
class NavigationPipeline {
public:
    // Builds every stage from the parsed config. ok()/error() report
    // construction failures instead of throwing.
    explicit NavigationPipeline(const NavigationPipelineConfig& config);

    [[nodiscard]] bool ok() const { return ok_; }
    [[nodiscard]] const std::string& error() const { return error_; }
    [[nodiscard]] std::uint64_t param_hash() const { return param_hash_; }

    // Advance the world by dt_s: runs due stages, controls and integrates the
    // vehicle, appends one NavTelemetry sample.
    void tick(const NavWorld& world, agbot::vehicles::EntityState& state, const Vec3& goal,
              double dt_s);

    [[nodiscard]] bool goal_reached() const { return goal_reached_; }
    [[nodiscard]] const Path& global_path() const { return global_path_; }
    [[nodiscard]] const Costmap& costmap() const { return costmap_; }
    [[nodiscard]] const std::vector<NavTelemetry>& telemetry() const { return telemetry_; }
    [[nodiscard]] std::uint8_t lethal_threshold() const;

    // Localization stage access: the pose the planners actually consume and
    // the localizer itself (covariance inspection in tests/scenarios).
    [[nodiscard]] Pose2D estimated_pose() const { return localizer_->pose(); }
    [[nodiscard]] const ILocalizer& localizer() const { return *localizer_; }

    // Urban-canyon degradation hook: scales the simulated GPS noise and the
    // EKF measurement covariance until changed again. 1.0 = nominal.
    void set_gps_quality(double quality) { gps_quality_ = quality; }

private:
    bool ok_ = false;
    std::string error_;
    std::uint64_t param_hash_ = 0;

    std::unique_ptr<agbot::vehicles::IVehicleModel> vehicle_;
    std::unique_ptr<ISensor> sensor_;
    std::unique_ptr<IPerception> perception_;
    std::unique_ptr<IMapper> mapper_;
    InflationLayer inflation_;
    std::unique_ptr<IGlobalPlanner> global_planner_;
    std::unique_ptr<ILocalPlanner> local_planner_;
    std::unique_ptr<IController> controller_;
    std::unique_ptr<ILocalizer> localizer_;

    // Simulated navigation sensors for non-ground-truth localizers.
    bool noisy_localization_ = false;
    double gps_sigma_m_ = 1.0;
    double gps_period_s_ = 0.2;
    double compass_sigma_rad_ = 0.05;
    double compass_period_s_ = 0.1;
    double odom_v_sigma_ = 0.1;
    double odom_yaw_rate_sigma_ = 0.02;
    std::uint64_t noise_seed_ = 1;
    double gps_quality_ = 1.0;
    double gps_elapsed_s_ = 1e9;
    double compass_elapsed_s_ = 1e9;
    std::uint64_t noise_counter_ = 0;
    bool has_last_truth_ = false;
    Pose2D last_truth_;

    double sensor_period_s_ = 0.1;
    double map_period_s_ = 0.1;
    double plan_period_s_ = 1.0;
    double local_period_s_ = 0.05;
    double goal_tolerance_m_ = 1.0;

    double time_s_ = 0.0;
    double sensor_elapsed_s_ = 1e9;
    double plan_elapsed_s_ = 1e9;
    double local_elapsed_s_ = 1e9;

    Costmap costmap_;
    bool has_costmap_ = false;
    Path global_path_;
    LocalPlan local_plan_;
    double last_min_range_m_ = 0.0;
    bool goal_reached_ = false;
    std::vector<NavTelemetry> telemetry_;
};

} // namespace agbot::nav
