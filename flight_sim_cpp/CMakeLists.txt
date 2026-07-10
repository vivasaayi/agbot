cmake_minimum_required(VERSION 3.24)

project(AgBotFlightSim LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

add_library(agbot_flight_sim_core
    src/AssetPaths.cpp
    src/CoordinationPreview.cpp
    src/DeterministicRegression.cpp
    src/DeterministicRunner.cpp
    src/DroneSimulation.cpp
    src/FaultInjection.cpp
    src/GeoTerrain.cpp
    src/HudTelemetry.cpp
    src/LabeledSyntheticDataset.cpp
    src/LidarSimulator.cpp
    src/LocationScenario.cpp
    src/MissionLoader.cpp
    src/MissionPreview.cpp
    src/MissionValidation.cpp
    src/MultispectralCamera.cpp
    src/RayTracedCamera.cpp
    src/SafetyRules.cpp
    src/SceneSynthesis.cpp
    src/SensorModel.cpp
    src/SimulationOps.cpp
    src/TelemetryReplay.cpp
    src/TelemetryVideoStream.cpp
    src/TelemetryRecorder.cpp
    src/TraceDiff.cpp
    src/TwinBackend.cpp
    src/TwinContractV1.cpp
    src/WeatherPreset.cpp
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
target_link_libraries(agbot_flight_sim_headless PRIVATE agbot_flight_sim_core agbot_config)

add_executable(agbot_mission_bridge
    src/mission_bridge_main.cpp
)
target_link_libraries(agbot_mission_bridge PRIVATE agbot_flight_sim_core)

add_executable(agbot_sim
    src/agbot_sim_main.cpp
)
target_link_libraries(agbot_sim PRIVATE agbot_flight_sim_core)
set_target_properties(agbot_sim PROPERTIES OUTPUT_NAME agbot-sim)

# Relocatable install layout. `cmake --install` places the binary under bin/
# and its runtime assets under share/agbot_flight_sim/, which AssetPaths finds
# via executable-relative lookup — so a copied/installed tree "just runs"
# without AGBOT_FLIGHT_SIM_SOURCE_DIR. Running straight from the build tree
# keeps using the compiled source-dir fallback (dev behaviour unchanged).
include(GNUInstallDirs)
install(TARGETS agbot_flight_sim_headless RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
install(DIRECTORY samples calibration
    DESTINATION ${CMAKE_INSTALL_DATADIR}/agbot_flight_sim)
install(FILES config/flight_sim.toml
    DESTINATION ${CMAKE_INSTALL_DATADIR}/agbot_flight_sim/config)

enable_testing()
add_executable(agbot_flight_sim_tests
    tests/simulation_tests.cpp
)
target_link_libraries(agbot_flight_sim_tests PRIVATE agbot_flight_sim_core)
add_test(NAME agbot_flight_sim_tests COMMAND agbot_flight_sim_tests)
add_test(NAME agbot_flight_sim_golden_regression COMMAND agbot_sim regress)

add_executable(agbot_weather_tests tests/weather_tests.cpp)
target_link_libraries(agbot_weather_tests PRIVATE agbot_flight_sim_core agbot_vehicles)
add_test(NAME agbot_weather_tests COMMAND agbot_weather_tests)

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

# --- World-simulator modules (config, terrain, city, vehicles, navigation, render) ---

add_library(agbot_third_party INTERFACE)
target_include_directories(agbot_third_party INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/third_party)

add_subdirectory(config)
add_subdirectory(terrain_engine)
add_subdirectory(worldgen)
add_subdirectory(vehicles)
add_subdirectory(nav)
add_subdirectory(render)

add_executable(agbot_world_demo demo/world_demo_main.cpp)
# agbot_worldgen PUBLIC-links agbot_terrain + agbot_render (the world compiler
# exposes their types), so the demo only names worldgen + nav directly.
target_link_libraries(agbot_world_demo
    PRIVATE
        agbot_worldgen
        agbot_nav
)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_world_demo PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_world_demo_check COMMAND agbot_world_demo --check)
set_tests_properties(agbot_world_demo_check PROPERTIES SKIP_RETURN_CODE 77)
