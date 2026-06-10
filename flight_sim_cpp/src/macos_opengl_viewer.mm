#import <Cocoa/Cocoa.h>
#import <OpenGL/gl.h>

#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_flight_sim/TelemetryRecorder.hpp"
#include "agbot_flight_sim/TelemetryReplay.hpp"

#include <chrono>
#include <cmath>
#include <ctime>
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
using agbot::flight_sim::TelemetryRecorder;
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

NSString* ns_string(const std::string& text) {
    return [NSString stringWithUTF8String:text.c_str()];
}

std::filesystem::path telemetry_latest_path() {
    return std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "telemetry.jsonl";
}

std::filesystem::path telemetry_runs_dir() {
    return std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "runs";
}

std::string timestamp_for_filename() {
    const auto now = std::chrono::system_clock::now();
    const std::time_t now_time = std::chrono::system_clock::to_time_t(now);
    std::tm local_time {};
    localtime_r(&now_time, &local_time);

    std::ostringstream output;
    output << std::put_time(&local_time, "%Y%m%d_%H%M%S");
    return output.str();
}

std::filesystem::path telemetry_run_path() {
    return telemetry_runs_dir() / ("flight_" + timestamp_for_filename() + ".jsonl");
}

NSTextField* make_label(NSString* text, NSFont* font, NSColor* color) {
    NSTextField* label = [NSTextField labelWithString:text];
    [label setFont:font];
    [label setTextColor:color];
    [label setLineBreakMode:NSLineBreakByWordWrapping];
    [label setSelectable:NO];
    return label;
}

NSButton* make_button(NSString* title, id target, SEL action) {
    NSButton* button = [NSButton buttonWithTitle:title target:target action:action];
    [button setBezelStyle:NSBezelStyleRounded];
    [button setFont:[NSFont systemFontOfSize:12.0 weight:NSFontWeightMedium]];
    return button;
}

NSSlider* make_slider(id target, SEL action) {
    NSSlider* slider = [NSSlider sliderWithValue:0.0 minValue:0.0 maxValue:1.0 target:target action:action];
    [slider setContinuous:YES];
    [slider setEnabled:NO];
    return slider;
}

struct ViewBounds {
    double min_x = 0.0;
    double max_x = 0.0;
    double min_z = 0.0;
    double max_z = 0.0;
};

} // namespace

@interface FlightSimOpenGLView : NSOpenGLView {
    std::unique_ptr<DroneSimulation> simulation_;
    std::unique_ptr<TelemetryReplay> replay_;
    std::unique_ptr<TelemetryRecorder> run_recorder_;
    std::unique_ptr<TelemetryRecorder> latest_recorder_;
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
    NSVisualEffectView* side_panel_;
    NSTextField* title_label_;
    NSTextField* telemetry_label_;
    NSTextField* mission_label_;
    NSTextField* message_label_;
    NSTextField* replay_time_label_;
    NSButton* manual_button_;
    NSButton* arm_button_;
    NSButton* pause_button_;
    NSButton* chase_button_;
    NSButton* fit_button_;
    NSButton* replay_button_;
    NSButton* load_mission_button_;
    NSButton* load_replay_button_;
    NSButton* save_button_;
    NSButton* reset_button_;
    NSSlider* replay_slider_;
    double zoom_m_;
    double pan_x_;
    double pan_z_;
    double trail_sample_accumulator_;
    double record_sample_accumulator_;
    double replay_time_s_;
    std::filesystem::path mission_path_;
    std::filesystem::path replay_path_;
    std::filesystem::path recording_path_;
    std::string status_message_;
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
        chase_camera_ = false;
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
        record_sample_accumulator_ = 0.0;
        replay_time_s_ = 0.0;
        status_message_ = "Ready";
        side_panel_ = nil;
        title_label_ = nil;
        telemetry_label_ = nil;
        mission_label_ = nil;
        message_label_ = nil;
        replay_time_label_ = nil;
        manual_button_ = nil;
        arm_button_ = nil;
        pause_button_ = nil;
        chase_button_ = nil;
        fit_button_ = nil;
        replay_button_ = nil;
        load_mission_button_ = nil;
        load_replay_button_ = nil;
        save_button_ = nil;
        reset_button_ = nil;
        replay_slider_ = nil;

