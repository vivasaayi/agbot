#pragma once

#include "agbot_render/RenderScene.hpp"

namespace agbot::render {

// Deterministic procedural demo scene so the viewer is demoable standalone:
// - a 200x200-vertex heightfield mesh built from value noise (mesh 0)
// - a grid of ~200 extruded boxes as a fake city (mesh 1)
// - a checkerboard-textured quad exercising the textured pipeline
// - a few colored markers and a fixed sun direction.
RenderScene build_demo_scene();

// Deterministic lattice value noise in [0, 1]. Exposed for tests.
float value_noise_2d(float x, float y, std::uint32_t seed);

} // namespace agbot::render
