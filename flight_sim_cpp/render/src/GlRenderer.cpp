#include "agbot_render/GlRenderer.hpp"

#include "agbot_render/Mat4.hpp"

#ifdef __APPLE__
#define GL_SILENCE_DEPRECATION 1
#include <OpenGL/gl3.h>
#else
#include <GL/glcorearb.h>
#endif

#include <cstddef>
#include <iostream>

namespace agbot::render {

namespace {

const char* kVertexShader = R"glsl(#version 410 core
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec4 a_color;

uniform mat4 u_mvp;
uniform mat4 u_model;

out vec3 v_world_pos;
out vec3 v_normal;
out vec4 v_color;

void main() {
    vec4 world = u_model * vec4(a_position, 1.0);
    v_world_pos = world.xyz;
    v_normal = mat3(u_model) * a_normal;
    v_color = a_color;
    gl_Position = u_mvp * vec4(a_position, 1.0);
}
)glsl";

const char* kFragmentShader = R"glsl(#version 410 core
in vec3 v_world_pos;
in vec3 v_normal;
in vec4 v_color;

uniform vec3 u_sun_dir;   // direction the light travels (toward the scene)
uniform vec3 u_view_pos;

out vec4 frag_color;

void main() {
    vec3 n = normalize(v_normal);
    vec3 to_light = normalize(-u_sun_dir);
    float diffuse = max(dot(n, to_light), 0.0);

    vec3 view_dir = normalize(u_view_pos - v_world_pos);
    vec3 half_vec = normalize(to_light + view_dir);
    float specular = pow(max(dot(n, half_vec), 0.0), 32.0) * 0.15;

    float ambient = 0.28;
    vec3 lit = v_color.rgb * (ambient + diffuse * 0.85) + vec3(specular);
    frag_color = vec4(lit, v_color.a);
}
)glsl";

GLuint compile_shader(GLenum type, const char* source, std::string& error_out) {
    GLuint shader = glCreateShader(type);
    glShaderSource(shader, 1, &source, nullptr);
    glCompileShader(shader);

    GLint ok = GL_FALSE;
    glGetShaderiv(shader, GL_COMPILE_STATUS, &ok);
    if (ok != GL_TRUE) {
        char log[2048] = {};
        GLsizei length = 0;
        glGetShaderInfoLog(shader, sizeof(log), &length, log);
        error_out = std::string("shader compile failed: ") + std::string(log, static_cast<std::size_t>(length));
        std::cerr << "[agbot_render] " << error_out << "\n";
        glDeleteShader(shader);
        return 0;
    }
    return shader;
}

void upload_mesh_buffers(GLuint vao, GLuint vbo, GLuint ebo,
                         const std::vector<RenderVertex>& vertices,
                         const std::vector<std::uint32_t>& indices) {
    glBindVertexArray(vao);

    glBindBuffer(GL_ARRAY_BUFFER, vbo);
    glBufferData(GL_ARRAY_BUFFER,
                 static_cast<GLsizeiptr>(vertices.size() * sizeof(RenderVertex)),
                 vertices.data(), GL_STATIC_DRAW);

    glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, ebo);
    glBufferData(GL_ELEMENT_ARRAY_BUFFER,
                 static_cast<GLsizeiptr>(indices.size() * sizeof(std::uint32_t)),
                 indices.data(), GL_STATIC_DRAW);

    const GLsizei stride = static_cast<GLsizei>(sizeof(RenderVertex));
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, stride,
                          reinterpret_cast<const void*>(offsetof(RenderVertex, px)));
    glEnableVertexAttribArray(1);
    glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE, stride,
                          reinterpret_cast<const void*>(offsetof(RenderVertex, nx)));
    glEnableVertexAttribArray(2);
    glVertexAttribPointer(2, 4, GL_FLOAT, GL_FALSE, stride,
                          reinterpret_cast<const void*>(offsetof(RenderVertex, r)));

    glBindVertexArray(0);
}

