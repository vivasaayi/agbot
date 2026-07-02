#include "agbot_vehicles/FixedWingModel.hpp"

#include <algorithm>
#include <cmath>

// 6-DOF fixed-wing dynamics, Cessna-172 class.
//
// Coefficient sources (public-domain C172 datasets): Roskam, "Airplane
// Flight Dynamics and Automatic Flight Controls"; Beard & McLain, "Small
// Unmanned Aircraft" (stall blend); UIUC aircraft coefficient database.
//
// Internal frames: NED nav frame (n = +Z_world, e = +X_world, d = -Y_world)
// and aircraft body frame (x forward, y right, z down). Attitude is a
// nav->body unit quaternion; translation state is body-frame velocity
// (u, v, w); rotation state is body rates (p, q, r). Integration is RK4 on
// the 13-dimensional state at substeps of at most kMaxSubstepS, with the
// quaternion renormalized after every substep. Air density is constant
// sea-level 1.225 kg/m^3 (no ISA lapse) - documented in FixedWingParams.

namespace agbot::vehicles {

namespace {

constexpr double kPi = 3.14159265358979323846;
constexpr int kStateSize = 13; // pos(3) + quat(4) + vel(3) + rate(3)

double clamp(double value, double lo, double hi) {
    return std::max(lo, std::min(hi, value));
}

double wrap_pi(double angle) {
    while (angle > kPi) {
        angle -= 2.0 * kPi;
    }
    while (angle < -kPi) {
        angle += 2.0 * kPi;
    }
    return angle;
}

int substep_count(double dt_s, double max_substep_s) {
    if (dt_s <= max_substep_s) {
        return 1;
    }
    return std::max(1, static_cast<int>(std::ceil(dt_s / max_substep_s - 1e-9)));
}

// ZYX (yaw-pitch-roll) Euler angles to nav->body quaternion.
void euler_to_quat(double psi, double theta, double phi, double* q) {
    const double cps = std::cos(psi * 0.5);
    const double sps = std::sin(psi * 0.5);
    const double cth = std::cos(theta * 0.5);
    const double sth = std::sin(theta * 0.5);
    const double cph = std::cos(phi * 0.5);
    const double sph = std::sin(phi * 0.5);
    q[0] = cph * cth * cps + sph * sth * sps;
    q[1] = sph * cth * cps - cph * sth * sps;
    q[2] = cph * sth * cps + sph * cth * sps;
    q[3] = cph * cth * sps - sph * sth * cps;
}

// nav->body quaternion to ZYX Euler angles.
void quat_to_euler(const double* q, double& psi, double& theta, double& phi) {
    phi = std::atan2(2.0 * (q[0] * q[1] + q[2] * q[3]),
                     1.0 - 2.0 * (q[1] * q[1] + q[2] * q[2]));
    theta = std::asin(clamp(2.0 * (q[0] * q[2] - q[3] * q[1]), -1.0, 1.0));
    psi = std::atan2(2.0 * (q[0] * q[3] + q[1] * q[2]),
                     1.0 - 2.0 * (q[2] * q[2] + q[3] * q[3]));
}

// Rotate a nav-frame vector into the body frame (C_bn * v).
void nav_to_body(const double* q, const double* vn, double* vb) {
    const double q0 = q[0];
    const double q1 = q[1];
    const double q2 = q[2];
    const double q3 = q[3];
    vb[0] = (q0 * q0 + q1 * q1 - q2 * q2 - q3 * q3) * vn[0]
        + 2.0 * (q1 * q2 + q0 * q3) * vn[1] + 2.0 * (q1 * q3 - q0 * q2) * vn[2];
    vb[1] = 2.0 * (q1 * q2 - q0 * q3) * vn[0]
        + (q0 * q0 - q1 * q1 + q2 * q2 - q3 * q3) * vn[1]
        + 2.0 * (q2 * q3 + q0 * q1) * vn[2];
    vb[2] = 2.0 * (q1 * q3 + q0 * q2) * vn[0] + 2.0 * (q2 * q3 - q0 * q1) * vn[1]
        + (q0 * q0 - q1 * q1 - q2 * q2 + q3 * q3) * vn[2];
}

// Rotate a body-frame vector into the nav frame (C_bn^T * v).
void body_to_nav(const double* q, const double* vb, double* vn) {
    const double q0 = q[0];
    const double q1 = q[1];
    const double q2 = q[2];
    const double q3 = q[3];
    vn[0] = (q0 * q0 + q1 * q1 - q2 * q2 - q3 * q3) * vb[0]
        + 2.0 * (q1 * q2 - q0 * q3) * vb[1] + 2.0 * (q1 * q3 + q0 * q2) * vb[2];
    vn[1] = 2.0 * (q1 * q2 + q0 * q3) * vb[0]
        + (q0 * q0 - q1 * q1 + q2 * q2 - q3 * q3) * vb[1]
        + 2.0 * (q2 * q3 - q0 * q1) * vb[2];
    vn[2] = 2.0 * (q1 * q3 - q0 * q2) * vb[0] + 2.0 * (q2 * q3 + q0 * q1) * vb[1]
        + (q0 * q0 - q1 * q1 - q2 * q2 + q3 * q3) * vb[2];
}

// Beard & McLain stall blend: sigma ramps 0 -> 1 around the blend center so
// the linear CL curve fades smoothly into a flat-plate falloff. The blend
// center sits slightly past the stall onset angle so that CL peaks (the CL
// "cap") near the stated stall angle without any discontinuity.
double blended_cl(const FixedWingParams& p, double alpha) {
    const double a0 = p.alpha_stall_rad + 0.02;
    const double m = p.stall_blend_rate;
    const double e_neg = std::exp(-m * (alpha - a0));
    const double e_pos = std::exp(m * (alpha + a0));
    const double sigma = (1.0 + e_neg + e_pos) / ((1.0 + e_neg) * (1.0 + e_pos));
    const double cl_linear = p.cl0 + p.cl_alpha * alpha;
    const double sign = (alpha >= 0.0) ? 1.0 : -1.0;
    const double sa = std::sin(alpha);
    const double cl_plate = 2.0 * sign * sa * sa * std::cos(alpha);
    return (1.0 - sigma) * cl_linear + sigma * cl_plate;
}

double thrust_n(const FixedWingParams& p, double throttle, double airspeed) {
    const double ratio = airspeed / p.v_max_mps;
    const double falloff = std::max(0.0, 1.0 - ratio * ratio);
    return clamp(throttle, 0.0, 1.0) * p.t_max_n * falloff;
}

} // namespace

FixedWingModel::FixedWingModel() = default;

FixedWingModel::FixedWingModel(const agbot::config::ParamTable& params) : FixedWingModel() {
    using agbot::config::double_or;
    FixedWingParams& p = params_;
    p.mass_kg = double_or(params, "mass_kg", p.mass_kg);
    p.wing_area_m2 = double_or(params, "wing_area_m2", p.wing_area_m2);
    p.wing_span_m = double_or(params, "wing_span_m", p.wing_span_m);
    p.wing_chord_m = double_or(params, "wing_chord_m", p.wing_chord_m);
    p.ixx = double_or(params, "ixx", p.ixx);
    p.iyy = double_or(params, "iyy", p.iyy);
    p.izz = double_or(params, "izz", p.izz);
    p.cl0 = double_or(params, "cl0", p.cl0);
    p.cl_alpha = double_or(params, "cl_alpha", p.cl_alpha);
    p.alpha_stall_rad = double_or(params, "alpha_stall_rad", p.alpha_stall_rad);
    p.stall_blend_rate = double_or(params, "stall_blend_rate", p.stall_blend_rate);
    p.cd0 = double_or(params, "cd0", p.cd0);
    p.k_induced = double_or(params, "k_induced", p.k_induced);
    p.cy_beta = double_or(params, "cy_beta", p.cy_beta);
    p.cm0 = double_or(params, "cm0", p.cm0);
    p.cm_alpha = double_or(params, "cm_alpha", p.cm_alpha);
    p.cm_de = double_or(params, "cm_de", p.cm_de);
    p.cm_q = double_or(params, "cm_q", p.cm_q);
    p.cl_da = double_or(params, "cl_da", p.cl_da);
    p.cl_p = double_or(params, "cl_p", p.cl_p);
    p.cl_beta = double_or(params, "cl_beta", p.cl_beta);
    p.cl_r = double_or(params, "cl_r", p.cl_r);
    p.cn_beta = double_or(params, "cn_beta", p.cn_beta);
    p.cn_dr = double_or(params, "cn_dr", p.cn_dr);
    p.cn_r = double_or(params, "cn_r", p.cn_r);
    p.cn_p = double_or(params, "cn_p", p.cn_p);
    p.elevator_max_rad = double_or(params, "elevator_max_rad", p.elevator_max_rad);
    p.aileron_max_rad = double_or(params, "aileron_max_rad", p.aileron_max_rad);
    p.rudder_max_rad = double_or(params, "rudder_max_rad", p.rudder_max_rad);
    p.t_max_n = double_or(params, "t_max_n", p.t_max_n);
    p.v_max_mps = double_or(params, "v_max_mps", p.v_max_mps);
    p.rho = double_or(params, "rho", p.rho);
    p.gravity = double_or(params, "gravity", p.gravity);
    limits_.max_speed_mps = double_or(params, "max_speed_mps", p.v_max_mps);
    limits_.max_steer_rad = double_or(params, "max_bank_rad", limits_.max_steer_rad);
}

void FixedWingModel::set_controls(const FixedWingControls& controls) {
    controls_.throttle = clamp(controls.throttle, 0.0, 1.0);
    controls_.elevator = clamp(controls.elevator, -1.0, 1.0);
    controls_.aileron = clamp(controls.aileron, -1.0, 1.0);
    controls_.rudder = clamp(controls.rudder, -1.0, 1.0);
    use_direct_controls_ = true;
}

void FixedWingModel::clear_controls() {
    use_direct_controls_ = false;
}

Vec3 FixedWingModel::body_rates() const {
    if (!internal_.has_value()) {
        return {};
    }
    return {internal_->rate[0], internal_->rate[1], internal_->rate[2]};
}

void FixedWingModel::derivatives(
    const RigidBodyState& s,
    const FixedWingControls& controls,
    double* out) const {
    const FixedWingParams& p = params_;
    const double u = s.vel[0];
    const double v = s.vel[1];
    const double w = s.vel[2];
    const double pr = s.rate[0];
    const double qr = s.rate[1];
    const double rr = s.rate[2];

    const double airspeed = std::sqrt(u * u + v * v + w * w);
    const double v_safe = std::max(airspeed, 1.0); // guards rate nondimensionalization
    const double alpha = std::atan2(w, std::max(u, 1e-6));
    const double beta =
        (airspeed > 1e-6) ? std::asin(clamp(v / airspeed, -1.0, 1.0)) : 0.0;

    const double qbar = 0.5 * p.rho * airspeed * airspeed;
    const double qbar_s = qbar * p.wing_area_m2;
    const double b_2v = p.wing_span_m / (2.0 * v_safe);
    const double c_2v = p.wing_chord_m / (2.0 * v_safe);

    const double de = clamp(controls.elevator, -1.0, 1.0) * p.elevator_max_rad;
    const double da = clamp(controls.aileron, -1.0, 1.0) * p.aileron_max_rad;
    const double dr = clamp(controls.rudder, -1.0, 1.0) * p.rudder_max_rad;

    const double cl = blended_cl(p, alpha);
    const double cd = p.cd0 + p.k_induced * cl * cl;

    // Wind->body force resolution (small-beta approximation for lift/drag).
    const double ca = std::cos(alpha);
    const double sa = std::sin(alpha);
    const double fx_aero = qbar_s * (cl * sa - cd * ca);
    const double fy_aero = qbar_s * p.cy_beta * beta;
    const double fz_aero = qbar_s * (-cl * ca - cd * sa);
    const double fx_thrust = thrust_n(p, controls.throttle, airspeed);

    // Gravity in body axes: C_bn * (0, 0, g).
    const double* q = s.quat;
    const double gx = 2.0 * (q[1] * q[3] - q[0] * q[2]) * p.gravity;
    const double gy = 2.0 * (q[2] * q[3] + q[0] * q[1]) * p.gravity;
    const double gz =
        (q[0] * q[0] - q[1] * q[1] - q[2] * q[2] + q[3] * q[3]) * p.gravity;

    // Moment coefficient buildup with standard rate nondimensionalization.
    const double cl_mom = p.cl_beta * beta + p.cl_da * da + p.cl_p * pr * b_2v
        + p.cl_r * rr * b_2v;
    const double cm_mom = p.cm0 + p.cm_alpha * alpha + p.cm_de * de + p.cm_q * qr * c_2v;
    const double cn_mom = p.cn_beta * beta + p.cn_dr * dr + p.cn_r * rr * b_2v
        + p.cn_p * pr * b_2v;
    const double mom_l = qbar_s * p.wing_span_m * cl_mom;
    const double mom_m = qbar_s * p.wing_chord_m * cm_mom;
    const double mom_n = qbar_s * p.wing_span_m * cn_mom;

    // Position derivative: nav-frame velocity.
    body_to_nav(q, s.vel, out);

    // Quaternion kinematics: q_dot = 0.5 * q (x) (0, p, q, r).
    out[3] = 0.5 * (-q[1] * pr - q[2] * qr - q[3] * rr);
    out[4] = 0.5 * (q[0] * pr + q[2] * rr - q[3] * qr);
    out[5] = 0.5 * (q[0] * qr - q[1] * rr + q[3] * pr);
    out[6] = 0.5 * (q[0] * rr + q[1] * qr - q[2] * pr);

    // Body-frame Newton (with Coriolis terms from the rotating frame).
    out[7] = rr * v - qr * w + (fx_aero + fx_thrust) / p.mass_kg + gx;
    out[8] = pr * w - rr * u + fy_aero / p.mass_kg + gy;
    out[9] = qr * u - pr * v + fz_aero / p.mass_kg + gz;

    // Euler rotational dynamics (Ixz = 0).
    out[10] = (mom_l + (p.iyy - p.izz) * qr * rr) / p.ixx;
    out[11] = (mom_m + (p.izz - p.ixx) * pr * rr) / p.iyy;
    out[12] = (mom_n + (p.ixx - p.iyy) * pr * qr) / p.izz;
}

void FixedWingModel::substep(
    RigidBodyState& state,
    const FixedWingControls& controls,
    double dt_s) {
    double y0[kStateSize];
    for (int i = 0; i < 3; ++i) {
        y0[i] = state.pos[i];
    }
    for (int i = 0; i < 4; ++i) {
        y0[3 + i] = state.quat[i];
    }
    for (int i = 0; i < 3; ++i) {
        y0[7 + i] = state.vel[i];
        y0[10 + i] = state.rate[i];
    }

    const auto eval = [&](const double* y, double* k) {
        RigidBodyState tmp;
        for (int i = 0; i < 3; ++i) {
            tmp.pos[i] = y[i];
        }
        for (int i = 0; i < 4; ++i) {
            tmp.quat[i] = y[3 + i];
        }
        for (int i = 0; i < 3; ++i) {
            tmp.vel[i] = y[7 + i];
            tmp.rate[i] = y[10 + i];
        }
        derivatives(tmp, controls, k);
    };

    double k1[kStateSize];
    double k2[kStateSize];
    double k3[kStateSize];
    double k4[kStateSize];
    double yt[kStateSize];

    eval(y0, k1);
    for (int i = 0; i < kStateSize; ++i) {
        yt[i] = y0[i] + 0.5 * dt_s * k1[i];
    }
    eval(yt, k2);
    for (int i = 0; i < kStateSize; ++i) {
        yt[i] = y0[i] + 0.5 * dt_s * k2[i];
    }
    eval(yt, k3);
    for (int i = 0; i < kStateSize; ++i) {
        yt[i] = y0[i] + dt_s * k3[i];
    }
    eval(yt, k4);

    for (int i = 0; i < kStateSize; ++i) {
        y0[i] += dt_s / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
    }

    // Renormalize the quaternion to keep the attitude on SO(3).
    const double norm = std::sqrt(y0[3] * y0[3] + y0[4] * y0[4] + y0[5] * y0[5]
                                  + y0[6] * y0[6]);
    if (norm > 1e-12) {
        for (int i = 3; i < 7; ++i) {
            y0[i] /= norm;
        }
    }

    for (int i = 0; i < 3; ++i) {
        state.pos[i] = y0[i];
    }
    for (int i = 0; i < 4; ++i) {
        state.quat[i] = y0[3 + i];
    }
    for (int i = 0; i < 3; ++i) {
        state.vel[i] = y0[7 + i];
        state.rate[i] = y0[10 + i];
    }
    state.time_s += dt_s;

    // Ground contact: this is a flight model, not a landing simulator. Clamp
    // to the ground plane (world y = 0 <=> NED d = 0) and zero any sink rate.
    if (state.pos[2] > 0.0) {
        state.pos[2] = 0.0;
        double vn[3];
        body_to_nav(state.quat, state.vel, vn);
        if (vn[2] > 0.0) {
            vn[2] = 0.0;
            nav_to_body(state.quat, vn, state.vel);
        }
    }
}

void FixedWingModel::sync_internal_from_entity(const EntityState& state) {
    RigidBodyState s;
    s.pos[0] = state.position.z;  // n
    s.pos[1] = state.position.x;  // e
    s.pos[2] = -state.position.y; // d
    const double psi = kPi / 2.0 - state.yaw_rad;
    euler_to_quat(psi, state.pitch_rad, state.roll_rad, s.quat);
    const double v_nav[3] = {state.velocity.z, state.velocity.x, -state.velocity.y};
    nav_to_body(s.quat, v_nav, s.vel);
    s.rate[0] = 0.0;
    s.rate[1] = 0.0;
    s.rate[2] = 0.0;
    s.time_s = state.time_s;
    internal_ = s;
}

EntityState FixedWingModel::entity_from_internal() const {
    EntityState out;
    const RigidBodyState& s = *internal_;
    out.position = {s.pos[1], -s.pos[2], s.pos[0]};
    double v_nav[3];
    body_to_nav(s.quat, s.vel, v_nav);
    out.velocity = {v_nav[1], -v_nav[2], v_nav[0]};
    double psi = 0.0;
    double theta = 0.0;
    double phi = 0.0;
    quat_to_euler(s.quat, psi, theta, phi);
    out.yaw_rad = wrap_pi(kPi / 2.0 - psi);
    out.pitch_rad = theta;
    out.roll_rad = phi;
    out.time_s = s.time_s;
    return out;
}

bool FixedWingModel::matches_last_output(const EntityState& state) const {
    return state.position.x == last_output_.position.x
        && state.position.y == last_output_.position.y
        && state.position.z == last_output_.position.z
        && state.velocity.x == last_output_.velocity.x
        && state.velocity.y == last_output_.velocity.y
        && state.velocity.z == last_output_.velocity.z
        && state.yaw_rad == last_output_.yaw_rad
        && state.pitch_rad == last_output_.pitch_rad
        && state.roll_rad == last_output_.roll_rad
        && state.time_s == last_output_.time_s;
}

EntityState FixedWingModel::set_initial_trim(
    double altitude_m,
    double airspeed_mps,
    double heading_rad,
    double x_m,
    double z_m) {
    const FixedWingParams& p = params_;
    const double qbar_s =
        0.5 * p.rho * airspeed_mps * airspeed_mps * p.wing_area_m2;
    const double weight = p.mass_kg * p.gravity;
    const double cl_req = weight / std::max(qbar_s, 1e-6);
    const double alpha = (cl_req - p.cl0) / p.cl_alpha;
    const double cd = p.cd0 + p.k_induced * cl_req * cl_req;
    const double drag = qbar_s * cd;
    const double thrust_req = drag / std::max(std::cos(alpha), 0.5);
    const double avail = thrust_n(p, 1.0, airspeed_mps);
    const double de_rad = -(p.cm0 + p.cm_alpha * alpha) / p.cm_de;

    trim_controls_.throttle = clamp(thrust_req / std::max(avail, 1e-6), 0.0, 1.0);
    trim_controls_.elevator = clamp(de_rad / p.elevator_max_rad, -1.0, 1.0);
    trim_controls_.aileron = 0.0;
    trim_controls_.rudder = 0.0;
    set_controls(trim_controls_);

    RigidBodyState s;
    s.pos[0] = z_m;
    s.pos[1] = x_m;
    s.pos[2] = -altitude_m;
    const double psi = kPi / 2.0 - heading_rad;
    // Level flight path: pitch equals angle of attack.
    euler_to_quat(psi, alpha, 0.0, s.quat);
    s.vel[0] = airspeed_mps * std::cos(alpha);
    s.vel[1] = 0.0;
    s.vel[2] = airspeed_mps * std::sin(alpha);
    s.rate[0] = 0.0;
    s.rate[1] = 0.0;
    s.rate[2] = 0.0;
    s.time_s = 0.0;
    internal_ = s;
    last_output_ = entity_from_internal();
    return last_output_;
}

EntityState FixedWingModel::step(const EntityState& state, const Actuation& input, double dt_s) {
    if (!(dt_s > 0.0)) {
        return state;
    }
    if (!internal_.has_value() || !matches_last_output(state)) {
        sync_internal_from_entity(state);
    }

    FixedWingControls controls = controls_;
    if (!use_direct_controls_) {
        // Generic Actuation compatibility: throttle -> throttle,
        // steer_rad -> aileron. Elevator/rudder keep their last values.
        controls.throttle = clamp(input.throttle, 0.0, 1.0);
        controls.aileron = clamp(input.steer_rad, -1.0, 1.0);
    }

    const int steps = substep_count(dt_s, kMaxSubstepS);
    const double substep_s = dt_s / static_cast<double>(steps);
    for (int i = 0; i < steps; ++i) {
        substep(*internal_, controls, substep_s);
    }

    // Refresh the aero observables from the final state.
    const RigidBodyState& s = *internal_;
    const double airspeed = std::sqrt(s.vel[0] * s.vel[0] + s.vel[1] * s.vel[1]
                                      + s.vel[2] * s.vel[2]);
    aero_debug_.airspeed_mps = airspeed;
    aero_debug_.alpha_rad = std::atan2(s.vel[2], std::max(s.vel[0], 1e-6));
    aero_debug_.beta_rad =
        (airspeed > 1e-6) ? std::asin(clamp(s.vel[1] / airspeed, -1.0, 1.0)) : 0.0;
    aero_debug_.cl = blended_cl(params_, aero_debug_.alpha_rad);
    aero_debug_.cd = params_.cd0 + params_.k_induced * aero_debug_.cl * aero_debug_.cl;

    last_output_ = entity_from_internal();
    return last_output_;
}

} // namespace agbot::vehicles
