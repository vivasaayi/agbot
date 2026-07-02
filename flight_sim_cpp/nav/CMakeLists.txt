add_library(agbot_nav
    src/Module.cpp
    src/Sensing.cpp
    src/Perception.cpp
    src/Mapping.cpp
    src/GlobalPlanner.cpp
    src/HybridAStarPlanner.cpp
    src/RoadGraphPlanner.cpp
    src/LocalPlanner.cpp
    src/MppiPlanner.cpp
    src/DynamicAgents.cpp
    src/Tracking.cpp
    src/Localization.cpp
    src/Controller.cpp
    src/NavigationPipeline.cpp
    src/AerialPlanner.cpp
)

target_include_directories(agbot_nav
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
)

target_compile_features(agbot_nav PUBLIC cxx_std_20)
# agbot_worldgen supplies the RoadNetwork consumed by the road_graph global
# planner (strategic street routing). Dependency direction is nav -> worldgen;
# worldgen never links nav.
target_link_libraries(agbot_nav
    PUBLIC agbot_config agbot_flight_sim_core agbot_vehicles agbot_worldgen
)

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_nav PRIVATE -Wall -Wextra -Wpedantic)
endif()

add_executable(agbot_nav_tests tests/nav_tests.cpp)
target_link_libraries(agbot_nav_tests PRIVATE agbot_nav)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_nav_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_nav_tests COMMAND agbot_nav_tests)

add_executable(agbot_nav_tests2 tests/nav_tests2.cpp)
target_link_libraries(agbot_nav_tests2 PRIVATE agbot_nav)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_nav_tests2 PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_nav_tests2 COMMAND agbot_nav_tests2)

add_executable(agbot_aerial_tests tests/aerial_tests.cpp)
target_link_libraries(agbot_aerial_tests PRIVATE agbot_nav)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_aerial_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_aerial_tests COMMAND agbot_aerial_tests)

add_executable(agbot_nav_road_tests tests/road_planner_tests.cpp)
target_link_libraries(agbot_nav_road_tests PRIVATE agbot_nav)
target_compile_definitions(agbot_nav_road_tests
    PRIVATE FLIGHT_SIM_ROOT_DIR="${CMAKE_CURRENT_SOURCE_DIR}/.."
)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_nav_road_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_nav_road_tests COMMAND agbot_nav_road_tests)

add_executable(agbot_nav_dynamic_tests tests/dynamic_tests.cpp)
target_link_libraries(agbot_nav_dynamic_tests PRIVATE agbot_nav)
target_compile_definitions(agbot_nav_dynamic_tests
    PRIVATE FLIGHT_SIM_ROOT_DIR="${CMAKE_CURRENT_SOURCE_DIR}/.."
)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_nav_dynamic_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_nav_dynamic_tests COMMAND agbot_nav_dynamic_tests)
