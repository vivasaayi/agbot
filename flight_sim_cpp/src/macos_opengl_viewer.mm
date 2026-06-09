#import <Cocoa/Cocoa.h>
#import <OpenGL/gl.h>

#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_flight_sim/TelemetryReplay.hpp"

#include <cmath>
#include <filesystem>
#include <iomanip>
#include <iostream>
#include <memory>
#include <algorithm>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

using agbot::flight_sim::DroneSimulation;
using agbot::flight_sim::DroneState;
using agbot::flight_sim::ControlMode;
using agbot::flight_sim::ManualControlInput;
using agbot::flight_sim::MissionLoader;
using agbot::flight_sim::TelemetryReplay;
using agbot::flight_sim::Vec3;
using agbot::flight_sim::Waypoint;
using agbot::flight_sim::WaypointAction;
using agbot::flight_sim::default_sample_mission_path;
using agbot::flight_sim::to_string;

namespace {

void set_color(double r, double g, double b, double a = 1.0) {
    glColor4d(r, g, b, a);
}

void draw_rect(double x, double y, double width, double height) {
    glBegin(GL_QUADS);
    glVertex2d(x, y);
    glVertex2d(x + width, y);
    glVertex2d(x + width, y + height);
    glVertex2d(x, y + height);
    glEnd();
}

void draw_circle(double x, double z, double radius, int segments = 32) {
    glBegin(GL_LINE_LOOP);
    for (int i = 0; i < segments; ++i) {
        const double a = (static_cast<double>(i) / static_cast<double>(segments)) * 2.0 * M_PI;
        glVertex2d(x + std::cos(a) * radius, z + std::sin(a) * radius);
    }
    glEnd();
}

void draw_filled_circle(double x, double z, double radius, int segments = 32) {
    glBegin(GL_TRIANGLE_FAN);
    glVertex2d(x, z);
    for (int i = 0; i <= segments; ++i) {
        const double a = (static_cast<double>(i) / static_cast<double>(segments)) * 2.0 * M_PI;
        glVertex2d(x + std::cos(a) * radius, z + std::sin(a) * radius);
    }
    glEnd();
}

void color_for_action(WaypointAction action) {
    switch (action) {
        case WaypointAction::Takeoff:
            set_color(0.1, 0.75, 1.0);
            return;
        case WaypointAction::Loiter:
            set_color(1.0, 0.82, 0.2);
            return;
        case WaypointAction::Land:
            set_color(1.0, 0.32, 0.25);
            return;
        case WaypointAction::ReturnHome:
            set_color(0.5, 0.85, 0.35);
            return;
        case WaypointAction::FlyThrough:
            set_color(0.9, 0.9, 0.95);
            return;
    }
}

std::filesystem::path mission_path_from_argv(int argc, char** argv) {
    for (int index = 1; index < argc; ++index) {
        const std::string current = argv[index];
        if (current == "--mission" && index + 1 < argc) {
            return argv[index + 1];
        }
    }
    return default_sample_mission_path();
}

NSString* ns_string(const std::filesystem::path& path) {
    return [NSString stringWithUTF8String:path.string().c_str()];
}

} // namespace

@interface FlightSimOpenGLView : NSOpenGLView {
    std::unique_ptr<DroneSimulation> simulation_;
    std::unique_ptr<TelemetryReplay> replay_;
    std::vector<Vec3> trail_;
    NSTimer* timer_;
    bool paused_;
    bool chase_camera_;
    bool manual_mode_;
    bool replay_mode_;
    bool key_w_;
    bool key_a_;
    bool key_s_;
    bool key_d_;
    bool key_q_;
    bool key_e_;
    bool key_up_;
    bool key_down_;
    bool key_t_;
    bool key_l_;
    int selected_waypoint_;
    bool dragging_waypoint_;
    double zoom_m_;
    double pan_x_;
    double pan_z_;
    double trail_sample_accumulator_;
    double replay_time_s_;
    std::filesystem::path mission_path_;
}

