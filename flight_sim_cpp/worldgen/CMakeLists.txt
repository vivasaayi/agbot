add_library(agbot_worldgen
    src/Module.cpp
    src/Feature.cpp
    src/FeatureExtractor.cpp
    src/HeightResolver.cpp
    src/SceneBridge.cpp
    src/SceneMesh.cpp
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
target_link_libraries(agbot_worldgen
    PUBLIC agbot_config agbot_flight_sim_core
    PRIVATE agbot_third_party
)

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
