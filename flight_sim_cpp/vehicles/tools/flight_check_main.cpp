// Runnable fixed-wing check-case tool (M7 batch 1).
//
//   agbot_flight_check                 run the standard cases, print a summary
//   agbot_flight_check --out <dir>     also write per-case S-119 CSV logs
//   agbot_flight_check --check         assert acceptance + deterministic replay
//
// The Gate 6 assertions live in agbot_flight_checkcase_tests; this tool makes
// the same cases runnable and their S-119 trajectories inspectable (CSV).

#include "agbot_vehicles/FixedWingModel.hpp"
#include "agbot_vehicles/FlightCheckCase.hpp"
#include "agbot_vehicles/FlightRecorder.hpp"

#include <cmath>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <string>

namespace {

using namespace agbot::vehicles;

constexpr double kPi = 3.14159265358979323846;
double deg(double r) { return r * 180.0 / kPi; }

void write_csv(const std::filesystem::path& path, const FlightCheckResult& r) {
    std::ofstream out(path);
    if (!out) {
        std::fprintf(stderr, "  (could not write %s)\n", path.string().c_str());
        return;
    }
    out << flight_record_csv_header() << "\n";
    for (const FlightRecord& rec : r.log) {
        out << flight_record_csv_row(rec) << "\n";
    }
}

void print_summary(const FlightCheckResult& r) {
    std::printf(
        "  %-16s alt %.0f->%.0f m  V[%.1f,%.1f]  maxAoA %5.1f deg  maxBank %5.1f deg  "
        "dHdg %6.1f deg  minAlt %.0f  hash %llu\n",
        r.name.c_str(), r.initial.altitudeMsl_m, r.final_record.altitudeMsl_m,
        r.min_true_airspeed_mps, r.max_true_airspeed_mps, deg(r.max_angle_of_attack_rad),
        deg(r.max_abs_bank_rad), deg(r.heading_change_rad), r.min_altitude_m,
        static_cast<unsigned long long>(r.log_hash));
}

// Per-case acceptance, mirroring the Gate 6 predicates.
bool accept(const FlightCheckResult& r) {
    const double kDeg = kPi / 180.0;
    if (r.name == "trimmed_cruise") {
        return r.max_altitude_dev_m < 20.0 &&
            std::abs(r.max_true_airspeed_mps - r.initial.trueAirspeed_mps) < 3.0 &&
            std::abs(r.heading_change_rad) < 5.0 * kDeg;
    }
    if (r.name == "banked_turn") {
        return r.max_abs_bank_rad > 15.0 * kDeg && std::abs(r.heading_change_rad) > 20.0 * kDeg;
    }
    if (r.name == "climb") {
        return r.final_record.altitudeMsl_m > r.initial.altitudeMsl_m + 30.0;
    }
    if (r.name == "descent") {
        return r.final_record.altitudeMsl_m < r.initial.altitudeMsl_m - 30.0;
    }
    if (r.name == "crosswind") {
        return r.max_abs_sideslip_rad > 2.0 * kDeg;
    }
    if (r.name == "stall_recovery") {
        FixedWingModel ref;
        return r.max_angle_of_attack_rad > ref.params().alpha_stall_rad &&
            r.min_altitude_m < r.initial.altitudeMsl_m - 50.0 &&
            r.final_record.trueAirspeed_mps > r.min_true_airspeed_mps + 3.0;
    }
    return true;
}

} // namespace

int main(int argc, char** argv) {
    bool check_mode = false;
    std::string out_dir;
    for (int i = 1; i < argc; ++i) {
        const std::string a = argv[i];
        if (a == "--check") {
            check_mode = true;
        } else if (a == "--out" && i + 1 < argc) {
            out_dir = argv[++i];
        }
    }

    if (!out_dir.empty()) {
        std::error_code ec;
        std::filesystem::create_directories(out_dir, ec);
    }

    const auto cases = standard_check_cases();
    std::printf("fixed-wing check cases (%zu):\n", cases.size());
    int failures = 0;
    for (const auto& c : cases) {
        const FlightCheckResult r = run_flight_check_case(c);
        print_summary(r);
        if (!out_dir.empty()) {
            write_csv(std::filesystem::path(out_dir) / (r.name + ".csv"), r);
        }
        if (check_mode) {
            const FlightCheckResult again = run_flight_check_case(c);
            const bool deterministic = again.log_hash == r.log_hash && r.log_hash != 0;
            const bool ok = accept(r);
            std::printf("    %s deterministic-replay, %s acceptance\n",
                        deterministic ? "PASS" : "FAIL", ok ? "PASS" : "FAIL");
            failures += (deterministic && ok) ? 0 : 1;
        }
    }
    if (!out_dir.empty()) {
        std::printf("wrote S-119 CSV logs to %s/\n", out_dir.c_str());
    }
    if (check_mode) {
        if (failures != 0) {
            std::printf("%d case(s) failed\n", failures);
            return 1;
        }
        std::printf("all flight check cases passed\n");
    }
    return 0;
}
