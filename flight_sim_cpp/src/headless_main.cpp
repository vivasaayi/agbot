#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_flight_sim/TelemetryRecorder.hpp"

#include <filesystem>
#include <iostream>
#include <stdexcept>
#include <string>

using agbot::flight_sim::DroneSimulation;
using agbot::flight_sim::MissionLoader;
using agbot::flight_sim::TelemetryRecorder;
using agbot::flight_sim::default_sample_mission_path;
using agbot::flight_sim::to_string;

namespace {

struct Args {
    std::filesystem::path mission_path = default_sample_mission_path();
    std::filesystem::path output_path = std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "telemetry.jsonl";
    double max_time_s = 600.0;
};

Args parse_args(int argc, char** argv) {
    Args args;
    for (int index = 1; index < argc; ++index) {
        const std::string current = argv[index];
        if (current == "--mission" && index + 1 < argc) {
            args.mission_path = argv[++index];
        } else if (current == "--output" && index + 1 < argc) {
            args.output_path = argv[++index];
        } else if (current == "--max-time" && index + 1 < argc) {
            args.max_time_s = std::stod(argv[++index]);
        } else if (current == "--help" || current == "-h") {
            std::cout << "Usage: agbot_flight_sim_headless [--mission path] [--output path] [--max-time seconds]\n";
            std::exit(0);
        } else {
            throw std::runtime_error("Unknown argument: " + current);
        }
    }
    return args;
}

} // namespace

int main(int argc, char** argv) {
    try {
        const Args args = parse_args(argc, argv);
        auto mission = MissionLoader::load_from_file(args.mission_path);
        DroneSimulation simulation(std::move(mission));
        TelemetryRecorder recorder(args.output_path);

        constexpr double dt_s = 1.0 / 60.0;
        constexpr double record_interval_s = 0.25;
        double next_record_s = 0.0;

        while (!simulation.is_complete() && simulation.state().mission_time_s < args.max_time_s) {
            simulation.step(dt_s);
            if (simulation.state().mission_time_s >= next_record_s) {
                recorder.write_sample(simulation.state());
                next_record_s += record_interval_s;
            }
        }
        recorder.write_sample(simulation.state());
        recorder.close();

        const auto& state = simulation.state();
        std::cout << "Mission: " << simulation.mission().name << "\n"
                  << "Status: " << to_string(state.mode) << "\n"
                  << "Time: " << state.mission_time_s << "s\n"
                  << "Final position: " << state.position << "\n"
                  << "Battery: " << state.battery_percent << "%\n"
                  << "Telemetry: " << args.output_path << "\n";

        return simulation.is_complete() ? 0 : 2;
    } catch (const std::exception& error) {
        std::cerr << "agbot_flight_sim_headless: " << error.what() << "\n";
        return 1;
    }
}
