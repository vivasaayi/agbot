#include "agbot_flight_sim/TraceDiff.hpp"

#include <filesystem>
#include <fstream>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <string>

namespace {

std::string read_all(const std::filesystem::path& path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        throw std::runtime_error("unable to open trace: " + path.string());
    }
    std::ostringstream buffer;
    buffer << file.rdbuf();
    return buffer.str();
}

void print_usage() {
    std::cout << "Usage: agbot-sim diff <trace-a.jsonl> <trace-b.jsonl>\n";
}

} // namespace

int main(int argc, char** argv) {
    try {
        if (argc == 2 && (std::string(argv[1]) == "--help" || std::string(argv[1]) == "-h")) {
            print_usage();
            return 0;
        }
        if (argc != 4 || std::string(argv[1]) != "diff") {
            print_usage();
            return 2;
        }

        const std::string left = read_all(argv[2]);
        const std::string right = read_all(argv[3]);
        const auto diff = agbot::flight_sim::diff_trace_text(left, right);
        std::cout << diff.message << "\n";
        return diff.identical ? 0 : 1;
    } catch (const std::exception& error) {
        std::cerr << "agbot-sim: " << error.what() << "\n";
        return 2;
    }
}
