#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_nav/NavTypes.hpp"
#include "agbot_vehicles/ParamRegistry.hpp"

#include <array>
#include <cmath>
#include <cstdint>
#include <memory>
#include <string>

namespace agbot::nav {

// Deterministic counter-based noise helpers shared by the pipeline's sensor
// simulation and the localization tests: identical (seed, stream, counter)
// keys always produce identical draws, independent of platform library
// distributions.
namespace noise {

[[nodiscard]] inline std::uint64_t splitmix64(std::uint64_t x) {
    x += 0x9E3779B97F4A7C15ULL;
    x = (x ^ (x >> 30)) * 0xBF58476D1CE4E5B9ULL;
    x = (x ^ (x >> 27)) * 0x94D049BB133111EBULL;
    return x ^ (x >> 31);
}

// One standard normal draw via Box-Muller over two derived uniforms.
[[nodiscard]] inline double gaussian(
    std::uint64_t seed,
    std::uint64_t stream,
    std::uint64_t counter) {
    std::uint64_t key = splitmix64(seed ^ 0xC2B2AE3D27D4EB4FULL);
    key = splitmix64(key ^ (stream * 0x9E3779B97F4A7C15ULL));
    key = splitmix64(key ^ (counter * 0xD1B54A32D192ED03ULL));
    const double u1 = (static_cast<double>(key >> 11) + 1.0) * 0x1.0p-53;
    const double u2 =
        (static_cast<double>(splitmix64(key) >> 11) + 1.0) * 0x1.0p-53;
    return std::sqrt(-2.0 * std::log(u1))
        * std::cos(6.28318530717958647692 * u2);
}

} // namespace noise

// Localization stage interface. The pipeline drives it every tick:
// observe_truth (pass-through localizers use it, filters ignore it after
// initialization), predict from odometry, and correct with GPS/compass
// measurements. GPS quality in [0, 1] scales the measurement covariance R so
// urban-canyon degradation can be injected per tick.
class ILocalizer {
public:
    virtual ~ILocalizer() = default;

    // True pose injection: ground_truth passes it through; ekf_2d only uses
    // the first call to initialize its state.
    virtual void observe_truth(const Pose2D& pose, double speed_mps) = 0;

    // Dead-reckoning prediction from measured odometry.
    virtual void predict(double v_mps, double yaw_rate_radps, double dt_s) = 0;

    // GPS position fix; effective sigma = sigma_m / max(quality, floor).
    virtual void correct_gps(double x, double z, double sigma_m, double quality) = 0;

    // Compass heading fix.
    virtual void correct_heading(double yaw_rad, double sigma_rad) = 0;

    [[nodiscard]] virtual Pose2D pose() const = 0;
    [[nodiscard]] virtual double speed_mps() const = 0;

    // Row-major 4x4 covariance over (x, z, yaw, v). Zero for pass-through.
    [[nodiscard]] virtual std::array<double, 16> covariance() const = 0;
    [[nodiscard]] virtual std::string name() const = 0;
};

// Pass-through localizer: pose() is exactly the last observed truth, so the
// pipeline behaves identically to running without a localization stage.
class GroundTruthLocalizer final : public ILocalizer {
public:
    GroundTruthLocalizer() = default;
    explicit GroundTruthLocalizer(const agbot::config::ParamTable& params);

    void observe_truth(const Pose2D& pose, double speed_mps) override;
    void predict(double v_mps, double yaw_rate_radps, double dt_s) override;
    void correct_gps(double x, double z, double sigma_m, double quality) override;
    void correct_heading(double yaw_rad, double sigma_rad) override;

    [[nodiscard]] Pose2D pose() const override { return pose_; }
    [[nodiscard]] double speed_mps() const override { return speed_mps_; }
    [[nodiscard]] std::array<double, 16> covariance() const override;
    [[nodiscard]] std::string name() const override { return "ground_truth"; }

private:
    Pose2D pose_;
    double speed_mps_ = 0.0;
};

// Planar EKF over state (x, z, yaw, v). Prediction integrates odometry
// (measured speed and yaw rate) with process noise from the odometry sigmas;
// corrections are standard EKF updates: GPS position (R scaled by
// 1/quality^2, so degraded fixes are trusted less) and compass heading with
// wrapped innovation. The first observe_truth initializes the state; later
// calls are ignored.
//
// Params: sigma_odom_v (process, m/s), sigma_odom_yaw_rate (process, rad/s),
// init_pos_sigma_m, init_yaw_sigma_rad, init_v_sigma, min_gps_quality.
class Ekf2dLocalizer final : public ILocalizer {
public:
    Ekf2dLocalizer() = default;
    explicit Ekf2dLocalizer(const agbot::config::ParamTable& params);

    void observe_truth(const Pose2D& pose, double speed_mps) override;
    void predict(double v_mps, double yaw_rate_radps, double dt_s) override;
    void correct_gps(double x, double z, double sigma_m, double quality) override;
    void correct_heading(double yaw_rad, double sigma_rad) override;

    [[nodiscard]] Pose2D pose() const override;
    [[nodiscard]] double speed_mps() const override { return state_[3]; }
    [[nodiscard]] std::array<double, 16> covariance() const override;
    [[nodiscard]] std::string name() const override { return "ekf_2d"; }

    // Sum of the position variances P(0,0) + P(1,1); tests use it to check
    // covariance growth during GPS outage and contraction after recovery.
    [[nodiscard]] double position_variance() const { return p_[0][0] + p_[1][1]; }

private:
    bool initialized_ = false;
    std::array<double, 4> state_{0.0, 0.0, 0.0, 0.0}; // x, z, yaw, v
    std::array<std::array<double, 4>, 4> p_{};
    double sigma_odom_v_ = 0.15;
    double sigma_odom_yaw_rate_ = 0.03;
    double init_pos_sigma_m_ = 0.5;
    double init_yaw_sigma_rad_ = 0.05;
    double init_v_sigma_ = 0.2;
    double min_gps_quality_ = 0.01;
};

using LocalizerRegistry = agbot::vehicles::ParamRegistry<ILocalizer>;

// Registry pre-populated with "ground_truth" and "ekf_2d".
[[nodiscard]] const LocalizerRegistry& default_localizer_registry();

} // namespace agbot::nav
