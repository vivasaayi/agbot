add_library(agbot_nav
    src/Module.cpp
    src/Sensing.cpp
    src/Perception.cpp
    src/Mapping.cpp
    src/GlobalPlanner.cpp
    src/LocalPlanner.cpp
    src/Controller.cpp
    src/NavigationPipeline.cpp
)

target_include_directories(agbot_nav
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
)

target_compile_features(agbot_nav PUBLIC cxx_std_20)
target_link_libraries(agbot_nav PUBLIC agbot_config agbot_flight_sim_core agbot_vehicles)

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_nav PRIVATE -Wall -Wextra -Wpedantic)
endif()

add_executable(agbot_nav_tests tests/nav_tests.cpp)
target_link_libraries(agbot_nav_tests PRIVATE agbot_nav)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_nav_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_nav_tests COMMAND agbot_nav_tests)
