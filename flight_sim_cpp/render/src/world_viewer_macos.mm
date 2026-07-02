// agbot_world_viewer — macOS Cocoa app hosting the modern OpenGL 4.1 Core renderer.
//
// Usage:
//   agbot_world_viewer [scene.agbscn]   windowed viewer (demo scene if no file)
//   agbot_world_viewer --self-check     offscreen render sanity check, exits 0/1

#import <Cocoa/Cocoa.h>

#define GL_SILENCE_DEPRECATION 1
#import <OpenGL/OpenGL.h>
#import <OpenGL/gl3.h>

#include "agbot_render/Camera.hpp"
#include "agbot_render/DemoScene.hpp"
#include "agbot_render/GlRenderer.hpp"
#include "agbot_render/SceneFile.hpp"

#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <memory>
#include <string>
#include <unordered_set>
#include <vector>

namespace {

agbot::render::RenderScene load_scene_or_demo(int argc, const char** argv) {
    if (argc > 1 && argv[1][0] != '-') {
        const std::filesystem::path path = argv[1];
        agbot::render::SceneFileResult result = agbot::render::read_scene_file(path);
        if (result.ok()) {
            std::printf("[agbot_world_viewer] loaded scene file: %s (%zu meshes, %zu markers)\n",
                        path.string().c_str(), result.scene.static_meshes.size(),
                        result.scene.markers.size());
            return result.scene;
        }
        std::fprintf(stderr, "[agbot_world_viewer] %s — falling back to demo scene\n",
                     result.error->message.c_str());
    }
    std::printf("[agbot_world_viewer] using built-in procedural demo scene\n");
    return agbot::render::build_demo_scene();
}

// ---------------------------------------------------------------------------
// Offscreen self-check: CGL context (no window, no NSApplication run loop),
// render the demo scene into an FBO, read pixels back, assert non-uniform.
// ---------------------------------------------------------------------------

bool write_ppm(const std::filesystem::path& path, int width, int height,
               const std::vector<std::uint8_t>& rgba) {
    std::error_code ec;
    std::filesystem::create_directories(path.parent_path(), ec);
    std::ofstream out(path, std::ios::binary | std::ios::trunc);
    if (!out.is_open()) {
        return false;
    }
    out << "P6\n" << width << " " << height << "\n255\n";
    // Flip vertically: glReadPixels origin is bottom-left, PPM is top-left.
    for (int row = height - 1; row >= 0; --row) {
        for (int col = 0; col < width; ++col) {
            const std::size_t src = (static_cast<std::size_t>(row) * width + col) * 4;
            out.put(static_cast<char>(rgba[src + 0]));
            out.put(static_cast<char>(rgba[src + 1]));
            out.put(static_cast<char>(rgba[src + 2]));
        }
    }
    return out.good();
}

int run_self_check(const agbot::render::RenderScene& scene) {
    constexpr int kWidth = 640;
    constexpr int kHeight = 480;

    CGLPixelFormatAttribute attrs_41[] = {
        kCGLPFAOpenGLProfile, static_cast<CGLPixelFormatAttribute>(kCGLOGLPVersion_GL4_Core),
        kCGLPFAColorSize, static_cast<CGLPixelFormatAttribute>(24),
        kCGLPFADepthSize, static_cast<CGLPixelFormatAttribute>(24),
        kCGLPFAAccelerated,
        static_cast<CGLPixelFormatAttribute>(0),
    };
    CGLPixelFormatAttribute attrs_32[] = {
        kCGLPFAOpenGLProfile, static_cast<CGLPixelFormatAttribute>(kCGLOGLPVersion_3_2_Core),
        kCGLPFAColorSize, static_cast<CGLPixelFormatAttribute>(24),
        kCGLPFADepthSize, static_cast<CGLPixelFormatAttribute>(24),
        static_cast<CGLPixelFormatAttribute>(0),
    };

    CGLPixelFormatObj pixel_format = nullptr;
    GLint num_formats = 0;
    const char* profile_label = "OpenGL 4.1 Core";
    if (CGLChoosePixelFormat(attrs_41, &pixel_format, &num_formats) != kCGLNoError ||
        pixel_format == nullptr) {
        profile_label = "OpenGL 3.2 Core (fallback)";
        if (CGLChoosePixelFormat(attrs_32, &pixel_format, &num_formats) != kCGLNoError ||
            pixel_format == nullptr) {
            std::fprintf(stderr, "self-check FAIL: no core-profile pixel format available\n");
            return 1;
        }
    }

    CGLContextObj context = nullptr;
    if (CGLCreateContext(pixel_format, nullptr, &context) != kCGLNoError || context == nullptr) {
        CGLReleasePixelFormat(pixel_format);
        std::fprintf(stderr, "self-check FAIL: CGLCreateContext failed\n");
        return 1;
    }
    CGLReleasePixelFormat(pixel_format);
    CGLSetCurrentContext(context);

    std::printf("[self-check] context profile: %s\n", profile_label);

    // Offscreen FBO.
    GLuint fbo = 0;
    GLuint color_rb = 0;
    GLuint depth_rb = 0;
    glGenFramebuffers(1, &fbo);
    glGenRenderbuffers(1, &color_rb);
    glGenRenderbuffers(1, &depth_rb);
    glBindRenderbuffer(GL_RENDERBUFFER, color_rb);
    glRenderbufferStorage(GL_RENDERBUFFER, GL_RGBA8, kWidth, kHeight);
    glBindRenderbuffer(GL_RENDERBUFFER, depth_rb);
    glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH_COMPONENT24, kWidth, kHeight);
    glBindFramebuffer(GL_FRAMEBUFFER, fbo);
    glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_RENDERBUFFER, color_rb);
    glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT, GL_RENDERBUFFER, depth_rb);
    if (glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE) {
        std::fprintf(stderr, "self-check FAIL: FBO incomplete\n");
        CGLSetCurrentContext(nullptr);
        CGLReleaseContext(context);
        return 1;
    }

    int exit_code = 1;
    {
        agbot::render::GlRenderer renderer;
        if (!renderer.init(kWidth, kHeight)) {
            std::fprintf(stderr, "self-check FAIL: renderer init: %s\n", renderer.last_error());
        } else {
            if (!renderer.uploadScene(scene)) {
                std::fprintf(stderr, "self-check FAIL: uploadScene: %s\n",
                             renderer.last_error());
            } else {
                // Frame the scene from its bounding box so arbitrary
                // .agbscn scenes (not just the built-in demo) are visible.
                float min_x = 1e9F, max_x = -1e9F, max_y = -1e9F, min_z = 1e9F, max_z = -1e9F;
                for (const agbot::render::RenderMesh& mesh : scene.static_meshes) {
                    for (const agbot::render::RenderVertex& vertex : mesh.vertices) {
                        min_x = std::min(min_x, vertex.px);
                        max_x = std::max(max_x, vertex.px);
                        max_y = std::max(max_y, vertex.py);
                        min_z = std::min(min_z, vertex.pz);
                        max_z = std::max(max_z, vertex.pz);
                    }
                }
                const float span = std::max(max_x - min_x, max_z - min_z);
                agbot::render::Camera camera;
                camera.far_plane = std::max(camera.far_plane, span * 2.5F);
                camera.position = agbot::render::Vec3f{
                    (min_x + max_x) * 0.5F,
                    std::max(60.0F, max_y + span * 0.18F),
                    max_z + span * 0.35F};
                camera.yaw_rad = 0.0F;
                camera.pitch_rad = -0.45F;

                for (int frame = 0; frame < 3; ++frame) {
                    glBindFramebuffer(GL_FRAMEBUFFER, fbo);
                    renderer.drawFrame(camera);
                    glFinish();
                }

                std::vector<std::uint8_t> rgba(
                    static_cast<std::size_t>(kWidth) * kHeight * 4, 0);
                glBindFramebuffer(GL_FRAMEBUFFER, fbo);
                glReadPixels(0, 0, kWidth, kHeight, GL_RGBA, GL_UNSIGNED_BYTE, rgba.data());

                // Compare against the clear color (sky blue, from GlRenderer::init).
                const std::uint8_t clear_r = static_cast<std::uint8_t>(0.53F * 255.0F + 0.5F);
                const std::uint8_t clear_g = static_cast<std::uint8_t>(0.71F * 255.0F + 0.5F);
                const std::uint8_t clear_b = static_cast<std::uint8_t>(0.92F * 255.0F + 0.5F);
                std::size_t differing = 0;
                const std::size_t pixel_count = static_cast<std::size_t>(kWidth) * kHeight;
                for (std::size_t p = 0; p < pixel_count; ++p) {
                    const std::uint8_t r = rgba[p * 4 + 0];
                    const std::uint8_t g = rgba[p * 4 + 1];
                    const std::uint8_t b = rgba[p * 4 + 2];
                    const int dr = std::abs(static_cast<int>(r) - clear_r);
                    const int dg = std::abs(static_cast<int>(g) - clear_g);
                    const int db = std::abs(static_cast<int>(b) - clear_b);
                    if (dr + dg + db > 12) {
                        ++differing;
                    }
                }
                const double fraction =
                    static_cast<double>(differing) / static_cast<double>(pixel_count);

                const std::filesystem::path ppm_path = "out/render/self_check.ppm";
                const bool ppm_ok = write_ppm(ppm_path, kWidth, kHeight, rgba);

                std::printf("[self-check] non-clear pixels: %zu / %zu (%.1f%%), ppm: %s (%s)\n",
                            differing, pixel_count, fraction * 100.0,
                            ppm_path.string().c_str(), ppm_ok ? "written" : "WRITE FAILED");

                if (fraction > 0.05) {
                    std::printf("self-check PASS\n");
                    exit_code = 0;
                } else {
                    std::fprintf(stderr,
                                 "self-check FAIL: rendered image too uniform "
                                 "(%.2f%% pixels differ, need >5%%)\n",
                                 fraction * 100.0);
                }
            }
        }
        renderer.shutdown();
    }

    glBindFramebuffer(GL_FRAMEBUFFER, 0);
    glDeleteRenderbuffers(1, &color_rb);
    glDeleteRenderbuffers(1, &depth_rb);
    glDeleteFramebuffers(1, &fbo);
    CGLSetCurrentContext(nullptr);
    CGLReleaseContext(context);
    return exit_code;
}

} // namespace

