add_library(agbot_vehicles
    src/Module.cpp
    src/VehicleTypes.cpp
    src/KinematicBicycleModel.cpp
    src/MultirotorModel.cpp
    src/FixedWingModel.cpp
    src/FixedWingAutopilot.cpp
    src/VehicleRegistry.cpp
)

target_include_directories(agbot_vehicles
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
)

target_compile_features(agbot_vehicles PUBLIC cxx_std_20)
target_link_libraries(agbot_vehicles PUBLIC agbot_config agbot_flight_sim_core)

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_vehicles PRIVATE -Wall -Wextra -Wpedantic)
endif()

add_executable(agbot_vehicle_tests tests/vehicle_tests.cpp)
target_link_libraries(agbot_vehicle_tests PRIVATE agbot_vehicles)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_vehicle_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_vehicle_tests COMMAND agbot_vehicle_tests)

add_executable(agbot_cessna_tests tests/cessna_tests.cpp)
target_link_libraries(agbot_cessna_tests PRIVATE agbot_vehicles)
target_compile_definitions(agbot_cessna_tests
    PRIVATE AGBOT_VEHICLES_CONFIG_DIR="${CMAKE_CURRENT_SOURCE_DIR}/configs")
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_cessna_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_cessna_tests COMMAND agbot_cessna_tests)