        try {
            mission_path_ = std::filesystem::path([missionPath UTF8String]);
            simulation_ = std::make_unique<DroneSimulation>(MissionLoader::load_from_file(mission_path_));
        } catch (const std::exception& error) {
            std::cerr << "Unable to load mission: " << error.what() << "\n";
            mission_path_ = default_sample_mission_path();
            simulation_ = std::make_unique<DroneSimulation>(MissionLoader::load_from_file(default_sample_mission_path()));
        }
        [self fitMissionCamera];
        [self setupOverlayControls];
        [self startNewRecording];
        [self updatePanelText];

        timer_ = [NSTimer scheduledTimerWithTimeInterval:(1.0 / 60.0)
                                                  target:self
                                                selector:@selector(tick:)
                                                userInfo:nil
                                                 repeats:YES];
    }

    return self;
}

- (void)dealloc {
    [self finishRecording];
    [timer_ invalidate];
    [super dealloc];
}

- (BOOL)acceptsFirstResponder {
    return YES;
}

- (NSRect)sceneRect {
    const NSRect bounds = [self bounds];
    const CGFloat panel_width = 306.0;
    const CGFloat scene_width = std::max<CGFloat>(240.0, bounds.size.width - panel_width);
    return NSMakeRect(0.0, 0.0, scene_width, bounds.size.height);
}

- (void)setupOverlayControls {
    side_panel_ = [[[NSVisualEffectView alloc] initWithFrame:NSZeroRect] autorelease];
    [side_panel_ setBlendingMode:NSVisualEffectBlendingModeWithinWindow];
    [side_panel_ setMaterial:NSVisualEffectMaterialSidebar];
    [side_panel_ setState:NSVisualEffectStateActive];
    [side_panel_ setWantsLayer:YES];
    [side_panel_.layer setCornerRadius:0.0];
    [self addSubview:side_panel_];

    title_label_ = make_label(@"AgBot FlightSim", [NSFont systemFontOfSize:18.0 weight:NSFontWeightSemibold], [NSColor labelColor]);
    telemetry_label_ = make_label(@"", [NSFont monospacedSystemFontOfSize:12.0 weight:NSFontWeightRegular], [NSColor labelColor]);
    mission_label_ = make_label(@"", [NSFont monospacedSystemFontOfSize:11.0 weight:NSFontWeightRegular], [NSColor secondaryLabelColor]);
    message_label_ = make_label(@"Ready", [NSFont systemFontOfSize:12.0 weight:NSFontWeightMedium], [NSColor controlAccentColor]);
    replay_time_label_ = make_label(@"Replay -", [NSFont monospacedSystemFontOfSize:11.0 weight:NSFontWeightRegular], [NSColor secondaryLabelColor]);

    manual_button_ = make_button(@"Manual", self, @selector(toggleManualMode:));
    arm_button_ = make_button(@"Arm", self, @selector(toggleArmAction:));
    pause_button_ = make_button(@"Pause", self, @selector(togglePause:));
    chase_button_ = make_button(@"Chase", self, @selector(toggleChaseCamera:));
    fit_button_ = make_button(@"Fit", self, @selector(fitCameraAction:));
    replay_button_ = make_button(@"Replay", self, @selector(toggleReplayAction:));
    load_mission_button_ = make_button(@"Load Mission", self, @selector(loadMissionAction:));
    load_replay_button_ = make_button(@"Replay File", self, @selector(loadReplayFileAction:));
    save_button_ = make_button(@"Save", self, @selector(saveMissionAction:));
    reset_button_ = make_button(@"Reset", self, @selector(resetMissionAction:));
    replay_slider_ = make_slider(self, @selector(scrubReplay:));

    for (NSView* subview in @[title_label_, telemetry_label_, mission_label_, message_label_,
                              replay_time_label_, replay_slider_,
                              manual_button_, arm_button_, pause_button_, reset_button_,
                              chase_button_, fit_button_, save_button_, load_mission_button_,
                              replay_button_, load_replay_button_]) {
        [side_panel_ addSubview:subview];
    }
}

