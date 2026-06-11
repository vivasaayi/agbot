cmake_minimum_required(VERSION 3.24)

project(AgBotFlightSim LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

add_library(agbot_flight_sim_core
    src/DeterministicRunner.cpp
    src/DroneSimulation.cpp
    src/FaultInjection.cpp
    src/GeoTerrain.cpp
    src/MissionLoader.cpp
    src/SafetyRules.cpp
    src/SensorModel.cpp
    src/SimulationOps.cpp
    src/TelemetryReplay.cpp
    src/TelemetryRecorder.cpp
    src/TraceDiff.cpp
    src/TwinContractV1.cpp
)

target_include_directories(agbot_flight_sim_core
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
)

target_compile_features(agbot_flight_sim_core PUBLIC cxx_std_20)
target_compile_definitions(agbot_flight_sim_core
    PUBLIC
        AGBOT_FLIGHT_SIM_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}"
)

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_flight_sim_core PRIVATE
        -Wall
        -Wextra
        -Wpedantic
    )
endif()

add_executable(agbot_flight_sim_headless
    src/headless_main.cpp
)
target_link_libraries(agbot_flight_sim_headless PRIVATE agbot_flight_sim_core)

add_executable(agbot_mission_bridge
    src/mission_bridge_main.cpp
)
target_link_libraries(agbot_mission_bridge PRIVATE agbot_flight_sim_core)

add_executable(agbot_sim
    src/agbot_sim_main.cpp
)
target_link_libraries(agbot_sim PRIVATE agbot_flight_sim_core)
set_target_properties(agbot_sim PROPERTIES OUTPUT_NAME agbot-sim)

enable_testing()
add_executable(agbot_flight_sim_tests
    tests/simulation_tests.cpp
)
target_link_libraries(agbot_flight_sim_tests PRIVATE agbot_flight_sim_core)
add_test(NAME agbot_flight_sim_tests COMMAND agbot_flight_sim_tests)

if(APPLE)
    enable_language(OBJCXX)

    find_library(COCOA_FRAMEWORK Cocoa REQUIRED)
    find_library(OPENGL_FRAMEWORK OpenGL REQUIRED)

    add_executable(agbot_flight_sim_viewer
        src/macos_opengl_viewer.mm
    )
    target_link_libraries(agbot_flight_sim_viewer
        PRIVATE
            agbot_flight_sim_core
            ${COCOA_FRAMEWORK}
            ${OPENGL_FRAMEWORK}
    )
    target_compile_options(agbot_flight_sim_viewer PRIVATE
        -Wno-deprecated-declarations
    )
endif()