- (instancetype)initWithFrame:(NSRect)frame missionPath:(NSString*)missionPath;

@end

@implementation FlightSimOpenGLView

- (instancetype)initWithFrame:(NSRect)frame missionPath:(NSString*)missionPath {
    NSOpenGLPixelFormatAttribute attributes[] = {
        NSOpenGLPFAAccelerated,
        NSOpenGLPFADoubleBuffer,
        NSOpenGLPFAColorSize,
        24,
        NSOpenGLPFADepthSize,
        16,
        0,
    };

    NSOpenGLPixelFormat* pixelFormat = [[NSOpenGLPixelFormat alloc] initWithAttributes:attributes];
    self = [super initWithFrame:frame pixelFormat:pixelFormat];
    [pixelFormat release];

    if (self) {
        paused_ = false;
        chase_camera_ = true;
        manual_mode_ = false;
        replay_mode_ = false;
        key_w_ = false;
        key_a_ = false;
        key_s_ = false;
        key_d_ = false;
        key_q_ = false;
        key_e_ = false;
        key_up_ = false;
        key_down_ = false;
        key_t_ = false;
        key_l_ = false;
        selected_waypoint_ = -1;
        dragging_waypoint_ = false;
        zoom_m_ = 260.0;
        pan_x_ = 0.0;
        pan_z_ = 0.0;
        trail_sample_accumulator_ = 0.0;
        replay_time_s_ = 0.0;

        try {
            mission_path_ = std::filesystem::path([missionPath UTF8String]);
            simulation_ = std::make_unique<DroneSimulation>(MissionLoader::load_from_file(mission_path_));
        } catch (const std::exception& error) {
            std::cerr << "Unable to load mission: " << error.what() << "\n";
            mission_path_ = default_sample_mission_path();
            simulation_ = std::make_unique<DroneSimulation>(MissionLoader::load_from_file(default_sample_mission_path()));
        }

        timer_ = [NSTimer scheduledTimerWithTimeInterval:(1.0 / 60.0)
                                                  target:self
                                                selector:@selector(tick:)
                                                userInfo:nil
                                                 repeats:YES];
    }

    return self;
}

- (void)dealloc {
    [timer_ invalidate];
    [super dealloc];
}

- (BOOL)acceptsFirstResponder {
    return YES;
}

- (void)prepareOpenGL {
    [super prepareOpenGL];
    GLint swapInterval = 1;
    [[self openGLContext] setValues:&swapInterval forParameter:NSOpenGLContextParameterSwapInterval];

    glDisable(GL_DEPTH_TEST);
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
    glClearColor(0.025f, 0.032f, 0.042f, 1.0f);
}

- (void)tick:(NSTimer*)timer {
    (void)timer;
    if (simulation_ && !paused_ && replay_mode_ && replay_ && !replay_->empty()) {
        replay_time_s_ += 1.0 / 60.0;
        const DroneState& sample = replay_->sample(replay_time_s_);
        if (trail_.empty() || trail_sample_accumulator_ >= 0.2) {
            trail_.push_back(sample.position);
            trail_sample_accumulator_ = 0.0;
        }
        trail_sample_accumulator_ += 1.0 / 60.0;
        if (replay_time_s_ >= replay_->duration_s()) {
            paused_ = true;
        }
    } else if (simulation_ && !paused_ && !simulation_->is_complete()) {
        constexpr double dt_s = 1.0 / 60.0;
        [self updateManualInput];
        simulation_->step(dt_s);
        trail_sample_accumulator_ += dt_s;

        if (trail_.empty() || trail_sample_accumulator_ >= 0.2) {
            trail_.push_back(simulation_->state().position);
            trail_sample_accumulator_ = 0.0;
        }
    }

    [self updateWindowTitle];
    [self setNeedsDisplay:YES];
}

