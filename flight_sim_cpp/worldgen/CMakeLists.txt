add_library(agbot_worldgen
    src/Module.cpp
    src/Feature.cpp
    src/FeatureExtractor.cpp
    src/HeightResolver.cpp
    src/Polygonize.cpp
    src/SceneBridge.cpp
    src/SceneMesh.cpp
    src/SegTiler.cpp
    src/extractors/ClassicalIndex.cpp
    src/extractors/OnnxSemSeg.cpp
    src/extractors/VectorImport.cpp
)

target_include_directories(agbot_worldgen
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
)

# Header-only third-party deps (nlohmann/json, mapbox/earcut) as SYSTEM so
# -Wall -Wextra -Wpedantic stays clean for module code.
target_include_directories(agbot_worldgen SYSTEM
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../third_party
)

target_compile_features(agbot_worldgen PUBLIC cxx_std_20)
# agbot_terrain provides the self-contained PNG decoder used by the imagery
# extractors (classical_index, onnx_semseg).
target_link_libraries(agbot_worldgen
    PUBLIC agbot_config agbot_flight_sim_core
    PRIVATE agbot_third_party agbot_terrain
)

# ONNX Runtime is optional; onnx_semseg registers a reason-coded stub when it
# is absent. Discovery mirrors terrain_engine:
#   1. explicit -Donnxruntime_DIR=<prefix>/lib/cmake/onnxruntime
#   2. default CMake search paths
#   3. Homebrew prefix (macOS: `brew install onnxruntime`)
# Set -DAGBOT_WORLDGEN_WITH_ONNX=OFF to force the stub build.
option(AGBOT_WORLDGEN_WITH_ONNX "Enable the ONNX semantic segmentation extractor when available" ON)
if(AGBOT_WORLDGEN_WITH_ONNX)
    set(_agbot_worldgen_ort_hints "")
    find_program(_agbot_worldgen_brew_executable brew)
    if(_agbot_worldgen_brew_executable)
        execute_process(
            COMMAND ${_agbot_worldgen_brew_executable} --prefix onnxruntime
            OUTPUT_VARIABLE _agbot_worldgen_brew_ort_prefix
            OUTPUT_STRIP_TRAILING_WHITESPACE
            ERROR_QUIET
            RESULT_VARIABLE _agbot_worldgen_brew_ort_result
        )
        if(_agbot_worldgen_brew_ort_result EQUAL 0 AND EXISTS "${_agbot_worldgen_brew_ort_prefix}/lib/cmake/onnxruntime")
            list(APPEND _agbot_worldgen_ort_hints "${_agbot_worldgen_brew_ort_prefix}/lib/cmake/onnxruntime")
        endif()
    endif()
    find_package(onnxruntime QUIET HINTS ${_agbot_worldgen_ort_hints})
endif()
if(onnxruntime_FOUND)
    message(STATUS "agbot_worldgen: ONNX Runtime found (onnx_semseg enabled)")
    target_compile_definitions(agbot_worldgen PRIVATE AGBOT_WORLDGEN_HAS_ONNX)
    target_link_libraries(agbot_worldgen PRIVATE onnxruntime::onnxruntime)
else()
    message(STATUS "agbot_worldgen: ONNX Runtime not found (onnx_semseg stub)")
endif()

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_worldgen PRIVATE -Wall -Wextra -Wpedantic)
endif()

add_executable(agbot_worldgen_tests tests/worldgen_tests.cpp)
target_link_libraries(agbot_worldgen_tests PRIVATE agbot_worldgen)
target_compile_definitions(agbot_worldgen_tests
    PRIVATE WORLDGEN_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}"
)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_worldgen_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_worldgen_tests COMMAND agbot_worldgen_tests)

# Segmentation-lane tests: polygonization, tiling, classical indices, and the
# (compile-gated) ONNX semantic segmentation extractor.
add_executable(agbot_worldgen_seg_tests tests/segmentation_tests.cpp)
target_link_libraries(agbot_worldgen_seg_tests PRIVATE agbot_worldgen)
target_compile_definitions(agbot_worldgen_seg_tests
    PRIVATE WORLDGEN_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}"
)
if(onnxruntime_FOUND)
    target_compile_definitions(agbot_worldgen_seg_tests PRIVATE AGBOT_WORLDGEN_HAS_ONNX)
endif()
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_worldgen_seg_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_worldgen_seg_tests COMMAND agbot_worldgen_seg_tests)