- (void)layout {
    [super layout];

    if (!side_panel_) {
        return;
    }

    const NSRect bounds = [self bounds];
    const CGFloat panel_width = 306.0;
    [side_panel_ setFrame:NSMakeRect(bounds.size.width - panel_width, 0.0, panel_width, bounds.size.height)];

    CGFloat y = bounds.size.height - 42.0;
    const CGFloat x = 18.0;
    const CGFloat width = panel_width - 36.0;
    [title_label_ setFrame:NSMakeRect(x, y, width, 24.0)];

    y -= 128.0;
    [telemetry_label_ setFrame:NSMakeRect(x, y, width, 118.0)];

    y -= 132.0;
    [mission_label_ setFrame:NSMakeRect(x, y, width, 118.0)];

    y -= 36.0;
    [message_label_ setFrame:NSMakeRect(x, y, width, 24.0)];

    y -= 30.0;
    [replay_time_label_ setFrame:NSMakeRect(x, y, width, 18.0)];

    y -= 24.0;
    [replay_slider_ setFrame:NSMakeRect(x, y, width, 22.0)];

    const CGFloat button_height = 30.0;
    const CGFloat gap = 8.0;
    const CGFloat col_width = (width - gap) * 0.5;
    y -= 44.0;
    [manual_button_ setFrame:NSMakeRect(x, y, col_width, button_height)];
    [arm_button_ setFrame:NSMakeRect(x + col_width + gap, y, col_width, button_height)];

    y -= button_height + gap;
    [pause_button_ setFrame:NSMakeRect(x, y, col_width, button_height)];
    [reset_button_ setFrame:NSMakeRect(x + col_width + gap, y, col_width, button_height)];

    y -= button_height + gap;
    [chase_button_ setFrame:NSMakeRect(x, y, col_width, button_height)];
    [fit_button_ setFrame:NSMakeRect(x + col_width + gap, y, col_width, button_height)];

    y -= button_height + gap;
    [save_button_ setFrame:NSMakeRect(x, y, col_width, button_height)];
    [load_mission_button_ setFrame:NSMakeRect(x + col_width + gap, y, col_width, button_height)];

    y -= button_height + gap;
    [replay_button_ setFrame:NSMakeRect(x, y, col_width, button_height)];
    [load_replay_button_ setFrame:NSMakeRect(x + col_width + gap, y, col_width, button_height)];
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

- (void)writeTelemetrySample:(const DroneState&)state {
    try {
        if (run_recorder_) {
            run_recorder_->write_sample(state);
        }
        if (latest_recorder_) {
            latest_recorder_->write_sample(state);
        }
    } catch (const std::exception& error) {
        std::cerr << "Telemetry recording failed: " << error.what() << "\n";
        run_recorder_.reset();
        latest_recorder_.reset();
        recording_path_.clear();
        [self setStatusMessage:"Recording failed"];
    }
}

- (void)startNewRecording {
    if (!simulation_) {
        return;
    }

    run_recorder_.reset();
    latest_recorder_.reset();
    record_sample_accumulator_ = 0.0;
    recording_path_ = telemetry_run_path();

    try {
        run_recorder_ = std::make_unique<TelemetryRecorder>(recording_path_);
        latest_recorder_ = std::make_unique<TelemetryRecorder>(telemetry_latest_path());
        [self writeTelemetrySample:simulation_->state()];
        [self setStatusMessage:"Recording flight"];
    } catch (const std::exception& error) {
        std::cerr << "Unable to start telemetry recording: " << error.what() << "\n";
        run_recorder_.reset();
        latest_recorder_.reset();
        recording_path_.clear();
        [self setStatusMessage:"Recording unavailable"];
    }
}

- (void)finishRecording {
    if (simulation_ && (run_recorder_ || latest_recorder_)) {
        [self writeTelemetrySample:simulation_->state()];
    }
    run_recorder_.reset();
    latest_recorder_.reset();
}

- (void)recordLiveTelemetry:(double)dt_s {
    if (!simulation_ || replay_mode_ || (!run_recorder_ && !latest_recorder_)) {
        return;
    }

    record_sample_accumulator_ += dt_s;
    if (record_sample_accumulator_ >= 0.25 || simulation_->is_complete()) {
        [self writeTelemetrySample:simulation_->state()];
        record_sample_accumulator_ = 0.0;
    }

    if (simulation_->is_complete()) {
        [self finishRecording];
    }
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
        [self recordLiveTelemetry:dt_s];
        trail_sample_accumulator_ += dt_s;

        if (trail_.empty() || trail_sample_accumulator_ >= 0.2) {
            trail_.push_back(simulation_->state().position);
            trail_sample_accumulator_ = 0.0;
        }
    }

    [self updateWindowTitle];
    [self updatePanelText];
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

- (void)setStatusMessage:(std::string)message {
    status_message_ = std::move(message);
    [self updatePanelText];
}

- (void)updatePanelText {
    if (!simulation_ || !telemetry_label_) {
        return;
    }

    const DroneState& state = [self displayState];
    const double speed = state.velocity.length();
    const std::size_t waypoint_count = simulation_->mission().waypoints.size();
    const std::size_t waypoint_index = std::min(state.target_waypoint_index + 1, waypoint_count);

    std::ostringstream telemetry;
    telemetry << std::fixed << std::setprecision(1)
              << "Mode      " << (replay_mode_ ? "replay" : to_string(simulation_->control_mode())) << "\n"
              << "Flight    " << to_string(state.mode) << "\n"
              << "Armed     " << (state.armed ? "yes" : "no") << "\n"
              << "Speed     " << speed << " m/s\n"
              << "Altitude  " << state.position.y << " m\n"
              << "Battery   " << std::setprecision(0) << state.battery_percent << "%\n"
              << std::setprecision(1)
              << "Time      " << state.mission_time_s << " s\n"
              << "Camera    " << (chase_camera_ ? "chase" : "map");
    [telemetry_label_ setStringValue:ns_string(telemetry.str())];

    std::ostringstream mission;
    mission << "Mission   " << simulation_->mission().name << "\n"
            << "Waypoint  " << waypoint_index << " / " << waypoint_count << "\n"
            << "Selected  ";
    if (selected_waypoint_ >= 0) {
        mission << (selected_waypoint_ + 1);
    } else {
        mission << "-";
    }
    mission << "\n"
            << "Path      " << mission_path_.filename().string() << "\n"
            << "Edited    " << std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR)
                   .append("out").append("edited_mission.json").filename().string() << "\n"
            << "Record    ";
    if (!recording_path_.empty()) {
        mission << recording_path_.filename().string();
    } else {
        mission << "-";
    }
    [mission_label_ setStringValue:ns_string(mission.str())];

    const double replay_duration = replay_ ? replay_->duration_s() : 0.0;
    std::ostringstream replay_time;
    replay_time << std::fixed << std::setprecision(1)
                << "Replay   " << replay_time_s_ << " / " << replay_duration << " s";
    if (!replay_path_.empty()) {
        replay_time << "  " << replay_path_.filename().string();
    }
    [replay_time_label_ setStringValue:ns_string(replay_time.str())];
    [replay_slider_ setMinValue:0.0];
    [replay_slider_ setMaxValue:std::max(1.0, replay_duration)];
    [replay_slider_ setDoubleValue:std::clamp(replay_time_s_, 0.0, std::max(1.0, replay_duration))];
    [replay_slider_ setEnabled:(replay_ && !replay_->empty())];

    [message_label_ setStringValue:ns_string(status_message_)];
    [manual_button_ setTitle:(manual_mode_ ? @"Autopilot" : @"Manual")];
    [arm_button_ setTitle:(state.armed ? @"Disarm" : @"Arm")];
    [pause_button_ setTitle:(paused_ ? @"Resume" : @"Pause")];
    [chase_button_ setTitle:(chase_camera_ ? @"Map" : @"Chase")];
    [replay_button_ setTitle:(replay_mode_ ? @"Live" : @"Replay")];
}