// Markers are few, so the simple robust path is to bake each marker as its own
// tiny colored cube mesh in world space at upload time (instancing can replace
// this later without changing the Renderer interface).
RenderMesh build_marker_cube(const RenderScene::Marker& marker) {
    RenderMesh mesh;
    const float h = marker.size_m * 0.5F;
    const float cx = marker.x;
    const float cy = marker.y;
    const float cz = marker.z;

    const float px[8] = {cx - h, cx + h, cx + h, cx - h, cx - h, cx + h, cx + h, cx - h};
    const float py[8] = {cy - h, cy - h, cy + h, cy + h, cy - h, cy - h, cy + h, cy + h};
    const float pz[8] = {cz - h, cz - h, cz - h, cz - h, cz + h, cz + h, cz + h, cz + h};

    for (int i = 0; i < 8; ++i) {
        RenderVertex v;
        v.px = px[i];
        v.py = py[i];
        v.pz = pz[i];
        // Approximate normal: from cube center outward (good enough for markers).
        const Vec3f n = vec3_normalize(Vec3f{px[i] - cx, py[i] - cy, pz[i] - cz});
        v.nx = n.x;
        v.ny = n.y;
        v.nz = n.z;
        v.r = marker.r;
        v.g = marker.g;
        v.b = marker.b;
        v.a = 1.0F;
        mesh.vertices.push_back(v);
    }

    const std::uint32_t idx[36] = {
        0, 1, 2, 0, 2, 3, // back
        4, 6, 5, 4, 7, 6, // front
        0, 3, 7, 0, 7, 4, // left
        1, 5, 6, 1, 6, 2, // right
        3, 2, 6, 3, 6, 7, // top
        0, 4, 5, 0, 5, 1, // bottom
    };
    mesh.indices.assign(idx, idx + 36);
    return mesh;
}

} // namespace

GlRenderer::~GlRenderer() {
    shutdown();
}

bool GlRenderer::build_shader_program() {
    std::string error;
    const GLuint vs = compile_shader(GL_VERTEX_SHADER, kVertexShader, error);
    if (vs == 0) {
        last_error_ = error;
        return false;
    }
    const GLuint fs = compile_shader(GL_FRAGMENT_SHADER, kFragmentShader, error);
    if (fs == 0) {
        glDeleteShader(vs);
        last_error_ = error;
        return false;
    }

    program_ = glCreateProgram();
    glAttachShader(program_, vs);
    glAttachShader(program_, fs);
    glLinkProgram(program_);
    glDeleteShader(vs);
    glDeleteShader(fs);

    GLint ok = GL_FALSE;
    glGetProgramiv(program_, GL_LINK_STATUS, &ok);
    if (ok != GL_TRUE) {
        char log[2048] = {};
        GLsizei length = 0;
        glGetProgramInfoLog(program_, sizeof(log), &length, log);
        last_error_ = std::string("program link failed: ") +
                      std::string(log, static_cast<std::size_t>(length));
        std::cerr << "[agbot_render] " << last_error_ << "\n";
        glDeleteProgram(program_);
        program_ = 0;
        return false;
    }

    loc_mvp_ = glGetUniformLocation(program_, "u_mvp");
    loc_model_ = glGetUniformLocation(program_, "u_model");
    loc_sun_dir_ = glGetUniformLocation(program_, "u_sun_dir");
    loc_view_pos_ = glGetUniformLocation(program_, "u_view_pos");
    return true;
}

bool GlRenderer::init(int width_px, int height_px) {
    if (initialized_) {
        return true;
    }
    width_px_ = width_px;
    height_px_ = height_px;

    const GLubyte* version = glGetString(GL_VERSION);
    if (version != nullptr) {
        std::cout << "[agbot_render] GL_VERSION: " << reinterpret_cast<const char*>(version)
                  << "\n";
    }

    if (!build_shader_program()) {
        return false;
    }

    glEnable(GL_DEPTH_TEST);
    glDepthFunc(GL_LESS);
    glEnable(GL_CULL_FACE);
    glCullFace(GL_BACK);
    glClearColor(0.53F, 0.71F, 0.92F, 1.0F);
    glViewport(0, 0, width_px_, height_px_);

    initialized_ = true;
    return true;
}

