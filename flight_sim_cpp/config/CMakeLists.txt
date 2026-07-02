add_library(agbot_config
    src/Params.cpp
    src/Toml.cpp
)

target_include_directories(agbot_config
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
)

target_compile_features(agbot_config PUBLIC cxx_std_20)

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_config PRIVATE -Wall -Wextra -Wpedantic)
endif()

add_executable(agbot_config_tests tests/config_tests.cpp)
target_link_libraries(agbot_config_tests PRIVATE agbot_config)
add_test(NAME agbot_config_tests COMMAND agbot_config_tests)