- (void)toggleManualMode:(id)sender {
    (void)sender;
    if (!simulation_) {
        return;
    }
    replay_mode_ = false;
    manual_mode_ = !manual_mode_;
    simulation_->set_control_mode(manual_mode_ ? ControlMode::Manual : ControlMode::Autopilot);
    if (manual_mode_) {
        [self setStatusMessage:"Manual enabled; arm to fly"];
    } else {
        [self setStatusMessage:"Autopilot enabled"];
    }
    [[self window] makeFirstResponder:self];
}

- (void)toggleArmAction:(id)sender {
    (void)sender;
    if (!simulation_ || replay_mode_) {
        return;
    }

    if (!manual_mode_) {
        manual_mode_ = true;
        simulation_->set_control_mode(ControlMode::Manual);
    }

    if (simulation_->state().armed) {
        simulation_->disarm();
        [self setStatusMessage:"Disarmed"];
    } else {
        simulation_->arm();
        [self setStatusMessage:"Armed"];
    }
    [[self window] makeFirstResponder:self];
}

- (void)togglePause:(id)sender {
    (void)sender;
    paused_ = !paused_;
    [self setStatusMessage:(paused_ ? "Paused" : "Running")];
    [[self window] makeFirstResponder:self];
}

- (void)toggleChaseCamera:(id)sender {
    (void)sender;
    chase_camera_ = !chase_camera_;
    [self setStatusMessage:(chase_camera_ ? "Chase camera" : "Mission map camera")];
    [[self window] makeFirstResponder:self];
}

