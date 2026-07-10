#pragma once

#include <filesystem>

namespace agbot::flight_sim {

// Relocatable asset resolution for the flight simulator.
//
// A built binary must be able to find its bundled assets (sample missions,
// sensor calibration profiles, default settings) and a writable scratch area
// no matter where it is copied. Resolution of the "home" root is, in order:
//
//   1. $AGBOT_FLIGHT_SIM_HOME, if set (explicit override).
//   2. A directory at or above the running executable that contains a
//      recognisable asset marker (a `samples` or `calibration` subdirectory),
//      including a `share/agbot_flight_sim` install layout. This is what makes
//      a relocated binary with assets bundled alongside it "just work".
//   3. The compile-time source directory (dev builds / running from the tree).
//
// The writable scratch/output root (`out/`) additionally honours
// $AGBOT_FLIGHT_SIM_OUT so a read-only install can redirect writes elsewhere.
std::filesystem::path sim_home();
std::filesystem::path sim_samples_dir();
std::filesystem::path sim_calibration_dir();
std::filesystem::path sim_config_dir();
std::filesystem::path sim_out_dir();

// Absolute path to the running executable, or an empty path if it cannot be
// determined on this platform. Exposed for diagnostics/tests.
std::filesystem::path executable_path();

} // namespace agbot::flight_sim