- (void)updateWindowTitle {
    if (!simulation_) {
        return;
    }

    const DroneState& state = [self displayState];
    std::ostringstream title;
    title << "AgBot FlightSim | " << simulation_->mission().name
          << " | " << (replay_mode_ ? "replay" : to_string(simulation_->control_mode()))
          << " | " << to_string(state.mode)
          << " | t=" << std::fixed << std::setprecision(1) << state.mission_time_s << "s"
          << " | battery=" << std::setprecision(0) << state.battery_percent << "%";
    [[self window] setTitle:[NSString stringWithUTF8String:title.str().c_str()]];
}

- (const DroneState&)displayState {
    if (replay_mode_ && replay_ && !replay_->empty()) {
        return replay_->sample(replay_time_s_);
    }
    return simulation_->state();
}

- (NSPoint)viewCenter {
    if (chase_camera_ && simulation_) {
        const DroneState& state = [self displayState];
        return NSMakePoint(state.position.x, state.position.z);
    }
    return NSMakePoint(pan_x_, pan_z_);
}

- (Vec3)worldPointFromEvent:(NSEvent*)event altitude:(double)altitude {
    const NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    const NSRect bounds = [self bounds];
    const NSPoint center = [self viewCenter];
    const double aspect = std::max(0.1, bounds.size.width / std::max(1.0, bounds.size.height));
    const double half_height = zoom_m_;
    const double half_width = zoom_m_ * aspect;
    const double x = center.x - half_width + (location.x / std::max(1.0, bounds.size.width)) * half_width * 2.0;
    const double z = center.y - half_height + (location.y / std::max(1.0, bounds.size.height)) * half_height * 2.0;
    return Vec3(x, altitude, z);
}

- (int)nearestWaypointIndex:(Vec3)point maxDistance:(double)maxDistance {
    if (!simulation_) {
        return -1;
    }

    int best_index = -1;
    double best_distance = maxDistance;
    const auto& waypoints = simulation_->mission().waypoints;
    for (std::size_t index = 0; index < waypoints.size(); ++index) {
        const Vec3 delta = waypoints[index].position - point;
        const double distance = std::sqrt(delta.x * delta.x + delta.z * delta.z);
        if (distance < best_distance) {
            best_distance = distance;
            best_index = static_cast<int>(index);
        }
    }
    return best_index;
}

- (void)addWaypointAt:(Vec3)point {
    if (!simulation_) {
        return;
    }

    Waypoint waypoint;
    waypoint.name = "edited_waypoint_" + std::to_string(simulation_->mission().waypoints.size() + 1);
    waypoint.position = point;
    waypoint.action = WaypointAction::FlyThrough;
    simulation_->mutable_mission().waypoints.push_back(waypoint);
    selected_waypoint_ = static_cast<int>(simulation_->mission().waypoints.size()) - 1;
}

- (void)deleteWaypointAtIndex:(int)index {
    if (!simulation_ || index < 0) {
        return;
    }
    auto& waypoints = simulation_->mutable_mission().waypoints;
    if (waypoints.size() <= 1 || static_cast<std::size_t>(index) >= waypoints.size()) {
        return;
    }
    waypoints.erase(waypoints.begin() + index);
    selected_waypoint_ = -1;
}

- (void)saveMission {
    if (!simulation_) {
        return;
    }

    const std::filesystem::path output =
        std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "edited_mission.json";
    try {
        MissionLoader::save_to_file(simulation_->mission(), output);
        std::cout << "Saved edited mission: " << output << "\n";
    } catch (const std::exception& error) {
        std::cerr << "Failed to save mission: " << error.what() << "\n";
    }
}

- (void)loadReplay {
    const std::filesystem::path path =
        std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "telemetry.jsonl";
    try {
        replay_ = std::make_unique<TelemetryReplay>(TelemetryReplay::load_jsonl(path));
        replay_mode_ = replay_ && !replay_->empty();
        replay_time_s_ = 0.0;
        paused_ = false;
        trail_.clear();
        if (simulation_) {
            simulation_->set_control_mode(ControlMode::Replay);
        }
        std::cout << "Loaded telemetry replay: " << path << "\n";
    } catch (const std::exception& error) {
        std::cerr << "Failed to load replay: " << error.what() << "\n";
    }
}