- (void)fitCameraAction:(id)sender {
    (void)sender;
    [self fitMissionCamera];
    [self setStatusMessage:"Mission fitted"];
    [[self window] makeFirstResponder:self];
}

- (void)toggleReplayAction:(id)sender {
    (void)sender;
    if (replay_mode_) {
        replay_mode_ = false;
        if (simulation_) {
            simulation_->set_control_mode(manual_mode_ ? ControlMode::Manual : ControlMode::Autopilot);
        }
        [self setStatusMessage:"Live simulation"];
    } else {
        [self loadReplay];
    }
    [[self window] makeFirstResponder:self];
}

- (void)saveMissionAction:(id)sender {
    (void)sender;
    [self saveMission];
    [[self window] makeFirstResponder:self];
}

- (void)resetMissionAction:(id)sender {
    (void)sender;
    if (!simulation_) {
        return;
    }
    if (replay_mode_) {
        replay_time_s_ = 0.0;
        paused_ = false;
        trail_.clear();
        [self rebuildReplayTrail];
    } else {
        [self finishRecording];
        simulation_->reset();
        if (manual_mode_) {
            simulation_->set_control_mode(ControlMode::Manual);
        }
        trail_.clear();
        [self startNewRecording];
    }
    [self setStatusMessage:"Reset"];
    [[self window] makeFirstResponder:self];
}

