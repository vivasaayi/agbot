#include "agbot_vehicles/FlightRecorder.hpp"

#include <cmath>
#include <sstream>

namespace agbot::vehicles {

FlightRecord capture_flight_record(const FixedWingModel& model, const EntityState& state) {
    const FixedWingAeroDebug& aero = model.aero_debug();
    const Vec3 rates = model.body_rates();
    FlightRecord r;
    r.time_s = state.time_s;
    r.altitudeMsl_m = state.position.y;
    r.posEast_m = state.position.x;
    r.posNorth_m = state.position.z;
    r.trueAirspeed_mps = aero.airspeed_mps;
    r.groundSpeed_mps =
        std::sqrt(state.velocity.x * state.velocity.x + state.velocity.z * state.velocity.z);
    r.climbRate_mps = state.velocity.y;
    r.angleOfAttack_rad = aero.alpha_rad;
    r.angleOfSideslip_rad = aero.beta_rad;
    r.eulerAngle_phi_rad = state.roll_rad;
    r.eulerAngle_theta_rad = state.pitch_rad;
    r.eulerAngle_psi_rad = state.yaw_rad;
    r.bodyAngularRate_p_radps = rates.x;
    r.bodyAngularRate_q_radps = rates.y;
    r.bodyAngularRate_r_radps = rates.z;
    r.coefficientOfLift = aero.cl;
    r.coefficientOfDrag = aero.cd;
    return r;
}

std::string flight_record_csv_header() {
    return "time,altitudeMsl,posEast,posNorth,trueAirspeed,groundSpeed,altitudeRate,"
           "angleOfAttack,angleOfSideslip,eulerAngle_phi,eulerAngle_theta,eulerAngle_psi,"
           "bodyAngularRate_p,bodyAngularRate_q,bodyAngularRate_r,coefficientOfLift,"
           "coefficientOfDrag";
}

std::string flight_record_csv_row(const FlightRecord& r) {
    std::ostringstream out;
    out.setf(std::ios::fixed);
    out.precision(6);
    out << r.time_s << ',' << r.altitudeMsl_m << ',' << r.posEast_m << ',' << r.posNorth_m << ','
        << r.trueAirspeed_mps << ',' << r.groundSpeed_mps << ',' << r.climbRate_mps << ','
        << r.angleOfAttack_rad << ',' << r.angleOfSideslip_rad << ',' << r.eulerAngle_phi_rad << ','
        << r.eulerAngle_theta_rad << ',' << r.eulerAngle_psi_rad << ',' << r.bodyAngularRate_p_radps
        << ',' << r.bodyAngularRate_q_radps << ',' << r.bodyAngularRate_r_radps << ','
        << r.coefficientOfLift << ',' << r.coefficientOfDrag;
    return out.str();
}

std::uint64_t flight_log_hash(const std::vector<FlightRecord>& log) {
    std::uint64_t acc = 1469598103934665603ULL;
    const auto fold = [&acc](std::int64_t value) {
        const auto bits = static_cast<std::uint64_t>(value);
        for (int i = 0; i < 8; ++i) {
            acc ^= (bits >> (i * 8)) & 0xFFu;
            acc *= 1099511628211ULL;
        }
    };
    // Quantize positions/speeds to mm, angles/rates to mrad; deterministic and
    // insensitive to sub-quantum float noise.
    const auto q_mm = [](double m) { return static_cast<std::int64_t>(std::llround(m * 1000.0)); };
    const auto q_mr = [](double r) { return static_cast<std::int64_t>(std::llround(r * 1000.0)); };
    for (const FlightRecord& r : log) {
        fold(q_mm(r.time_s));
        fold(q_mm(r.altitudeMsl_m));
        fold(q_mm(r.posEast_m));
        fold(q_mm(r.posNorth_m));
        fold(q_mm(r.trueAirspeed_mps));
        fold(q_mm(r.groundSpeed_mps));
        fold(q_mm(r.climbRate_mps));
        fold(q_mr(r.angleOfAttack_rad));
        fold(q_mr(r.angleOfSideslip_rad));
        fold(q_mr(r.eulerAngle_phi_rad));
        fold(q_mr(r.eulerAngle_theta_rad));
        fold(q_mr(r.eulerAngle_psi_rad));
        fold(q_mr(r.bodyAngularRate_p_radps));
        fold(q_mr(r.bodyAngularRate_q_radps));
        fold(q_mr(r.bodyAngularRate_r_radps));
        fold(q_mr(r.coefficientOfLift));
        fold(q_mr(r.coefficientOfDrag));
    }
    return acc;
}

} // namespace agbot::vehicles