- (void)updateManualInput {
    if (!simulation_ || !manual_mode_ || replay_mode_) {
        return;
    }

    ManualControlInput input;
    input.arm = true;
    input.pitch = (key_w_ ? 1.0 : 0.0) + (key_s_ ? -1.0 : 0.0);
    input.roll = (key_d_ ? 1.0 : 0.0) + (key_a_ ? -1.0 : 0.0);
    input.yaw = (key_e_ ? 1.0 : 0.0) + (key_q_ ? -1.0 : 0.0);
    input.throttle = (key_up_ ? 1.0 : 0.0) + (key_down_ ? -1.0 : 0.0);
    input.takeoff = key_t_;
    input.land = key_l_;
    simulation_->set_manual_input(input);
}

- (void)drawGrid {
    set_color(0.16, 0.18, 0.21, 1.0);
    glLineWidth(1.0f);
    glBegin(GL_LINES);
    for (int value = -1000; value <= 1000; value += 20) {
        glVertex2d(value, -1000);
        glVertex2d(value, 1000);
        glVertex2d(-1000, value);
        glVertex2d(1000, value);
    }
    glEnd();

    set_color(0.36, 0.4, 0.45, 1.0);
    glLineWidth(2.0f);
    glBegin(GL_LINES);
    glVertex2d(-1000, 0);
    glVertex2d(1000, 0);
    glVertex2d(0, -1000);
    glVertex2d(0, 1000);
    glEnd();
}

- (void)drawMissionPath {
    if (!simulation_) {
        return;
    }

    const auto& mission = simulation_->mission();
    set_color(0.72, 0.78, 0.88, 0.45);
    glLineWidth(3.0f);
    glBegin(GL_LINE_STRIP);
    glVertex2d(mission.home.x, mission.home.z);
    for (const Waypoint& waypoint : mission.waypoints) {
        glVertex2d(waypoint.position.x, waypoint.position.z);
    }
    glEnd();

    for (const Waypoint& waypoint : mission.waypoints) {
        const int waypoint_index = static_cast<int>(&waypoint - mission.waypoints.data());
        color_for_action(waypoint.action);
        draw_filled_circle(waypoint.position.x, waypoint.position.z, 3.2);
        if (waypoint_index == selected_waypoint_) {
            set_color(1.0, 1.0, 1.0, 1.0);
            draw_circle(waypoint.position.x, waypoint.position.z, 8.0);
        } else {
            set_color(0.02, 0.025, 0.03, 1.0);
            draw_circle(waypoint.position.x, waypoint.position.z, 5.0);
        }
    }

    set_color(0.25, 1.0, 0.72, 1.0);
    draw_filled_circle(mission.home.x, mission.home.z, 4.0);
}

- (void)drawTrail {
    if (trail_.size() < 2) {
        return;
    }

    set_color(0.2, 0.85, 1.0, 0.85);
    glLineWidth(2.0f);
    glBegin(GL_LINE_STRIP);
    for (const Vec3& point : trail_) {
        glVertex2d(point.x, point.z);
    }
    glEnd();
}

- (void)drawDrone {
    if (!simulation_) {
        return;
    }

    const DroneState& state = [self displayState];
    const Vec3 position = state.position;
    const double yaw = state.yaw_rad;

    const Vec3 forward(std::sin(yaw), 0.0, std::cos(yaw));
    const Vec3 right(std::cos(yaw), 0.0, -std::sin(yaw));

    const Vec3 nose = position + forward * 9.0;
    const Vec3 left = position - forward * 6.0 - right * 5.0;
    const Vec3 right_point = position - forward * 6.0 + right * 5.0;

    set_color(1.0, 0.95, 0.55, 1.0);
    glBegin(GL_TRIANGLES);
    glVertex2d(nose.x, nose.z);
    glVertex2d(left.x, left.z);
    glVertex2d(right_point.x, right_point.z);
    glEnd();

    set_color(0.02, 0.025, 0.03, 1.0);
    glLineWidth(2.0f);
    glBegin(GL_LINE_LOOP);
    glVertex2d(nose.x, nose.z);
    glVertex2d(left.x, left.z);
    glVertex2d(right_point.x, right_point.z);
    glEnd();

    set_color(0.1, 0.9, 1.0, 0.75);
    draw_circle(position.x, position.z, std::max(5.0, state.position.y * 0.2));
}