- (void)loadMissionFromPath:(std::filesystem::path)path {
    if (!simulation_) {
        return;
    }

    try {
        auto mission = MissionLoader::load_from_file(path);
        [self finishRecording];
        simulation_->replace_mission(std::move(mission));
        mission_path_ = std::move(path);
        replay_.reset();
        replay_path_.clear();
        replay_mode_ = false;
        manual_mode_ = false;
        selected_waypoint_ = -1;
        dragging_waypoint_ = false;
        replay_time_s_ = 0.0;
        trail_.clear();
        simulation_->set_control_mode(ControlMode::Autopilot);
        [self fitMissionCamera];
        [self startNewRecording];
        [self setStatusMessage:"Mission loaded"];
    } catch (const std::exception& error) {
        std::cerr << "Failed to load mission: " << error.what() << "\n";
        [self setStatusMessage:"Mission load failed"];
    }
}

- (void)loadMissionAction:(id)sender {
    (void)sender;
    NSOpenPanel* panel = [NSOpenPanel openPanel];
    [panel setAllowsMultipleSelection:NO];
    [panel setCanChooseDirectories:NO];
    [panel setCanChooseFiles:YES];
    [panel setAllowedFileTypes:@[@"json"]];
    if ([panel runModal] == NSModalResponseOK) {
        NSURL* url = [[panel URLs] firstObject];
        if (url) {
            [self loadMissionFromPath:std::filesystem::path([[url path] UTF8String])];
        }
    }
    [[self window] makeFirstResponder:self];
}

- (void)rebuildReplayTrail {
    trail_.clear();
    if (!replay_ || replay_->empty()) {
        return;
    }

    double next_sample_s = 0.0;
    for (const auto& frame : replay_->frames()) {
        if (frame.state.mission_time_s > replay_time_s_) {
            break;
        }
        if (frame.state.mission_time_s + 1e-6 >= next_sample_s) {
            trail_.push_back(frame.state.position);
            next_sample_s += 0.2;
        }
    }

    const Vec3 current_position = replay_->sample(replay_time_s_).position;
    if (trail_.empty() || (trail_.back() - current_position).length() > 1e-6) {
        trail_.push_back(current_position);
    }
}

- (void)loadReplayFromPath:(std::filesystem::path)path {
    try {
        [self finishRecording];
        replay_ = std::make_unique<TelemetryReplay>(TelemetryReplay::load_jsonl(path));
        replay_path_ = std::move(path);
        replay_mode_ = replay_ && !replay_->empty();
        replay_time_s_ = 0.0;
        paused_ = false;
        trail_sample_accumulator_ = 0.0;
        [self rebuildReplayTrail];
        if (simulation_) {
            simulation_->set_control_mode(ControlMode::Replay);
        }
        std::cout << "Loaded telemetry replay: " << replay_path_ << "\n";
        [self setStatusMessage:"Telemetry replay loaded"];
    } catch (const std::exception& error) {
        std::cerr << "Failed to load replay: " << error.what() << "\n";
        [self setStatusMessage:"Replay load failed"];
    }
}

- (void)loadReplayFileAction:(id)sender {
    (void)sender;
    NSOpenPanel* panel = [NSOpenPanel openPanel];
    [panel setAllowsMultipleSelection:NO];
    [panel setCanChooseDirectories:NO];
    [panel setCanChooseFiles:YES];
    [panel setDirectoryURL:[NSURL fileURLWithPath:ns_string(telemetry_runs_dir())]];
    [panel setAllowedFileTypes:@[@"jsonl", @"json"]];
    if ([panel runModal] == NSModalResponseOK) {
        NSURL* url = [[panel URLs] firstObject];
        if (url) {
            [self loadReplayFromPath:std::filesystem::path([[url path] UTF8String])];
        }
    }
    [[self window] makeFirstResponder:self];
}

