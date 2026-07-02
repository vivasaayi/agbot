#include "agbot_nav/NavigationPipeline.hpp"

#include <algorithm>
#include <cmath>
#include <limits>

namespace agbot::nav {

namespace {

const agbot::config::ParamTable kEmptyTable;

const agbot::config::ParamTable& section_or_empty(
    const agbot::config::ParamTable& root,
    const std::string& name) {
    const agbot::config::ParamTable* table = agbot::config::find_table(root, name);
    return table != nullptr ? *table : kEmptyTable;
}

std::string algorithm_of(const agbot::config::ParamTable& section, const std::string& fallback) {
    return agbot::config::string_or(section, "algorithm", fallback);
}

} // namespace

NavigationPipelineConfig parse_pipeline_config(const std::string& toml_text) {
    NavigationPipelineConfig config;
    const agbot::config::TomlParseResult parsed = agbot::config::parse_toml(toml_text);
    if (!parsed.ok) {
        config.error = "toml_parse_failed: " + parsed.error;
        return config;
    }
    config.root = parsed.root;
    config.param_hash = agbot::config::param_hash(config.root);
    config.ok = true;
    return config;
}

NavigationPipeline::NavigationPipeline(const NavigationPipelineConfig& config) {
    if (!config.ok) {
        error_ = config.error.empty() ? "invalid_pipeline_config" : config.error;
        return;
    }
    param_hash_ = config.param_hash;
    const agbot::config::ParamTable& root = config.root;

    const agbot::config::ParamTable& pipeline = section_or_empty(root, "pipeline");
    sensor_period_s_ = agbot::config::double_or(pipeline, "sensor_period_s", sensor_period_s_);
    map_period_s_ = agbot::config::double_or(pipeline, "map_period_s", map_period_s_);
    plan_period_s_ = agbot::config::double_or(pipeline, "plan_period_s", plan_period_s_);
    local_period_s_ = agbot::config::double_or(pipeline, "local_period_s", local_period_s_);
    goal_tolerance_m_ = agbot::config::double_or(pipeline, "goal_tolerance_m", goal_tolerance_m_);

    const agbot::config::ParamTable& vehicle = section_or_empty(root, "vehicle");
    vehicle_ = agbot::vehicles::create_vehicle_model(
        algorithm_of(vehicle, "kinematic_bicycle"), vehicle);
    if (vehicle_ == nullptr) {
        error_ = "unknown_vehicle_model: " + algorithm_of(vehicle, "kinematic_bicycle");
        return;
    }

    const agbot::config::ParamTable& sensing = section_or_empty(root, "sensing");
    sensor_ = default_sensor_registry().create(algorithm_of(sensing, "depth_camera"), sensing);
    if (sensor_ == nullptr) {
        error_ = "unknown_sensor: " + algorithm_of(sensing, "depth_camera");
        return;
    }

    const agbot::config::ParamTable& perception = section_or_empty(root, "perception");
    perception_ = default_perception_registry().create(
        algorithm_of(perception, "height_threshold"), perception);
    if (perception_ == nullptr) {
        error_ = "unknown_perception: " + algorithm_of(perception, "height_threshold");
        return;
    }

    const agbot::config::ParamTable& mapping = section_or_empty(root, "mapping");
    mapper_ = default_mapper_registry().create(algorithm_of(mapping, "occupancy_grid"), mapping);
    if (mapper_ == nullptr) {
        error_ = "unknown_mapper: " + algorithm_of(mapping, "occupancy_grid");
        return;
    }
    inflation_ = InflationLayer(mapping);

    const agbot::config::ParamTable& global = section_or_empty(root, "global");
    global_planner_ =
        default_global_planner_registry().create(algorithm_of(global, "astar"), global);
    if (global_planner_ == nullptr) {
        error_ = "unknown_global_planner: " + algorithm_of(global, "astar");
        return;
    }

    const agbot::config::ParamTable& local = section_or_empty(root, "local");
    local_planner_ =
        default_local_planner_registry().create(algorithm_of(local, "pure_pursuit"), local);
    if (local_planner_ == nullptr) {
        error_ = "unknown_local_planner: " + algorithm_of(local, "pure_pursuit");
        return;
    }

    const agbot::config::ParamTable& control = section_or_empty(root, "control");
    controller_ =
        default_controller_registry().create(algorithm_of(control, "pid_stanley"), control);
    if (controller_ == nullptr) {
        error_ = "unknown_controller: " + algorithm_of(control, "pid_stanley");
        return;
    }

    const agbot::config::ParamTable& localization = section_or_empty(root, "localization");
    const std::string localizer_name = algorithm_of(localization, "ground_truth");
    localizer_ = default_localizer_registry().create(localizer_name, localization);
    if (localizer_ == nullptr) {
        error_ = "unknown_localizer: " + localizer_name;
        return;
    }
    noisy_localization_ = localizer_name != "ground_truth";
    gps_sigma_m_ = agbot::config::double_or(localization, "gps_sigma_m", gps_sigma_m_);
    gps_period_s_ = agbot::config::double_or(localization, "gps_period_s", gps_period_s_);
    compass_sigma_rad_ =
        agbot::config::double_or(localization, "compass_sigma_rad", compass_sigma_rad_);
    compass_period_s_ =
        agbot::config::double_or(localization, "compass_period_s", compass_period_s_);
    odom_v_sigma_ = agbot::config::double_or(localization, "odom_v_sigma", odom_v_sigma_);
    odom_yaw_rate_sigma_ =
        agbot::config::double_or(localization, "odom_yaw_rate_sigma", odom_yaw_rate_sigma_);
    noise_seed_ = static_cast<std::uint64_t>(std::max<std::int64_t>(
        0, agbot::config::integer_or(
               localization, "noise_seed", static_cast<std::int64_t>(noise_seed_))));

    ok_ = true;
}

std::uint8_t NavigationPipeline::lethal_threshold() const {
    return inflation_.lethal_threshold();
}

void NavigationPipeline::tick(
    const NavWorld& world,
    agbot::vehicles::EntityState& state,
    const Vec3& goal,
    double dt_s) {
    if (!ok_ || dt_s <= 0.0) {
        return;
    }

    sensor_elapsed_s_ += dt_s;
    plan_elapsed_s_ += dt_s;
    local_elapsed_s_ += dt_s;

    // Localization stage: pass truth through (ground_truth uses it, filters
    // initialize from it once), then for filter localizers synthesize
    // deterministic noisy odometry/GPS/compass measurements from the true
    // motion and run predict/correct. Planning below consumes the estimate.
    const Pose2D truth{state.position.x, state.position.z, state.yaw_rad};
    const double truth_speed = state.velocity.horizontal_length();
    localizer_->observe_truth(truth, truth_speed);
    if (noisy_localization_) {
        if (has_last_truth_) {
            const double dx = truth.x - last_truth_.x;
            const double dz = truth.z - last_truth_.z;
            const double forward = dx * std::cos(last_truth_.yaw) + dz * std::sin(last_truth_.yaw);
            double dyaw = truth.yaw - last_truth_.yaw;
            while (dyaw > 3.14159265358979323846) {
                dyaw -= 2.0 * 3.14159265358979323846;
            }
            while (dyaw < -3.14159265358979323846) {
                dyaw += 2.0 * 3.14159265358979323846;
            }
            const double v_odom = forward / dt_s
                + odom_v_sigma_ * noise::gaussian(noise_seed_, 1, noise_counter_);
            const double yaw_rate_odom = dyaw / dt_s
                + odom_yaw_rate_sigma_ * noise::gaussian(noise_seed_, 2, noise_counter_);
            localizer_->predict(v_odom, yaw_rate_odom, dt_s);
        }
        gps_elapsed_s_ += dt_s;
        if (gps_elapsed_s_ + 1e-9 >= gps_period_s_) {
            gps_elapsed_s_ = 0.0;
            const double quality = std::clamp(gps_quality_, 0.01, 1.0);
            const double sigma_eff = gps_sigma_m_ / quality;
            const double gx =
                truth.x + sigma_eff * noise::gaussian(noise_seed_, 3, noise_counter_);
            const double gz =
                truth.z + sigma_eff * noise::gaussian(noise_seed_, 4, noise_counter_);
            localizer_->correct_gps(gx, gz, gps_sigma_m_, quality);
        }
        compass_elapsed_s_ += dt_s;
        if (compass_elapsed_s_ + 1e-9 >= compass_period_s_) {
            compass_elapsed_s_ = 0.0;
            const double heading = truth.yaw
                + compass_sigma_rad_ * noise::gaussian(noise_seed_, 5, noise_counter_);
            localizer_->correct_heading(heading, compass_sigma_rad_);
        }
        ++noise_counter_;
    }
    has_last_truth_ = true;
    last_truth_ = truth;

    // The pose the planners consume: exactly the true state for ground_truth,
    // the filter estimate otherwise.
    agbot::vehicles::EntityState nav_state = state;
    if (noisy_localization_) {
        const Pose2D estimate = localizer_->pose();
        const double est_speed = localizer_->speed_mps();
        nav_state.position.x = estimate.x;
        nav_state.position.z = estimate.z;
        nav_state.yaw_rad = estimate.yaw;
        nav_state.velocity = {
            est_speed * std::cos(estimate.yaw),
            0.0,
            est_speed * std::sin(estimate.yaw),
        };
    }

    // Sensing + perception + mapping stage.
    if (sensor_elapsed_s_ + 1e-9 >= sensor_period_s_) {
        sensor_elapsed_s_ = 0.0;
        const SensorFrame frame = sensor_->sense(world, state, time_s_);
        last_min_range_m_ = 0.0;
        double min_range = std::numeric_limits<double>::infinity();
        for (const double range : frame.depth_m) {
            if (range > 0.0) {
                min_range = std::min(min_range, range);
            }
        }
        if (std::isfinite(min_range)) {
            last_min_range_m_ = min_range;
        }
        const PerceptionResult segmented = perception_->segment(frame);
        const Pose2D sensor_pose{
            nav_state.position.x, nav_state.position.z, nav_state.yaw_rad};
        mapper_->integrate(segmented.obstacles, sensor_pose, time_s_);
        costmap_ = inflation_.inflate(mapper_->grid());
        has_costmap_ = true;
    }

    // Global planning stage.
    if (has_costmap_ && (plan_elapsed_s_ + 1e-9 >= plan_period_s_ || global_path_.points.empty())) {
        plan_elapsed_s_ = 0.0;
        const PlanResult planned = global_planner_->plan(costmap_, nav_state.position, goal);
        if (planned.ok) {
            global_path_ = planned.path;
        }
    }

    // Local planning stage.
    if (!global_path_.points.empty() && local_elapsed_s_ + 1e-9 >= local_period_s_) {
        local_elapsed_s_ = 0.0;
        local_plan_ =
            local_planner_->compute(costmap_, global_path_, nav_state, vehicle_->limits(), goal);
    }

    // Control + vehicle integration every tick.
    const double goal_distance = (goal - nav_state.position).horizontal_length();
    if (goal_distance <= goal_tolerance_m_) {
        goal_reached_ = true;
    }

    agbot::vehicles::Actuation actuation;
    if (goal_reached_) {
        actuation.throttle = -1.0; // brake to a stop
        actuation.steer_rad = 0.0;
    } else if (local_plan_.ok) {
        Path reference;
        reference.points.reserve(local_plan_.trajectory.points.size());
        for (const TrajectoryPoint& point : local_plan_.trajectory.points) {
            reference.points.push_back(point.position);
        }
        actuation = controller_->control(
            nav_state, reference, local_plan_.v_cmd, vehicle_->limits(), dt_s);
    }
    state = vehicle_->step(state, actuation, dt_s);
    time_s_ += dt_s;

    // Telemetry evidence.
    NavTelemetry sample;
    sample.time_s = time_s_;
    sample.pose = {state.position.x, state.position.z, state.yaw_rad};
    sample.speed_mps = state.velocity.horizontal_length();
    if (has_costmap_) {
        sample.robot_cell_cost = costmap_.cost_at_world(state.position.x, state.position.z);
        for (const std::uint8_t cell : costmap_.cells) {
            if (cell != OccupancyGrid::kUnknown && cell > 0) {
                ++sample.costmap_occupied_cells;
            }
            if (cell == OccupancyGrid::kLethal) {
                ++sample.costmap_lethal_cells;
            }
        }
    }
    sample.path_length_m = global_path_.length_m();
    sample.min_obstacle_distance_m = last_min_range_m_;
    sample.distance_to_goal_m = goal_distance;
    sample.goal_reached = goal_reached_;
    telemetry_.push_back(sample);
}

} // namespace agbot::nav