- (void)drawHud:(NSRect)bounds {
    if (!simulation_) {
        return;
    }

    const double width = bounds.size.width;
    const double height = bounds.size.height;
    const DroneState& state = [self displayState];

    glMatrixMode(GL_PROJECTION);
    glPushMatrix();
    glLoadIdentity();
    glOrtho(0.0, width, 0.0, height, -1.0, 1.0);

    glMatrixMode(GL_MODELVIEW);
    glPushMatrix();
    glLoadIdentity();

    set_color(0.0, 0.0, 0.0, 0.35);
    draw_rect(20.0, height - 58.0, 230.0, 38.0);

    set_color(0.22, 0.24, 0.28, 1.0);
    draw_rect(34.0, height - 45.0, 180.0, 10.0);

    const double battery_width = 180.0 * std::clamp(state.battery_percent / 100.0, 0.0, 1.0);
    if (state.battery_percent < 25.0) {
        set_color(1.0, 0.25, 0.18, 1.0);
    } else {
        set_color(0.2, 0.86, 0.42, 1.0);
    }
    draw_rect(34.0, height - 45.0, battery_width, 10.0);

    set_color(0.22, 0.24, 0.28, 1.0);
    draw_rect(34.0, height - 28.0, 180.0, 8.0);
    set_color(0.25, 0.55, 1.0, 1.0);
    draw_rect(34.0, height - 28.0, 180.0 * simulation_->progress(), 8.0);

    glPopMatrix();
    glMatrixMode(GL_PROJECTION);
    glPopMatrix();
    glMatrixMode(GL_MODELVIEW);
}

- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    [[self openGLContext] makeCurrentContext];

    const NSRect bounds = [self bounds];
    glViewport(0, 0, static_cast<GLsizei>(bounds.size.width), static_cast<GLsizei>(bounds.size.height));
    glClear(GL_COLOR_BUFFER_BIT);

    double center_x = pan_x_;
    double center_z = pan_z_;
    if (chase_camera_ && simulation_) {
        const DroneState& state = [self displayState];
        center_x = state.position.x;
        center_z = state.position.z;
    }

    const double aspect = std::max(0.1, bounds.size.width / std::max(1.0, bounds.size.height));
    const double half_height = zoom_m_;
    const double half_width = zoom_m_ * aspect;

    glMatrixMode(GL_PROJECTION);
    glLoadIdentity();
    glOrtho(center_x - half_width, center_x + half_width, center_z - half_height, center_z + half_height, -1.0, 1.0);
    glMatrixMode(GL_MODELVIEW);
    glLoadIdentity();

    [self drawGrid];
    [self drawMissionPath];
    [self drawTrail];
    [self drawDrone];
    [self drawHud:bounds];

    [[self openGLContext] flushBuffer];
}