void GlRenderer::destroy_scene_buffers() {
    for (GpuMesh& mesh : meshes_) {
        if (mesh.vao != 0) {
            glDeleteVertexArrays(1, &mesh.vao);
        }
        if (mesh.vbo != 0) {
            glDeleteBuffers(1, &mesh.vbo);
        }
        if (mesh.ebo != 0) {
            glDeleteBuffers(1, &mesh.ebo);
        }
    }
    meshes_.clear();
}

void GlRenderer::shutdown() {
    if (!initialized_) {
        return;
    }
    destroy_scene_buffers();
    if (program_ != 0) {
        glDeleteProgram(program_);
        program_ = 0;
    }
    initialized_ = false;
}

bool GlRenderer::uploadScene(const RenderScene& scene) {
    if (!initialized_) {
        last_error_ = "uploadScene called before init";
        return false;
    }
    destroy_scene_buffers();

    auto upload_one = [this](const RenderMesh& mesh) {
        GpuMesh gpu;
        glGenVertexArrays(1, &gpu.vao);
        glGenBuffers(1, &gpu.vbo);
        glGenBuffers(1, &gpu.ebo);
        upload_mesh_buffers(gpu.vao, gpu.vbo, gpu.ebo, mesh.vertices, mesh.indices);
        gpu.index_count = static_cast<std::int32_t>(mesh.indices.size());
        meshes_.push_back(gpu);
    };

    for (const RenderMesh& mesh : scene.static_meshes) {
        if (mesh.vertices.empty() || mesh.indices.empty()) {
            continue;
        }
        upload_one(mesh);
    }

    // Markers: baked as small world-space cubes (simple path; instancing later).
    for (const RenderScene::Marker& marker : scene.markers) {
        upload_one(build_marker_cube(marker));
    }

    markers_ = scene.markers;
    sun_dir_[0] = scene.sun_dir[0];
    sun_dir_[1] = scene.sun_dir[1];
    sun_dir_[2] = scene.sun_dir[2];

    const GLenum err = glGetError();
    if (err != GL_NO_ERROR) {
        last_error_ = "GL error during uploadScene: " + std::to_string(err);
        return false;
    }
    return true;
}

void GlRenderer::resize(int width_px, int height_px) {
    width_px_ = width_px;
    height_px_ = height_px;
    if (initialized_) {
        glViewport(0, 0, width_px_, height_px_);
    }
}

void GlRenderer::drawFrame(const Camera& camera) {
    if (!initialized_ || program_ == 0) {
        return;
    }

    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
    glUseProgram(program_);

    const float aspect =
        height_px_ > 0 ? static_cast<float>(width_px_) / static_cast<float>(height_px_) : 1.0F;
    const Mat4 model = mat4_identity();
    const Mat4 mvp = mat4_multiply(camera.view_proj_matrix(aspect), model);

    glUniformMatrix4fv(loc_mvp_, 1, GL_FALSE, mvp.data());
    glUniformMatrix4fv(loc_model_, 1, GL_FALSE, model.data());
    const Vec3f sun = vec3_normalize(Vec3f{sun_dir_[0], sun_dir_[1], sun_dir_[2]});
    glUniform3f(loc_sun_dir_, sun.x, sun.y, sun.z);
    glUniform3f(loc_view_pos_, camera.position.x, camera.position.y, camera.position.z);

    for (const GpuMesh& mesh : meshes_) {
        glBindVertexArray(mesh.vao);
        glDrawElements(GL_TRIANGLES, mesh.index_count, GL_UNSIGNED_INT, nullptr);
    }
    glBindVertexArray(0);
}

} // namespace agbot::render
