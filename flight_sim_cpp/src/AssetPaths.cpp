#include "agbot_flight_sim/AssetPaths.hpp"

#include <cstdint>
#include <cstdlib>
#include <string>
#include <system_error>

#if defined(__APPLE__)
#include <mach-o/dyld.h>
#elif defined(__linux__)
#include <unistd.h>
#endif

#ifndef AGBOT_FLIGHT_SIM_SOURCE_DIR
#define AGBOT_FLIGHT_SIM_SOURCE_DIR "."
#endif

namespace fs = std::filesystem;

namespace agbot::flight_sim {
namespace {

fs::path env_path(const char* name) {
    const char* value = std::getenv(name);
    if (value != nullptr && value[0] != '\0') {
        return fs::path(value);
    }
    return {};
}

// A directory qualifies as an asset root if it carries either the sample
// missions or the sensor calibration profiles.
bool has_asset_marker(const fs::path& dir) {
    std::error_code ec;
    if (fs::exists(dir / "samples", ec)) {
        return true;
    }
    return fs::exists(dir / "calibration", ec);
}

fs::path compiled_source_dir() {
    return fs::path(AGBOT_FLIGHT_SIM_SOURCE_DIR);
}

} // namespace

fs::path executable_path() {
#if defined(__APPLE__)
    std::uint32_t size = 0;
    _NSGetExecutablePath(nullptr, &size);
    std::string buffer(size, '\0');
    if (_NSGetExecutablePath(buffer.data(), &size) != 0) {
        return {};
    }
    // Trim the trailing NUL the API leaves in the buffer.
    if (!buffer.empty() && buffer.back() == '\0') {
        buffer.pop_back();
    }
    std::error_code ec;
    fs::path resolved = fs::weakly_canonical(fs::path(buffer), ec);
    return ec ? fs::path(buffer) : resolved;
#elif defined(__linux__)
    std::error_code ec;
    fs::path resolved = fs::read_symlink("/proc/self/exe", ec);
    return ec ? fs::path{} : resolved;
#else
    return {};
#endif
}

fs::path sim_home() {
    if (fs::path overridden = env_path("AGBOT_FLIGHT_SIM_HOME"); !overridden.empty()) {
        return overridden;
    }

    fs::path exe = executable_path();
    if (!exe.empty()) {
        fs::path dir = exe.parent_path();
        // Walk up a few levels; assets may sit next to the binary, or in a
        // sibling `share/agbot_flight_sim` under a conventional install prefix.
        for (int level = 0; level < 5 && !dir.empty(); ++level) {
            if (has_asset_marker(dir)) {
                return dir;
            }
            fs::path share = dir / "share" / "agbot_flight_sim";
            if (has_asset_marker(share)) {
                return share;
            }
            fs::path parent = dir.parent_path();
            if (parent == dir) {
                break;
            }
            dir = parent;
        }
    }

    return compiled_source_dir();
}

fs::path sim_samples_dir() {
    return sim_home() / "samples";
}

fs::path sim_calibration_dir() {
    return sim_home() / "calibration";
}

fs::path sim_config_dir() {
    return sim_home() / "config";
}

fs::path sim_out_dir() {
    if (fs::path overridden = env_path("AGBOT_FLIGHT_SIM_OUT"); !overridden.empty()) {
        return overridden;
    }
    return sim_home() / "out";
}

} // namespace agbot::flight_sim
