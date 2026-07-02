add_library(agbot_terrain
    src/Module.cpp
    src/Raster.cpp
    src/Png.cpp
    src/Estimators.cpp
    src/Fusion.cpp
    src/Validation.cpp
    src/TerrainPipeline.cpp
)

target_include_directories(agbot_terrain
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
)

target_compile_features(agbot_terrain PUBLIC cxx_std_20)
# agbot_flight_sim_core publicly defines AGBOT_FLIGHT_SIM_SOURCE_DIR, which
# the DEM estimator uses to locate cached tiles under out/elevation_tiles.
target_link_libraries(agbot_terrain PUBLIC agbot_config agbot_flight_sim_core)

# ONNX Runtime is optional; the mono_depth_onnx estimator registers a
# reason-coded stub when it is absent.
find_package(onnxruntime QUIET)
if(onnxruntime_FOUND)
    target_compile_definitions(agbot_terrain PRIVATE AGBOT_TERRAIN_HAS_ONNX)
    target_link_libraries(agbot_terrain PUBLIC onnxruntime::onnxruntime)
endif()

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_terrain PRIVATE -Wall -Wextra -Wpedantic)
endif()

add_executable(agbot_terrain_tests tests/terrain_tests.cpp)
target_link_libraries(agbot_terrain_tests PRIVATE agbot_terrain)
target_compile_definitions(agbot_terrain_tests PRIVATE
    AGBOT_TERRAIN_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}"
)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_terrain_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_terrain_tests COMMAND agbot_terrain_tests)