- (void)scrubReplay:(id)sender {
    (void)sender;
    if (!replay_ || replay_->empty()) {
        return;
    }

    replay_time_s_ = std::clamp([replay_slider_ doubleValue], 0.0, replay_->duration_s());
    replay_mode_ = true;
    paused_ = true;
    if (simulation_) {
        simulation_->set_control_mode(ControlMode::Replay);
    }
    [self rebuildReplayTrail];
    [self setStatusMessage:"Replay scrubbed"];
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

- (ViewBounds)viewBoundsForRect:(NSRect)bounds {
    const NSPoint center = [self viewCenter];
    const double aspect = std::max(0.1, bounds.size.width / std::max(1.0, bounds.size.height));
    const double half_height = zoom_m_;
    const double half_width = zoom_m_ * aspect;
    return {
        center.x - half_width,
        center.x + half_width,
        center.y - half_height,
        center.y + half_height,
    };
}

- (void)fitMissionCamera {
    if (!simulation_) {
        return;
    }

    const auto& mission = simulation_->mission();
    double min_x = mission.home.x;
    double max_x = mission.home.x;
    double min_z = mission.home.z;
    double max_z = mission.home.z;

    for (const Waypoint& waypoint : mission.waypoints) {
        min_x = std::min(min_x, waypoint.position.x);
        max_x = std::max(max_x, waypoint.position.x);
        min_z = std::min(min_z, waypoint.position.z);
        max_z = std::max(max_z, waypoint.position.z);
    }

    for (const Vec3& point : trail_) {
        min_x = std::min(min_x, point.x);
        max_x = std::max(max_x, point.x);
        min_z = std::min(min_z, point.z);
        max_z = std::max(max_z, point.z);
    }

    const NSRect bounds = [self sceneRect];
    const double aspect = std::max(0.1, bounds.size.width / std::max(1.0, bounds.size.height));
    const double width = std::max(80.0, max_x - min_x);
    const double height = std::max(80.0, max_z - min_z);
    const double padding = 45.0;

    pan_x_ = (min_x + max_x) * 0.5;
    pan_z_ = (min_z + max_z) * 0.5;
    zoom_m_ = std::max((height * 0.5) + padding, ((width * 0.5) + padding) / aspect);
    zoom_m_ = std::clamp(zoom_m_, 80.0, 1600.0);
    chase_camera_ = false;
}

- (Vec3)worldPointFromEvent:(NSEvent*)event altitude:(double)altitude {
    const NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    const NSRect bounds = [self sceneRect];
    const NSPoint center = [self viewCenter];
    const double aspect = std::max(0.1, bounds.size.width / std::max(1.0, bounds.size.height));
    const double half_height = zoom_m_;
    const double half_width = zoom_m_ * aspect;
    const double x = center.x - half_width + ((location.x - bounds.origin.x) / std::max(1.0, bounds.size.width)) * half_width * 2.0;
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
        [self setStatusMessage:"Saved edited_mission.json"];
    } catch (const std::exception& error) {
        std::cerr << "Failed to save mission: " << error.what() << "\n";
        [self setStatusMessage:"Save failed"];
    }
}

- (void)loadReplay {
    [self loadReplayFromPath:telemetry_latest_path()];
}

- (void)updateManualInput {
    if (!simulation_ || !manual_mode_ || replay_mode_) {
        return;
    }

    ManualControlInput input;
    input.pitch = (key_w_ ? 1.0 : 0.0) + (key_s_ ? -1.0 : 0.0);
    input.roll = (key_d_ ? 1.0 : 0.0) + (key_a_ ? -1.0 : 0.0);
    input.yaw = (key_e_ ? 1.0 : 0.0) + (key_q_ ? -1.0 : 0.0);
    input.throttle = (key_up_ ? 1.0 : 0.0) + (key_down_ ? -1.0 : 0.0);
    input.takeoff = key_t_;
    input.land = key_l_;
    simulation_->set_manual_input(input);
}

