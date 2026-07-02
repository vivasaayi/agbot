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
# reason-coded stub when it is absent. Discovery order:
#   1. explicit -Donnxruntime_DIR=<prefix>/lib/cmake/onnxruntime
#   2. default CMake search paths
#   3. Homebrew prefix (macOS: `brew install onnxruntime`)
# Set -DAGBOT_TERRAIN_WITH_ONNX=OFF to force the stub build.
option(AGBOT_TERRAIN_WITH_ONNX "Enable the ONNX Runtime mono-depth estimator when available" ON)
if(AGBOT_TERRAIN_WITH_ONNX)
    set(_agbot_ort_hints "")
    find_program(_agbot_brew_executable brew)
    if(_agbot_brew_executable)
        execute_process(
            COMMAND ${_agbot_brew_executable} --prefix onnxruntime
            OUTPUT_VARIABLE _agbot_brew_ort_prefix
            OUTPUT_STRIP_TRAILING_WHITESPACE
            ERROR_QUIET
            RESULT_VARIABLE _agbot_brew_ort_result
        )
        if(_agbot_brew_ort_result EQUAL 0 AND EXISTS "${_agbot_brew_ort_prefix}/lib/cmake/onnxruntime")
            list(APPEND _agbot_ort_hints "${_agbot_brew_ort_prefix}/lib/cmake/onnxruntime")
        endif()
    endif()
    find_package(onnxruntime QUIET HINTS ${_agbot_ort_hints})
endif()
if(onnxruntime_FOUND)
    message(STATUS "agbot_terrain: ONNX Runtime found (mono_depth_onnx enabled)")
    target_compile_definitions(agbot_terrain PRIVATE AGBOT_TERRAIN_HAS_ONNX)
    target_link_libraries(agbot_terrain PUBLIC onnxruntime::onnxruntime)
else()
    message(STATUS "agbot_terrain: ONNX Runtime not found (mono_depth_onnx stub)")
endif()

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_terrain PRIVATE -Wall -Wextra -Wpedantic)
endif()

add_executable(agbot_terrain_tests tests/terrain_tests.cpp)
target_link_libraries(agbot_terrain_tests PRIVATE agbot_terrain)
target_compile_definitions(agbot_terrain_tests PRIVATE
    AGBOT_TERRAIN_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}"
)
if(onnxruntime_FOUND)
    target_compile_definitions(agbot_terrain_tests PRIVATE AGBOT_TERRAIN_HAS_ONNX)
endif()
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_terrain_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_terrain_tests COMMAND agbot_terrain_tests)