// ---------------------------------------------------------------------------
// Windowed viewer
// ---------------------------------------------------------------------------

@interface AgbotWorldView : NSOpenGLView {
    agbot::render::GlRenderer* _renderer;
    agbot::render::RenderScene* _scene;
    agbot::render::Camera _camera;
    NSTimer* _timer;
    std::unordered_set<unsigned short>* _keysDown;
    BOOL _sceneUploaded;
    double _lastFrameTime;
    double _fpsAccumTime;
    int _fpsAccumFrames;
    double _fps;
}
- (instancetype)initWithFrame:(NSRect)frame scene:(agbot::render::RenderScene*)scene;
@end

@implementation AgbotWorldView

- (instancetype)initWithFrame:(NSRect)frame scene:(agbot::render::RenderScene*)scene {
    NSOpenGLPixelFormatAttribute attrs41[] = {
        NSOpenGLPFAOpenGLProfile, NSOpenGLProfileVersion4_1Core,
        NSOpenGLPFAColorSize, 24,
        NSOpenGLPFADepthSize, 24,
        NSOpenGLPFADoubleBuffer,
        NSOpenGLPFAAccelerated,
        NSOpenGLPFAMultisample,
        NSOpenGLPFASampleBuffers, 1,
        NSOpenGLPFASamples, 4,
        0,
    };
    NSOpenGLPixelFormat* format = [[NSOpenGLPixelFormat alloc] initWithAttributes:attrs41];
    if (format == nil) {
        NSOpenGLPixelFormatAttribute attrs32[] = {
            NSOpenGLPFAOpenGLProfile, NSOpenGLProfileVersion3_2Core,
            NSOpenGLPFAColorSize, 24,
            NSOpenGLPFADepthSize, 24,
            NSOpenGLPFADoubleBuffer,
            0,
        };
        format = [[NSOpenGLPixelFormat alloc] initWithAttributes:attrs32];
        NSLog(@"[agbot_world_viewer] falling back to OpenGL 3.2 Core profile");
    }

    self = [super initWithFrame:frame pixelFormat:format];
    if (self != nil) {
        _renderer = new agbot::render::GlRenderer();
        _scene = scene;
        _keysDown = new std::unordered_set<unsigned short>();
        _sceneUploaded = NO;
        _lastFrameTime = 0.0;
        _fpsAccumTime = 0.0;
        _fpsAccumFrames = 0;
        _fps = 0.0;

        _camera.position = agbot::render::Vec3f{0.0F, 60.0F, 180.0F};
        _camera.pitch_rad = -0.25F;
        [self setWantsBestResolutionOpenGLSurface:YES];
    }
    return self;
}