- (void)keyDown:(NSEvent*)event {
    if (([event modifierFlags] & NSEventModifierFlagCommand) != 0) {
        NSString* commandCharacters = [[event charactersIgnoringModifiers] lowercaseString];
        if ([commandCharacters length] > 0 && [commandCharacters characterAtIndex:0] == 's') {
            [self saveMission];
            return;
        }
    }

    NSString* characters = [[event charactersIgnoringModifiers] lowercaseString];
    if ([characters length] > 0) {
        const unichar key = [characters characterAtIndex:0];
        if (key == ' ') {
            paused_ = !paused_;
            return;
        }
        if (key == 'r' && simulation_) {
            if (replay_mode_) {
                replay_time_s_ = 0.0;
                paused_ = false;
            } else {
                simulation_->reset();
                if (manual_mode_) {
                    simulation_->set_control_mode(ControlMode::Manual);
                    simulation_->arm();
                }
            }
            trail_.clear();
            return;
        }
        if (key == 'm' && simulation_) {
            replay_mode_ = false;
            manual_mode_ = !manual_mode_;
            simulation_->set_control_mode(manual_mode_ ? ControlMode::Manual : ControlMode::Autopilot);
            if (manual_mode_) {
                simulation_->arm();
            }
            return;
        }
        if (key == 'g') {
            if (replay_mode_) {
                replay_mode_ = false;
                if (simulation_) {
                    simulation_->set_control_mode(manual_mode_ ? ControlMode::Manual : ControlMode::Autopilot);
                }
            } else {
                [self loadReplay];
            }
            return;
        }
        if (key == 'w') {
            key_w_ = true;
            return;
        }
        if (key == 'a') {
            key_a_ = true;
            return;
        }
        if (key == 's') {
            key_s_ = true;
            return;
        }
        if (key == 'd') {
            key_d_ = true;
            return;
        }
        if (key == 'q') {
            key_q_ = true;
            return;
        }
        if (key == 'e') {
            key_e_ = true;
            return;
        }
        if (key == 't') {
            key_t_ = true;
            return;
        }
        if (key == 'l') {
            key_l_ = true;
            return;
        }
        if (key == 'c') {
            chase_camera_ = !chase_camera_;
            return;
        }
        if (key == '-' || key == '_') {
            zoom_m_ = std::min(1000.0, zoom_m_ * 1.15);
            return;
        }
        if (key == '=' || key == '+') {
            zoom_m_ = std::max(30.0, zoom_m_ / 1.15);
            return;
        }
    }

    constexpr double pan_step = 20.0;
    switch ([event keyCode]) {
        case 123:
            chase_camera_ = false;
            pan_x_ -= pan_step;
            break;
        case 124:
            chase_camera_ = false;
            pan_x_ += pan_step;
            break;
        case 125:
            if (manual_mode_) {
                key_down_ = true;
            } else {
                chase_camera_ = false;
                pan_z_ -= pan_step;
            }
            break;
        case 126:
            if (manual_mode_) {
                key_up_ = true;
            } else {
                chase_camera_ = false;
                pan_z_ += pan_step;
            }
            break;
        default:
            [super keyDown:event];
            break;
    }
}

- (void)keyUp:(NSEvent*)event {
    NSString* characters = [[event charactersIgnoringModifiers] lowercaseString];
    if ([characters length] > 0) {
        const unichar key = [characters characterAtIndex:0];
        if (key == 'w') {
            key_w_ = false;
            return;
        }
        if (key == 'a') {
            key_a_ = false;
            return;
        }
        if (key == 's') {
            key_s_ = false;
            return;
        }
        if (key == 'd') {
            key_d_ = false;
            return;
        }
        if (key == 'q') {
            key_q_ = false;
            return;
        }
        if (key == 'e') {
            key_e_ = false;
            return;
        }
        if (key == 't') {
            key_t_ = false;
            return;
        }
        if (key == 'l') {
            key_l_ = false;
            return;
        }
    }

    switch ([event keyCode]) {
        case 125:
            key_down_ = false;
            break;
        case 126:
            key_up_ = false;
            break;
        default:
            [super keyUp:event];
            break;
    }
}

- (void)mouseDown:(NSEvent*)event {
    if (!simulation_ || replay_mode_) {
        return;
    }

    double altitude = 30.0;
    if (!simulation_->mission().waypoints.empty()) {
        altitude = simulation_->mission().waypoints.back().position.y;
    }

    const Vec3 point = [self worldPointFromEvent:event altitude:altitude];
    const int nearest = [self nearestWaypointIndex:point maxDistance:(zoom_m_ * 0.035)];

    if (([event modifierFlags] & NSEventModifierFlagOption) != 0) {
        [self deleteWaypointAtIndex:nearest];
        return;
    }

    if (nearest >= 0) {
        selected_waypoint_ = nearest;
        dragging_waypoint_ = true;
    } else {
        [self addWaypointAt:point];
        dragging_waypoint_ = true;
    }
}

