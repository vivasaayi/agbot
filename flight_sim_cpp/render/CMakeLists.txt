# agbot_render — modern shader-based renderer (OpenGL 4.1 Core on macOS).
# Non-GL parts (math, camera, scene types, scene file IO, demo scene) build on
# every platform; the GL backend and viewer app are APPLE-only for now. All GL
# usage sits behind agbot_render/Renderer.hpp so another backend can slot in.

add_library(agbot_render
    src/Module.cpp
    src/Camera.cpp
    src/DemoScene.cpp
    src/SceneFile.cpp
)

target_include_directories(agbot_render
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
)

target_compile_features(agbot_render PUBLIC cxx_std_20)
target_link_libraries(agbot_render PUBLIC agbot_config agbot_flight_sim_core)

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_render PRIVATE -Wall -Wextra -Wpedantic)
endif()

if(APPLE)
    find_library(RENDER_OPENGL_FRAMEWORK OpenGL REQUIRED)

    # GL backend lives in the same library, compiled only where GL is available.
    target_sources(agbot_render PRIVATE src/GlRenderer.cpp)
    target_link_libraries(agbot_render PUBLIC ${RENDER_OPENGL_FRAMEWORK})
    set_source_files_properties(src/GlRenderer.cpp PROPERTIES
        COMPILE_OPTIONS "-Wno-deprecated-declarations"
    )
endif()

# --- Tests (no GL context required) ---

add_executable(agbot_render_tests tests/render_tests.cpp)
target_link_libraries(agbot_render_tests PRIVATE agbot_render)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(agbot_render_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()
add_test(NAME agbot_render_tests COMMAND agbot_render_tests)

# --- macOS viewer app ---

if(APPLE)
    enable_language(OBJCXX)
    find_library(RENDER_COCOA_FRAMEWORK Cocoa REQUIRED)

    add_executable(agbot_world_viewer
        src/world_viewer_macos.mm
    )
    target_link_libraries(agbot_world_viewer
        PRIVATE
            agbot_render
            ${RENDER_COCOA_FRAMEWORK}
            ${RENDER_OPENGL_FRAMEWORK}
    )
    target_compile_options(agbot_world_viewer PRIVATE
        -Wall
        -Wextra
        -Wpedantic
        -Wno-deprecated-declarations
    )

    # Offscreen render sanity check: creates a CGL core-profile context,
    # renders the demo scene into an FBO, and asserts non-uniform output.
    add_test(NAME agbot_world_viewer_self_check
             COMMAND agbot_world_viewer --self-check)
endif()