- (void)dealloc {
    [_timer invalidate];
    delete _renderer;
    delete _keysDown;
    [super dealloc];
}

- (BOOL)acceptsFirstResponder {
    return YES;
}

- (void)prepareOpenGL {
    [super prepareOpenGL];
    [[self openGLContext] makeCurrentContext];

    GLint swap_interval = 1;
    [[self openGLContext] setValues:&swap_interval forParameter:NSOpenGLContextParameterSwapInterval];

    const NSRect backing = [self convertRectToBacking:[self bounds]];
    if (!_renderer->init(static_cast<int>(backing.size.width),
                         static_cast<int>(backing.size.height))) {
        NSLog(@"[agbot_world_viewer] renderer init failed: %s", _renderer->last_error());
        [NSApp terminate:nil];
        return;
    }
    if (!_renderer->uploadScene(*_scene)) {
        NSLog(@"[agbot_world_viewer] scene upload failed: %s", _renderer->last_error());
        [NSApp terminate:nil];
        return;
    }
    _sceneUploaded = YES;
    _lastFrameTime = [[NSDate date] timeIntervalSinceReferenceDate];

    _timer = [NSTimer scheduledTimerWithTimeInterval:(1.0 / 60.0)
                                              target:self
                                            selector:@selector(tick:)
                                            userInfo:nil
                                             repeats:YES];
    [[NSRunLoop currentRunLoop] addTimer:_timer forMode:NSEventTrackingRunLoopMode];
}