- (void)drawGrid:(ViewBounds)viewBounds {
    const double major_step = 100.0;
    const double minor_step = 20.0;
    const double min_x = std::floor(viewBounds.min_x / minor_step) * minor_step;
    const double max_x = std::ceil(viewBounds.max_x / minor_step) * minor_step;
    const double min_z = std::floor(viewBounds.min_z / minor_step) * minor_step;
    const double max_z = std::ceil(viewBounds.max_z / minor_step) * minor_step;

    set_color(0.16, 0.18, 0.21, 1.0);
    glLineWidth(1.0f);
    glBegin(GL_LINES);
    for (double x = min_x; x <= max_x; x += minor_step) {
        glVertex2d(x, min_z);
        glVertex2d(x, max_z);
    }
    for (double z = min_z; z <= max_z; z += minor_step) {
        glVertex2d(min_x, z);
        glVertex2d(max_x, z);
    }
    glEnd();

    set_color(0.24, 0.28, 0.34, 1.0);
    glLineWidth(1.4f);
    glBegin(GL_LINES);
    for (double x = std::floor(viewBounds.min_x / major_step) * major_step; x <= max_x; x += major_step) {
        glVertex2d(x, min_z);
        glVertex2d(x, max_z);
    }
    for (double z = std::floor(viewBounds.min_z / major_step) * major_step; z <= max_z; z += major_step) {
        glVertex2d(min_x, z);
        glVertex2d(max_x, z);
    }
    glEnd();

    set_color(0.48, 0.55, 0.66, 1.0);
    glLineWidth(2.0f);
    glBegin(GL_LINES);
    glVertex2d(min_x, 0);
    glVertex2d(max_x, 0);
    glVertex2d(0, min_z);
    glVertex2d(0, max_z);
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
    const NSRect scene = [self sceneRect];
    glViewport(0, 0, static_cast<GLsizei>(scene.size.width), static_cast<GLsizei>(scene.size.height));
    glClear(GL_COLOR_BUFFER_BIT);

    const ViewBounds viewBounds = [self viewBoundsForRect:scene];

    glMatrixMode(GL_PROJECTION);
    glLoadIdentity();
    glOrtho(viewBounds.min_x, viewBounds.max_x, viewBounds.min_z, viewBounds.max_z, -1.0, 1.0);
    glMatrixMode(GL_MODELVIEW);
    glLoadIdentity();

    [self drawGrid:viewBounds];
    [self drawMissionPath];
    [self drawTrail];
    [self drawDrone];
    [self drawHud:scene];

    [[self openGLContext] flushBuffer];
}

- (void)keyDown:(NSEvent*)event {
    if (([event modifierFlags] & NSEventModifierFlagCommand) != 0) {
        NSString* commandCharacters = [[event charactersIgnoringModifiers] lowercaseString];
        if ([commandCharacters length] > 0 && [commandCharacters characterAtIndex:0] == 's') {
            [self saveMission];
            return;
        }
        if ([commandCharacters length] > 0 && [commandCharacters characterAtIndex:0] == 'o') {
            [self loadMissionAction:nil];
            return;
        }
    }

    NSString* characters = [[event charactersIgnoringModifiers] lowercaseString];
    if ([characters length] > 0) {
        const unichar key = [characters characterAtIndex:0];
        if (key == ' ') {
            [self togglePause:nil];
            return;
        }
        if (key == 'r' && simulation_) {
            [self resetMissionAction:nil];
            return;
        }
        if (key == 'm' && simulation_) {
            [self toggleManualMode:nil];
            return;
        }
        if (key == 'x' && simulation_) {
            [self toggleArmAction:nil];
            return;
        }
        if (key == 'g') {
            [self toggleReplayAction:nil];
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
            [self toggleChaseCamera:nil];
            return;
        }
        if (key == 'f') {
            [self fitCameraAction:nil];
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

    const NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    if (location.x > [self sceneRect].size.width) {
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