- (void)mouseDragged:(NSEvent*)event {
    if (!simulation_ || selected_waypoint_ < 0 || !dragging_waypoint_) {
        return;
    }

    auto& waypoints = simulation_->mutable_mission().waypoints;
    if (static_cast<std::size_t>(selected_waypoint_) >= waypoints.size()) {
        return;
    }

    const double altitude = waypoints[static_cast<std::size_t>(selected_waypoint_)].position.y;
    waypoints[static_cast<std::size_t>(selected_waypoint_)].position = [self worldPointFromEvent:event altitude:altitude];
}

- (void)mouseUp:(NSEvent*)event {
    (void)event;
    dragging_waypoint_ = false;
}

- (void)scrollWheel:(NSEvent*)event {
    if ([event scrollingDeltaY] > 0.0) {
        zoom_m_ = std::max(30.0, zoom_m_ / 1.08);
    } else if ([event scrollingDeltaY] < 0.0) {
        zoom_m_ = std::min(1000.0, zoom_m_ * 1.08);
    }
}

@end

@interface FlightSimAppDelegate : NSObject <NSApplicationDelegate> {
    NSWindow* window_;
    std::filesystem::path mission_path_;
}

- (instancetype)initWithMissionPath:(std::filesystem::path)missionPath;

@end

@implementation FlightSimAppDelegate

- (instancetype)initWithMissionPath:(std::filesystem::path)missionPath {
    self = [super init];
    if (self) {
        mission_path_ = std::move(missionPath);
    }
    return self;
}

- (void)applicationDidFinishLaunching:(NSNotification*)notification {
    (void)notification;

    NSRect frame = NSMakeRect(80, 80, 1280, 800);
    window_ = [[NSWindow alloc] initWithContentRect:frame
                                          styleMask:(NSWindowStyleMaskTitled |
                                                     NSWindowStyleMaskClosable |
                                                     NSWindowStyleMaskResizable |
                                                     NSWindowStyleMaskMiniaturizable)
                                            backing:NSBackingStoreBuffered
                                              defer:NO];

    [window_ setTitle:@"AgBot FlightSim"];
    FlightSimOpenGLView* view = [[FlightSimOpenGLView alloc] initWithFrame:frame
                                                               missionPath:ns_string(mission_path_)];
    [window_ setContentView:view];
    [window_ makeFirstResponder:view];
    [view release];

    [window_ center];
    [window_ makeKeyAndOrderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];
}

- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication*)sender {
    (void)sender;
    return YES;
}

@end

int main(int argc, char** argv) {
    @autoreleasepool {
        const std::filesystem::path mission_path = mission_path_from_argv(argc, argv);

        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];

        NSMenu* mainMenu = [[[NSMenu alloc] initWithTitle:@"AgBot FlightSim"] autorelease];
        NSMenuItem* appMenuItem = [[[NSMenuItem alloc] init] autorelease];
        [mainMenu addItem:appMenuItem];
        [NSApp setMainMenu:mainMenu];

        NSMenu* appMenu = [[[NSMenu alloc] initWithTitle:@"AgBot FlightSim"] autorelease];
        NSString* quitTitle = @"Quit AgBot FlightSim";
        NSMenuItem* quitItem = [[[NSMenuItem alloc] initWithTitle:quitTitle
                                                           action:@selector(terminate:)
                                                    keyEquivalent:@"q"] autorelease];
        [appMenu addItem:quitItem];
        [appMenuItem setSubmenu:appMenu];

        FlightSimAppDelegate* delegate = [[FlightSimAppDelegate alloc] initWithMissionPath:mission_path];
        [NSApp setDelegate:delegate];
        [NSApp run];
        [delegate release];
    }
    return 0;
}