- (void)reshape {
    [super reshape];
    [[self openGLContext] makeCurrentContext];
    const NSRect backing = [self convertRectToBacking:[self bounds]];
    _renderer->resize(static_cast<int>(backing.size.width),
                      static_cast<int>(backing.size.height));
}

- (void)tick:(NSTimer*)timer {
    const double now = [[NSDate date] timeIntervalSinceReferenceDate];
    const float dt = static_cast<float>(now - _lastFrameTime);
    _lastFrameTime = now;

    const float move_speed = 45.0F; // m/s fly speed
    const float step = move_speed * dt;
    if (_keysDown->count(13) > 0) { _camera.move_forward(step); }  // W
    if (_keysDown->count(1) > 0) { _camera.move_forward(-step); }  // S
    if (_keysDown->count(0) > 0) { _camera.move_right(-step); }    // A
    if (_keysDown->count(2) > 0) { _camera.move_right(step); }     // D
    if (_keysDown->count(12) > 0) { _camera.move_up(-step); }      // Q
    if (_keysDown->count(14) > 0) { _camera.move_up(step); }       // E

    [self setNeedsDisplay:YES];

    _fpsAccumTime += dt;
    _fpsAccumFrames += 1;
    if (_fpsAccumTime >= 0.5) {
        _fps = static_cast<double>(_fpsAccumFrames) / _fpsAccumTime;
        _fpsAccumTime = 0.0;
        _fpsAccumFrames = 0;
        [[self window] setTitle:[NSString stringWithFormat:
            @"agbot_world_viewer — %.0f fps — pos (%.1f, %.1f, %.1f)",
            _fps, _camera.position.x, _camera.position.y, _camera.position.z]];
    }
}

- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    [[self openGLContext] makeCurrentContext];
    if (_sceneUploaded) {
        _renderer->drawFrame(_camera);
    }
    [[self openGLContext] flushBuffer];
}

- (void)keyDown:(NSEvent*)event {
    if ([event keyCode] == 53) { // ESC
        [NSApp terminate:nil];
        return;
    }
    _keysDown->insert([event keyCode]);
}

- (void)keyUp:(NSEvent*)event {
    _keysDown->erase([event keyCode]);
}

- (void)mouseDragged:(NSEvent*)event {
    const float sensitivity = 0.005F;
    _camera.add_yaw_pitch(static_cast<float>([event deltaX]) * sensitivity,
                          -static_cast<float>([event deltaY]) * sensitivity);
}

- (void)scrollWheel:(NSEvent*)event {
    _camera.zoom_fov(-static_cast<float>([event deltaY]) * 1.5F);
}

@end

@interface AgbotAppDelegate : NSObject <NSApplicationDelegate>
@end

@implementation AgbotAppDelegate
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication*)sender {
    (void)sender;
    return YES;
}
@end

namespace {

int run_windowed(int argc, const char** argv) {
    @autoreleasepool {
        static agbot::render::RenderScene scene = load_scene_or_demo(argc, argv);

        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];

        AgbotAppDelegate* delegate = [[AgbotAppDelegate alloc] init];
        [NSApp setDelegate:delegate];

        const NSRect frame = NSMakeRect(0, 0, 1280, 800);
        NSWindow* window = [[NSWindow alloc]
            initWithContentRect:frame
                      styleMask:(NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                                 NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable)
                        backing:NSBackingStoreBuffered
                          defer:NO];
        [window setTitle:@"agbot_world_viewer"];
        [window center];

        AgbotWorldView* view = [[AgbotWorldView alloc] initWithFrame:frame scene:&scene];
        [window setContentView:view];
        [window makeFirstResponder:view];
        [window makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];

        std::printf("[agbot_world_viewer] controls: WASD move, Q/E down/up, "
                    "mouse drag look, scroll zoom, ESC quit\n");
        [NSApp run];
    }
    return 0;
}

} // namespace

int main(int argc, const char** argv) {
    for (int i = 1; i < argc; ++i) {
        if (std::string(argv[i]) == "--self-check") {
            return run_self_check(load_scene_or_demo(argc, argv));
        }
    }
    return run_windowed(argc, argv);
}
