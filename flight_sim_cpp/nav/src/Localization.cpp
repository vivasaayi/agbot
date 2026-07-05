#include "agbot_nav/Localization.hpp"

#include <algorithm>
#include <cmath>

namespace agbot::nav {

namespace {

constexpr double kPi = 3.14159265358979323846;
constexpr double kTwoPi = 2.0 * kPi;

double wrap_angle(double angle) {
    while (angle > kPi) {
        angle -= kTwoPi;
    }
    while (angle < -kPi) {
        angle += kTwoPi;
    }
    return angle;
}

} // namespace

// ---------------------------------------------------------------------------
// GroundTruthLocalizer
// ---------------------------------------------------------------------------

GroundTruthLocalizer::GroundTruthLocalizer(const agbot::config::ParamTable& params) {
    (void)params; // pass-through has no parameters
}

void GroundTruthLocalizer::observe_truth(const Pose2D& pose, double speed_mps) {
    pose_ = pose;
    speed_mps_ = speed_mps;
}

void GroundTruthLocalizer::predict(double v_mps, double yaw_rate_radps, double dt_s) {
    (void)v_mps;
    (void)yaw_rate_radps;
    (void)dt_s;
}

void GroundTruthLocalizer::correct_gps(double x, double z, double sigma_m, double quality) {
    (void)x;
    (void)z;
    (void)sigma_m;
    (void)quality;
}

void GroundTruthLocalizer::correct_heading(double yaw_rad, double sigma_rad) {
    (void)yaw_rad;
    (void)sigma_rad;
}

std::array<double, 16> GroundTruthLocalizer::covariance() const {
    return {};
}

// ---------------------------------------------------------------------------
// Ekf2dLocalizer
// ---------------------------------------------------------------------------

Ekf2dLocalizer::Ekf2dLocalizer(const agbot::config::ParamTable& params) {
    using agbot::config::double_or;
    sigma_odom_v_ = double_or(params, "sigma_odom_v", sigma_odom_v_);
    sigma_odom_yaw_rate_ = double_or(params, "sigma_odom_yaw_rate", sigma_odom_yaw_rate_);
    init_pos_sigma_m_ = double_or(params, "init_pos_sigma_m", init_pos_sigma_m_);
    init_yaw_sigma_rad_ = double_or(params, "init_yaw_sigma_rad", init_yaw_sigma_rad_);
    init_v_sigma_ = double_or(params, "init_v_sigma", init_v_sigma_);
    min_gps_quality_ = double_or(params, "min_gps_quality", min_gps_quality_);
}

void Ekf2dLocalizer::observe_truth(const Pose2D& pose, double speed_mps) {
    if (initialized_) {
        return; // filters only use truth once, to initialize
    }
    initialized_ = true;
    state_ = {pose.x, pose.z, pose.yaw, speed_mps};
    p_ = {};
    p_[0][0] = init_pos_sigma_m_ * init_pos_sigma_m_;
    p_[1][1] = init_pos_sigma_m_ * init_pos_sigma_m_;
    p_[2][2] = init_yaw_sigma_rad_ * init_yaw_sigma_rad_;
    p_[3][3] = init_v_sigma_ * init_v_sigma_;
}

void Ekf2dLocalizer::predict(double v_mps, double yaw_rate_radps, double dt_s) {
    if (!initialized_ || dt_s <= 0.0) {
        return;
    }
    const double yaw = state_[2];
    const double ds = v_mps * dt_s;

    // State propagation with the odometry as control input; v tracks the
    // measured speed directly.
    state_[0] += ds * std::cos(yaw);
    state_[1] += ds * std::sin(yaw);
    state_[2] = wrap_angle(state_[2] + yaw_rate_radps * dt_s);
    state_[3] = v_mps;

    // Jacobian F of the propagation w.r.t. (x, z, yaw, v). The new v comes
    // entirely from the measurement, so its row is zero and Q re-injects the
    // odometry speed variance.
    std::array<std::array<double, 4>, 4> f{};
    f[0][0] = 1.0;
    f[0][2] = -ds * std::sin(yaw);
    f[1][1] = 1.0;
    f[1][2] = ds * std::cos(yaw);
    f[2][2] = 1.0;

    std::array<std::array<double, 4>, 4> fp{};
    for (int i = 0; i < 4; ++i) {
        for (int j = 0; j < 4; ++j) {
            double acc = 0.0;
            for (int k = 0; k < 4; ++k) {
                acc += f[i][k] * p_[k][j];
            }
            fp[i][j] = acc;
        }
    }
    std::array<std::array<double, 4>, 4> next{};
    for (int i = 0; i < 4; ++i) {
        for (int j = 0; j < 4; ++j) {
            double acc = 0.0;
            for (int k = 0; k < 4; ++k) {
                acc += fp[i][k] * f[j][k];
            }
            next[i][j] = acc;
        }
    }

    // Process noise from the odometry sigmas over this step.
    const double q_pos = sigma_odom_v_ * dt_s;
    const double q_yaw = sigma_odom_yaw_rate_ * dt_s;
    next[0][0] += q_pos * q_pos;
    next[1][1] += q_pos * q_pos;
    next[2][2] += q_yaw * q_yaw;
    next[3][3] += sigma_odom_v_ * sigma_odom_v_;
    p_ = next;
}

void Ekf2dLocalizer::correct_gps(double x, double z, double sigma_m, double quality) {
    if (!initialized_ || !(sigma_m > 0.0)) {
        return;
    }
    // Urban-canyon degradation: quality in [0, 1] inflates R.
    const double q = std::clamp(quality, min_gps_quality_, 1.0);
    const double sigma_eff = sigma_m / q;
    const double r = sigma_eff * sigma_eff;

    // H = [[1,0,0,0],[0,1,0,0]]; S = P(0:2,0:2) + R.
    const double s00 = p_[0][0] + r;
    const double s01 = p_[0][1];
    const double s10 = p_[1][0];
    const double s11 = p_[1][1] + r;
    const double det = s00 * s11 - s01 * s10;
    if (std::abs(det) < 1e-18) {
        return;
    }
    const double i00 = s11 / det;
    const double i01 = -s01 / det;
    const double i10 = -s10 / det;
    const double i11 = s00 / det;

    // K = P H^T S^-1 (4x2); P H^T is just the first two columns of P.
    double k[4][2];
    for (int i = 0; i < 4; ++i) {
        k[i][0] = p_[i][0] * i00 + p_[i][1] * i10;
        k[i][1] = p_[i][0] * i01 + p_[i][1] * i11;
    }
    const double y0 = x - state_[0];
    const double y1 = z - state_[1];
    for (int i = 0; i < 4; ++i) {
        state_[i] += k[i][0] * y0 + k[i][1] * y1;
    }
    state_[2] = wrap_angle(state_[2]);

    // P = (I - K H) P; K H only touches columns 0 and 1 of the identity.
    std::array<std::array<double, 4>, 4> next{};
    for (int i = 0; i < 4; ++i) {
        for (int j = 0; j < 4; ++j) {
            next[i][j] = p_[i][j] - k[i][0] * p_[0][j] - k[i][1] * p_[1][j];
        }
    }
    p_ = next;
}

void Ekf2dLocalizer::correct_heading(double yaw_rad, double sigma_rad) {
    if (!initialized_ || !(sigma_rad > 0.0)) {
        return;
    }
    // H = [0,0,1,0]; scalar update with wrapped innovation.
    const double s = p_[2][2] + sigma_rad * sigma_rad;
    if (s < 1e-18) {
        return;
    }
    double k[4];
    for (int i = 0; i < 4; ++i) {
        k[i] = p_[i][2] / s;
    }
    const double innovation = wrap_angle(yaw_rad - state_[2]);
    for (int i = 0; i < 4; ++i) {
        state_[i] += k[i] * innovation;
    }
    state_[2] = wrap_angle(state_[2]);

    std::array<std::array<double, 4>, 4> next{};
    for (int i = 0; i < 4; ++i) {
        for (int j = 0; j < 4; ++j) {
            next[i][j] = p_[i][j] - k[i] * p_[2][j];
        }
    }
    p_ = next;
}

Pose2D Ekf2dLocalizer::pose() const {
    return {state_[0], state_[1], state_[2]};
}

std::array<double, 16> Ekf2dLocalizer::covariance() const {
    std::array<double, 16> flat{};
    for (int i = 0; i < 4; ++i) {
        for (int j = 0; j < 4; ++j) {
            flat[static_cast<std::size_t>(i * 4 + j)] = p_[i][j];
        }
    }
    return flat;
}

const LocalizerRegistry& default_localizer_registry() {
    static const LocalizerRegistry registry = [] {
        LocalizerRegistry built;
        built.register_factory(
            "ground_truth",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<ILocalizer> {
                return std::make_unique<GroundTruthLocalizer>(params);
            });
        built.register_factory(
            "ekf_2d",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<ILocalizer> {
                return std::make_unique<Ekf2dLocalizer>(params);
            });
        return built;
    }();
    return registry;
}

} // namespace agbot::nav
