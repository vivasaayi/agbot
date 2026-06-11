#import <Cocoa/Cocoa.h>
#import <OpenGL/gl.h>

#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_flight_sim/TelemetryRecorder.hpp"
#include "agbot_flight_sim/TelemetryReplay.hpp"

#include <chrono>
#include <cmath>
#include <cstdint>
#include <ctime>
#include <filesystem>
#include <iomanip>
#include <iostream>
#include <memory>
#include <optional>
#include <algorithm>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

using agbot::flight_sim::DroneSimulation;
using agbot::flight_sim::DroneState;
using agbot::flight_sim::GeoCoordinate;
using agbot::flight_sim::GeoBounds;
using agbot::flight_sim::ControlMode;
using agbot::flight_sim::ElevationTile;
using agbot::flight_sim::ManualControlInput;
using agbot::flight_sim::Mission;
using agbot::flight_sim::MissionLoader;
using agbot::flight_sim::TerrainMesh;
using agbot::flight_sim::TelemetryRecorder;
using agbot::flight_sim::TelemetryReplay;
using agbot::flight_sim::TileCoordinate;
using agbot::flight_sim::Vec3;
using agbot::flight_sim::Waypoint;
using agbot::flight_sim::WaypointAction;
using agbot::flight_sim::default_sample_mission_path;
using agbot::flight_sim::build_terrain_mesh;
using agbot::flight_sim::composite_elevation_with_state;
using agbot::flight_sim::elevation_tile_from_terrarium_rgba;
using agbot::flight_sim::geo_from_local;
using agbot::flight_sim::local_from_geo;
using agbot::flight_sim::radius_m_for_area_km2;
using agbot::flight_sim::tile_for_geo;
using agbot::flight_sim::tiles_for_bounds;
using agbot::flight_sim::to_string;
using agbot::flight_sim::zoom_for_radius_m;

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

struct MapTile {
    GLuint texture_id = 0;
    double min_x = 0.0;
    double max_x = 0.0;
    double min_z = 0.0;
    double max_z = 0.0;
};

struct GlobeMapTile {
    GLuint texture_id = 0;
    GeoBounds bounds;
};

struct GlobePoint {
    double x = 0.0;
    double y = 0.0;
    double depth = 0.0;
    bool visible = false;
};

constexpr int kGlobeMapMinZoom = 3;
constexpr int kGlobeMapMaxZoom = 17;
constexpr int kMaxGlobeTileLoads = 128;
constexpr double kMinGlobeViewZoom = 0.75;
constexpr double kMaxGlobeViewZoom = 16384.0;

std::filesystem::path map_tile_cache_path(int zoom, int x, int y) {
    return std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR)
        / "out" / "map_tiles" / std::to_string(zoom) / std::to_string(x) / (std::to_string(y) + ".png");
}

std::filesystem::path elevation_tile_cache_path(int zoom, int x, int y) {
    return std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR)
        / "out" / "elevation_tiles" / std::to_string(zoom) / std::to_string(x) / (std::to_string(y) + ".png");
}

NSData* data_for_url_cached(const std::filesystem::path& cache_path, const std::string& url_text) {
    if (std::filesystem::exists(cache_path)) {
        return [NSData dataWithContentsOfFile:ns_string(cache_path)];
    }

    NSURL* url = [NSURL URLWithString:ns_string(url_text)];
    NSMutableURLRequest* request = [NSMutableURLRequest requestWithURL:url];
    [request setValue:@"AgBotFlightSim/0.1 local simulator" forHTTPHeaderField:@"User-Agent"];
    [request setTimeoutInterval:5.0];

    NSError* error = nil;
    NSHTTPURLResponse* response = nil;
    NSData* data = [NSURLConnection sendSynchronousRequest:request returningResponse:&response error:&error];
    if (!data || error || [response statusCode] >= 400) {
        return nil;
    }

    std::filesystem::create_directories(cache_path.parent_path());
    [data writeToFile:ns_string(cache_path) atomically:YES];
    return data;
}

NSData* data_for_tile(int zoom, int x, int y) {
    std::ostringstream url_text;
    url_text << "https://tile.openstreetmap.org/" << zoom << "/" << x << "/" << y << ".png";
    return data_for_url_cached(map_tile_cache_path(zoom, x, y), url_text.str());
}

NSData* data_for_elevation_tile(int zoom, int x, int y) {
    std::ostringstream url_text;
    url_text << "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/" << zoom << "/" << x << "/" << y << ".png";
    return data_for_url_cached(elevation_tile_cache_path(zoom, x, y), url_text.str());
}

void push_unique_tile(std::vector<TileCoordinate>& tiles, TileCoordinate tile) {
    for (const TileCoordinate& existing : tiles) {
        if (existing.z == tile.z && existing.x == tile.x && existing.y == tile.y) {
            return;
        }
    }
    tiles.push_back(tile);
}

std::string key_for_tiles(const std::vector<TileCoordinate>& tiles) {
    std::ostringstream key;
    for (const TileCoordinate& tile : tiles) {
        key << tile.z << ":" << tile.x << ":" << tile.y << ";";
    }
    return key.str();
}

std::vector<TileCoordinate> globe_tiles_for_view(
    int zoom,
    double center_latitude,
    double center_longitude,
    double view_zoom) {
    const int tile_count = 1 << zoom;
    std::vector<TileCoordinate> tiles;

    if (zoom <= 4) {
        tiles.reserve(static_cast<std::size_t>(tile_count * tile_count));
        for (int y = 0; y < tile_count; ++y) {
            for (int x = 0; x < tile_count; ++x) {
                tiles.push_back({zoom, x, y});
            }
        }
        return tiles;
    }

    const GeoCoordinate center {
        std::clamp(center_latitude, -85.0, 85.0),
        std::clamp(center_longitude, -180.0, 180.0),
        0.0,
    };
    const TileCoordinate center_tile = tile_for_geo(center, zoom);
    const double visible_fraction = std::clamp(1.0 / std::max(0.78, 0.78 * view_zoom), 0.00002, 0.995);
    const double angular_radius_deg = std::asin(visible_fraction) * 180.0 / M_PI * 1.25;
    const double lat_cos = std::max(0.10, std::abs(std::cos(center.latitude * M_PI / 180.0)));
    const double tile_width_deg = 360.0 / static_cast<double>(tile_count);
    const int radius_x = std::max(1, static_cast<int>(std::ceil((angular_radius_deg / lat_cos) / tile_width_deg)) + 1);
    const TileCoordinate north_tile = tile_for_geo({
        std::clamp(center.latitude + angular_radius_deg, -85.0, 85.0),
        center.longitude,
        0.0,
    }, zoom);
    const TileCoordinate south_tile = tile_for_geo({
        std::clamp(center.latitude - angular_radius_deg, -85.0, 85.0),
        center.longitude,
        0.0,
    }, zoom);
    const int radius_y = std::max(
        1,
        std::max(std::abs(center_tile.y - north_tile.y), std::abs(center_tile.y - south_tile.y)) + 1
    );

    tiles.reserve(static_cast<std::size_t>((radius_x * 2 + 1) * (radius_y * 2 + 1)));
    for (int dy = -radius_y; dy <= radius_y; ++dy) {
        const int y = std::clamp(center_tile.y + dy, 0, tile_count - 1);
        for (int dx = -radius_x; dx <= radius_x; ++dx) {
            int x = (center_tile.x + dx) % tile_count;
            if (x < 0) {
                x += tile_count;
            }
            push_unique_tile(tiles, {zoom, x, y});
        }
    }

    return tiles;
}

bool rgba_pixels_from_image_data(NSData* data, std::vector<std::uint8_t>& pixels, int& width, int& height) {
    if (!data) {
        return false;
    }

    NSImage* image = [[NSImage alloc] initWithData:data];
    if (!image) {
        return false;
    }

    CGImageRef image_ref = [image CGImageForProposedRect:nullptr context:nil hints:nil];
    if (!image_ref) {
        [image release];
        return false;
    }

    width = static_cast<int>(CGImageGetWidth(image_ref));
    height = static_cast<int>(CGImageGetHeight(image_ref));
    pixels.assign(static_cast<std::size_t>(width * height * 4), 0);

    CGColorSpaceRef color_space = CGColorSpaceCreateDeviceRGB();
    CGContextRef context = CGBitmapContextCreate(
        pixels.data(),
        static_cast<std::size_t>(width),
        static_cast<std::size_t>(height),
        8,
        static_cast<std::size_t>(width * 4),
        color_space,
        static_cast<CGBitmapInfo>(kCGImageAlphaPremultipliedLast) | kCGBitmapByteOrder32Big
    );

    if (!context) {
        CGColorSpaceRelease(color_space);
        [image release];
        return false;
    }

    CGContextDrawImage(context, CGRectMake(0.0, 0.0, width, height), image_ref);
    CGContextRelease(context);
    CGColorSpaceRelease(color_space);
    [image release];
    return true;
}

GLuint texture_from_image_data(NSData* data) {
    if (!data) {
        return 0;
    }

    NSImage* image = [[NSImage alloc] initWithData:data];
    if (!image) {
        return 0;
    }

    CGImageRef image_ref = [image CGImageForProposedRect:nullptr context:nil hints:nil];
    if (!image_ref) {
        [image release];
        return 0;
    }

    const std::size_t width = CGImageGetWidth(image_ref);
    const std::size_t height = CGImageGetHeight(image_ref);
    std::vector<unsigned char> pixels(width * height * 4);
    CGColorSpaceRef color_space = CGColorSpaceCreateDeviceRGB();
    CGContextRef context = CGBitmapContextCreate(
        pixels.data(),
        width,
        height,
        8,
        width * 4,
        color_space,
        static_cast<CGBitmapInfo>(kCGImageAlphaPremultipliedLast) | kCGBitmapByteOrder32Big
    );

    if (!context) {
        CGColorSpaceRelease(color_space);
        [image release];
        return 0;
    }

    CGContextDrawImage(context, CGRectMake(0.0, 0.0, width, height), image_ref);

    GLuint texture_id = 0;
    glGenTextures(1, &texture_id);
    glBindTexture(GL_TEXTURE_2D, texture_id);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glTexImage2D(
        GL_TEXTURE_2D,
        0,
        GL_RGBA,
        static_cast<GLsizei>(width),
        static_cast<GLsizei>(height),
        0,
        GL_RGBA,
        GL_UNSIGNED_BYTE,
        pixels.data()
    );
    glBindTexture(GL_TEXTURE_2D, 0);

    CGContextRelease(context);
    CGColorSpaceRelease(color_space);
    [image release];
    return texture_id;
}

Mission mission_for_location(GeoCoordinate center, double area_km2) {
    const double radius_m = radius_m_for_area_km2(area_km2);
    const double half_extent_m = radius_m / std::sqrt(2.0);

    Mission mission;
    std::ostringstream name;
    name << "Location " << std::fixed << std::setprecision(6)
         << center.latitude << ", " << center.longitude;
    mission.name = name.str();
    mission.home = Vec3(0.0, 0.0, 0.0);
    mission.home_geo = center;
    mission.cruise_speed_mps = 12.0;
    mission.acceptance_radius_m = 3.0;

    const std::vector<std::pair<std::string, Vec3>> points = {
        {"takeoff", Vec3(0.0, 30.0, 0.0)},
        {"north_west", Vec3(-half_extent_m, 30.0, half_extent_m)},
        {"north_east", Vec3(half_extent_m, 30.0, half_extent_m)},
        {"south_east", Vec3(half_extent_m, 30.0, -half_extent_m)},
        {"south_west", Vec3(-half_extent_m, 30.0, -half_extent_m)},
        {"land", Vec3(0.0, 0.0, 0.0)},
    };

    for (const auto& [waypoint_name, position] : points) {
        Waypoint waypoint;
        waypoint.name = waypoint_name;
        waypoint.position = position;
        waypoint.geo = geo_from_local(position, center);
        waypoint.speed_mps = mission.cruise_speed_mps;
        if (waypoint_name == "takeoff") {
            waypoint.action = WaypointAction::Takeoff;
        } else if (waypoint_name == "land") {
            waypoint.action = WaypointAction::Land;
        } else {
            waypoint.action = WaypointAction::FlyThrough;
        }
        mission.waypoints.push_back(waypoint);
    }

    return mission;
}

GlobePoint project_globe_point(double latitude, double longitude, double center_latitude, double center_longitude) {
    const double lat = latitude * M_PI / 180.0;
    const double lon = (longitude - center_longitude) * M_PI / 180.0;
    const double center_lat = center_latitude * M_PI / 180.0;

    const double cos_lat = std::cos(lat);
    const double x = cos_lat * std::sin(lon);
    const double y = std::cos(center_lat) * std::sin(lat) -
        std::sin(center_lat) * cos_lat * std::cos(lon);
    const double depth = std::sin(center_lat) * std::sin(lat) +
        std::cos(center_lat) * cos_lat * std::cos(lon);

    return {x, y, depth, depth >= 0.0};
}

Vec3 cross(Vec3 lhs, Vec3 rhs) {
    return {
        lhs.y * rhs.z - lhs.z * rhs.y,
        lhs.z * rhs.x - lhs.x * rhs.z,
        lhs.x * rhs.y - lhs.y * rhs.x,
    };
}

void apply_look_at(Vec3 eye, Vec3 center, Vec3 up) {
    const Vec3 forward = (center - eye).normalized();
    const Vec3 side = cross(forward, up).normalized();
    const Vec3 corrected_up = cross(side, forward);

    const GLdouble matrix[16] = {
        side.x, corrected_up.x, -forward.x, 0.0,
        side.y, corrected_up.y, -forward.y, 0.0,
        side.z, corrected_up.z, -forward.z, 0.0,
        0.0, 0.0, 0.0, 1.0,
    };
    glMultMatrixd(matrix);
    glTranslated(-eye.x, -eye.y, -eye.z);
}

} // namespace

@interface FlightSimOpenGLView : NSOpenGLView {
    std::unique_ptr<DroneSimulation> simulation_;
    std::unique_ptr<TelemetryReplay> replay_;
    std::unique_ptr<TelemetryRecorder> run_recorder_;
    std::unique_ptr<TelemetryRecorder> latest_recorder_;
    std::vector<Vec3> trail_;
    std::vector<MapTile> map_tiles_;
    std::vector<GlobeMapTile> globe_map_tiles_;
    TerrainMesh terrain_mesh_;
    NSTimer* timer_;
    bool paused_;
    bool chase_camera_;
    bool manual_mode_;
    bool replay_mode_;
    bool globe_mode_;
    bool dragging_globe_;
    bool terrain_3d_mode_;
    bool dragging_3d_camera_;
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
    NSTextField* globe_overlay_label_;
    NSButton* manual_button_;
    NSButton* arm_button_;
    NSButton* pause_button_;
    NSButton* chase_button_;
    NSButton* fit_button_;
    NSButton* globe_button_;
    NSButton* three_d_button_;
    NSButton* replay_button_;
    NSButton* load_mission_button_;
    NSButton* load_replay_button_;
    NSButton* location_button_;
    NSButton* save_button_;
    NSButton* reset_button_;
    NSSlider* replay_slider_;
    double zoom_m_;
    double pan_x_;
    double pan_z_;
    double trail_sample_accumulator_;
    double record_sample_accumulator_;
    double replay_time_s_;
    double real_world_area_km2_;
    double globe_view_zoom_;
    int globe_map_zoom_;
    std::string globe_map_key_;
    double globe_center_latitude_;
    double globe_center_longitude_;
    std::optional<GeoCoordinate> globe_hover_coordinate_;
    NSPoint globe_drag_start_;
    double globe_drag_start_latitude_;
    double globe_drag_start_longitude_;
    NSPoint terrain3d_drag_start_;
    double terrain3d_yaw_rad_;
    double terrain3d_pitch_rad_;
    double terrain3d_distance_m_;
    double terrain3d_drag_start_yaw_;
    double terrain3d_drag_start_pitch_;
    std::filesystem::path mission_path_;
    std::filesystem::path replay_path_;
    std::filesystem::path recording_path_;
    std::string status_message_;
    std::string map_status_;
    std::string terrain_status_;
    std::string globe_map_status_;
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
    [self setWantsBestResolutionOpenGLSurface:YES];
    [pixelFormat release];

    if (self) {
        paused_ = false;
        chase_camera_ = false;
        manual_mode_ = false;
        replay_mode_ = false;
        globe_mode_ = false;
        dragging_globe_ = false;
        terrain_3d_mode_ = false;
        dragging_3d_camera_ = false;
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
        real_world_area_km2_ = 20.0;
        globe_view_zoom_ = 1.0;
        globe_map_zoom_ = 0;
        globe_center_latitude_ = 20.0;
        globe_center_longitude_ = 0.0;
        globe_hover_coordinate_.reset();
        terrain3d_yaw_rad_ = -0.72;
        terrain3d_pitch_rad_ = 0.78;
        terrain3d_distance_m_ = 4200.0;
        terrain3d_drag_start_yaw_ = terrain3d_yaw_rad_;
        terrain3d_drag_start_pitch_ = terrain3d_pitch_rad_;
        status_message_ = "Ready";
        map_status_ = "Map off";
        terrain_status_ = "Terrain off";
        globe_map_status_ = "World map off";
        side_panel_ = nil;
        title_label_ = nil;
        telemetry_label_ = nil;
        mission_label_ = nil;
        message_label_ = nil;
        replay_time_label_ = nil;
        globe_overlay_label_ = nil;
        manual_button_ = nil;
        arm_button_ = nil;
        pause_button_ = nil;
        chase_button_ = nil;
        fit_button_ = nil;
        globe_button_ = nil;
        three_d_button_ = nil;
        replay_button_ = nil;
        load_mission_button_ = nil;
        load_replay_button_ = nil;
        location_button_ = nil;
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
    [self clearMapTiles];
    [self clearGlobeMapTiles];
    [timer_ invalidate];
    [super dealloc];
}

- (BOOL)acceptsFirstResponder {
    return YES;
}

- (void)viewDidMoveToWindow {
    [super viewDidMoveToWindow];
    [[self window] setAcceptsMouseMovedEvents:YES];
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
    globe_overlay_label_ = make_label(@"", [NSFont monospacedSystemFontOfSize:12.0 weight:NSFontWeightMedium], [NSColor colorWithCalibratedWhite:0.92 alpha:1.0]);
    [globe_overlay_label_ setWantsLayer:YES];
    [globe_overlay_label_ setDrawsBackground:YES];
    [globe_overlay_label_ setBackgroundColor:[NSColor colorWithCalibratedWhite:0.0 alpha:0.55]];
    [globe_overlay_label_.layer setCornerRadius:6.0];
    [globe_overlay_label_ setHidden:YES];

    manual_button_ = make_button(@"Manual", self, @selector(toggleManualMode:));
    arm_button_ = make_button(@"Arm", self, @selector(toggleArmAction:));
    pause_button_ = make_button(@"Pause", self, @selector(togglePause:));
    chase_button_ = make_button(@"Chase", self, @selector(toggleChaseCamera:));
    fit_button_ = make_button(@"Fit", self, @selector(fitCameraAction:));
    globe_button_ = make_button(@"Globe", self, @selector(toggleGlobeMode:));
    three_d_button_ = make_button(@"3D", self, @selector(toggle3DMode:));
    replay_button_ = make_button(@"Replay", self, @selector(toggleReplayAction:));
    load_mission_button_ = make_button(@"Load Mission", self, @selector(loadMissionAction:));
    load_replay_button_ = make_button(@"Replay File", self, @selector(loadReplayFileAction:));
    location_button_ = make_button(@"Location", self, @selector(loadLocationAction:));
    save_button_ = make_button(@"Save", self, @selector(saveMissionAction:));
    reset_button_ = make_button(@"Reset", self, @selector(resetMissionAction:));
    replay_slider_ = make_slider(self, @selector(scrubReplay:));

    for (NSView* subview in @[title_label_, telemetry_label_, mission_label_, message_label_,
                              replay_time_label_, replay_slider_,
                              manual_button_, arm_button_, pause_button_, reset_button_,
                              chase_button_, fit_button_, save_button_, load_mission_button_,
                              replay_button_, load_replay_button_, globe_button_, three_d_button_,
                              location_button_]) {
        [side_panel_ addSubview:subview];
    }
    [self addSubview:globe_overlay_label_];
}

- (void)layout {
    [super layout];

    if (!side_panel_) {
        return;
    }

    const NSRect bounds = [self bounds];
    const CGFloat panel_width = 306.0;
    [side_panel_ setFrame:NSMakeRect(bounds.size.width - panel_width, 0.0, panel_width, bounds.size.height)];
    [globe_overlay_label_ setFrame:NSMakeRect(24.0, 24.0, std::max<CGFloat>(420.0, bounds.size.width - panel_width - 72.0), 68.0)];

    CGFloat y = bounds.size.height - 42.0;
    const CGFloat x = 18.0;
    const CGFloat width = panel_width - 36.0;
    [title_label_ setFrame:NSMakeRect(x, y, width, 24.0)];

    y -= 176.0;
    [telemetry_label_ setFrame:NSMakeRect(x, y, width, 166.0)];

    y -= 142.0;
    [mission_label_ setFrame:NSMakeRect(x, y, width, 128.0)];

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

    y -= button_height + gap;
    [globe_button_ setFrame:NSMakeRect(x, y, col_width, button_height)];
    [three_d_button_ setFrame:NSMakeRect(x + col_width + gap, y, col_width, button_height)];

    y -= button_height + gap;
    [location_button_ setFrame:NSMakeRect(x, y, width, button_height)];
}

- (void)prepareOpenGL {
    [super prepareOpenGL];
    GLint swapInterval = 1;
    [[self openGLContext] setValues:&swapInterval forParameter:NSOpenGLContextParameterSwapInterval];

    glDisable(GL_DEPTH_TEST);
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
    glClearColor(0.025f, 0.032f, 0.042f, 1.0f);
    [self loadMapTilesForMission];
    [self loadRealWorldTerrainForMission];
}

- (void)clearMapTiles {
    [[self openGLContext] makeCurrentContext];
    for (const MapTile& tile : map_tiles_) {
        if (tile.texture_id != 0) {
            GLuint texture_id = tile.texture_id;
            glDeleteTextures(1, &texture_id);
        }
    }
    map_tiles_.clear();
    map_status_ = "Map off";
}

- (void)clearGlobeMapTiles {
    [[self openGLContext] makeCurrentContext];
    for (const GlobeMapTile& tile : globe_map_tiles_) {
        if (tile.texture_id != 0) {
            GLuint texture_id = tile.texture_id;
            glDeleteTextures(1, &texture_id);
        }
    }
    globe_map_tiles_.clear();
    globe_map_zoom_ = 0;
    globe_map_key_.clear();
    globe_map_status_ = "World map off";
}

- (int)desiredGlobeMapZoom {
    const double zoom_level = std::round(std::log2(std::max(1.0, globe_view_zoom_)) + static_cast<double>(kGlobeMapMinZoom));
    return std::clamp(static_cast<int>(zoom_level), kGlobeMapMinZoom, kGlobeMapMaxZoom);
}

- (void)loadGlobeMapTiles {
    int selected_zoom = [self desiredGlobeMapZoom];
    std::vector<TileCoordinate> requested_tiles;
    while (selected_zoom >= kGlobeMapMinZoom) {
        requested_tiles = globe_tiles_for_view(
            selected_zoom,
            globe_center_latitude_,
            globe_center_longitude_,
            globe_view_zoom_
        );
        if (requested_tiles.size() <= kMaxGlobeTileLoads || selected_zoom <= 4) {
            break;
        }
        --selected_zoom;
    }

    const std::string requested_key = key_for_tiles(requested_tiles);
    if (!globe_map_tiles_.empty() && globe_map_zoom_ == selected_zoom && globe_map_key_ == requested_key) {
        return;
    }

    [[self openGLContext] makeCurrentContext];
    [self clearGlobeMapTiles];

    int loaded_count = 0;
    for (const TileCoordinate& tile : requested_tiles) {
        NSData* data = data_for_tile(tile.z, tile.x, tile.y);
        const GLuint texture_id = texture_from_image_data(data);
        if (texture_id == 0) {
            continue;
        }
        globe_map_tiles_.push_back({
            texture_id,
            tile.bounds(),
        });
        ++loaded_count;
    }

    if (loaded_count > 0) {
        globe_map_zoom_ = selected_zoom;
        globe_map_key_ = requested_key;
        globe_map_status_ = "Globe OSM z" + std::to_string(selected_zoom) + " " +
            std::to_string(loaded_count) + "/" + std::to_string(requested_tiles.size());
        [self setStatusMessage:"World map loaded"];
    } else {
        globe_map_key_.clear();
        globe_map_status_ = "World map unavailable";
        [self setStatusMessage:"World map unavailable"];
    }
}

- (void)loadMapTilesForMission {
    if (!simulation_ || !simulation_->mission().home_geo) {
        [self clearMapTiles];
        return;
    }

    [[self openGLContext] makeCurrentContext];
    [self clearMapTiles];

    const auto& mission = simulation_->mission();
    const GeoCoordinate origin = *mission.home_geo;
    const ViewBounds view_bounds = [self viewBoundsForRect:[self sceneRect]];
    const double padding_m = 40.0;
    const std::vector<GeoCoordinate> corners = {
        geo_from_local(Vec3(view_bounds.min_x - padding_m, 0.0, view_bounds.min_z - padding_m), origin),
        geo_from_local(Vec3(view_bounds.min_x - padding_m, 0.0, view_bounds.max_z + padding_m), origin),
        geo_from_local(Vec3(view_bounds.max_x + padding_m, 0.0, view_bounds.min_z - padding_m), origin),
        geo_from_local(Vec3(view_bounds.max_x + padding_m, 0.0, view_bounds.max_z + padding_m), origin),
    };

    int selected_zoom = 18;
    int min_tile_x = 0;
    int max_tile_x = 0;
    int min_tile_y = 0;
    int max_tile_y = 0;

    for (int zoom = 18; zoom >= 14; --zoom) {
        bool first = true;
        for (const GeoCoordinate& corner : corners) {
            const TileCoordinate tile = tile_for_geo(corner, zoom);
            if (first) {
                min_tile_x = max_tile_x = tile.x;
                min_tile_y = max_tile_y = tile.y;
                first = false;
            } else {
                min_tile_x = std::min(min_tile_x, tile.x);
                max_tile_x = std::max(max_tile_x, tile.x);
                min_tile_y = std::min(min_tile_y, tile.y);
                max_tile_y = std::max(max_tile_y, tile.y);
            }
        }

        min_tile_x = std::max(0, min_tile_x - 1);
        min_tile_y = std::max(0, min_tile_y - 1);
        const int max_index = (1 << zoom) - 1;
        max_tile_x = std::min(max_index, max_tile_x + 1);
        max_tile_y = std::min(max_index, max_tile_y + 1);

        const int tile_count = (max_tile_x - min_tile_x + 1) * (max_tile_y - min_tile_y + 1);
        selected_zoom = zoom;
        if (tile_count <= 16 || zoom == 14) {
            break;
        }
    }

    int loaded_count = 0;
    for (int tile_y = min_tile_y; tile_y <= max_tile_y; ++tile_y) {
        for (int tile_x = min_tile_x; tile_x <= max_tile_x; ++tile_x) {
            NSData* data = data_for_tile(selected_zoom, tile_x, tile_y);
            const GLuint texture_id = texture_from_image_data(data);
            if (texture_id == 0) {
                continue;
            }

            const GeoBounds tile_bounds = TileCoordinate{selected_zoom, tile_x, tile_y}.bounds();
            const GeoCoordinate north_west {tile_bounds.max_latitude, tile_bounds.min_longitude, 0.0};
            const GeoCoordinate south_east {tile_bounds.min_latitude, tile_bounds.max_longitude, 0.0};
            const Vec3 local_north_west = local_from_geo(north_west, origin);
            const Vec3 local_south_east = local_from_geo(south_east, origin);

            map_tiles_.push_back({
                texture_id,
                local_north_west.x,
                local_south_east.x,
                local_south_east.z,
                local_north_west.z,
            });
            ++loaded_count;
        }
    }

    if (loaded_count > 0) {
        map_status_ = "OSM z" + std::to_string(selected_zoom);
        [self setStatusMessage:"Map tiles loaded"];
    } else {
        map_status_ = "Grid only";
        [self setStatusMessage:"Map unavailable"];
    }
}

- (void)clearTerrain {
    terrain_mesh_ = {};
    terrain_status_ = "Terrain off";
}

- (void)loadRealWorldTerrainForMission {
    if (!simulation_ || !simulation_->mission().home_geo) {
        [self clearTerrain];
        return;
    }

    const GeoCoordinate origin = *simulation_->mission().home_geo;
    const double radius_m = radius_m_for_area_km2(real_world_area_km2_);
    const GeoBounds bounds = GeoBounds::from_center(origin, radius_m);
    int zoom = zoom_for_radius_m(radius_m);
    std::vector<TileCoordinate> requested_tiles = tiles_for_bounds(bounds, zoom);

    while (requested_tiles.size() > 16 && zoom > 10) {
        --zoom;
        requested_tiles = tiles_for_bounds(bounds, zoom);
    }

    std::vector<ElevationTile> elevation_tiles;
    int failed_tiles = 0;
    for (const TileCoordinate& tile : requested_tiles) {
        NSData* data = data_for_elevation_tile(tile.z, tile.x, tile.y);
        std::vector<std::uint8_t> rgba_pixels;
        int width = 0;
        int height = 0;
        if (!rgba_pixels_from_image_data(data, rgba_pixels, width, height)) {
            ++failed_tiles;
            continue;
        }

        auto elevation_tile = elevation_tile_from_terrarium_rgba(tile, width, height, rgba_pixels);
        if (!elevation_tile) {
            ++failed_tiles;
            continue;
        }
        elevation_tiles.push_back(std::move(*elevation_tile));
    }

    constexpr int kTerrainResolution = 96;
    const auto composite = composite_elevation_with_state(
        elevation_tiles,
        bounds,
        kTerrainResolution,
        requested_tiles);
    terrain_mesh_ = build_terrain_mesh(composite.heightmap, kTerrainResolution, bounds.width_m(), bounds.height_m(), 1.0);

    std::ostringstream status;
    status << "Terrain z" << zoom << " " << elevation_tiles.size() << "/" << requested_tiles.size();
    const auto fallback_count = static_cast<int>(std::count_if(
        composite.tile_states.begin(),
        composite.tile_states.end(),
        [](const auto& tile_status) {
            return tile_status.state == agbot::flight_sim::TerrainTileState::FlatFallback;
        }));
    if (failed_tiles > 0 || fallback_count > 0) {
        status << " (" << std::max(failed_tiles, fallback_count) << " flat_fallback)";
    }
    if (terrain_mesh_.has_elevation) {
        status << " " << std::fixed << std::setprecision(0)
               << terrain_mesh_.min_elevation_m << "-" << terrain_mesh_.max_elevation_m << "m";
        [self setStatusMessage:"Elevation loaded"];
    } else if (elevation_tiles.empty()) {
        status << " flat/no data";
        [self setStatusMessage:"Elevation unavailable"];
    } else {
        status << " flat";
        [self setStatusMessage:"Flat elevation"];
    }
    terrain_status_ = status.str();
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
              << "Camera    "
              << (globe_mode_ ? "globe" : (terrain_3d_mode_ ? (chase_camera_ ? "3d chase" : "3d") : (chase_camera_ ? "chase" : "map")))
              << "\n"
              << "Map       " << map_status_ << "\n"
              << "Terrain   " << terrain_status_ << "\n"
              << "Globe     " << globe_map_status_;
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
    if (simulation_->mission().home_geo) {
        const GeoCoordinate home = *simulation_->mission().home_geo;
        mission << "\nHome      " << std::fixed << std::setprecision(6)
                << home.latitude << ", " << home.longitude;
    }
    [mission_label_ setStringValue:ns_string(mission.str())];

    if (globe_overlay_label_) {
        std::ostringstream overlay_text;
        if (globe_mode_) {
            if (globe_hover_coordinate_) {
                overlay_text << std::fixed << std::setprecision(6)
                             << "Cursor lat " << globe_hover_coordinate_->latitude
                             << "  lon " << globe_hover_coordinate_->longitude << "\n";
            }
            if (simulation_->mission().home_geo) {
                const GeoCoordinate home = *simulation_->mission().home_geo;
                overlay_text << std::fixed << std::setprecision(6)
                             << "Pin  lat " << home.latitude << "  lon " << home.longitude
                             << "   Area " << std::setprecision(1) << real_world_area_km2_ << " km2\n";
            } else {
                overlay_text << "Pin  no geodetic mission loaded\n";
            }
            overlay_text << globe_map_status_ << "   View "
                         << std::fixed << std::setprecision(1) << globe_view_zoom_
                         << "x   Wheel/+/- zoom, drag rotate, click to load";
        } else if (terrain_3d_mode_) {
            overlay_text << "3D terrain  distance "
                         << std::fixed << std::setprecision(0) << terrain3d_distance_m_ << " m"
                         << "   " << terrain_status_ << "\n"
                         << "Drag orbit, wheel/+/- zoom, arrows pan, C chase, V return to 2D";
        } else {
            overlay_text << "";
        }
        [globe_overlay_label_ setStringValue:ns_string(overlay_text.str())];
        [globe_overlay_label_ setHidden:(!globe_mode_ && !terrain_3d_mode_)];
    }

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
    [globe_button_ setTitle:(globe_mode_ ? @"Map" : @"Globe")];
    [three_d_button_ setTitle:(terrain_3d_mode_ ? @"2D" : @"3D")];
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
    globe_mode_ = false;
    chase_camera_ = !chase_camera_;
    [self setStatusMessage:(chase_camera_ ? "Chase camera" : (terrain_3d_mode_ ? "3D terrain camera" : "Mission map camera"))];
    [[self window] makeFirstResponder:self];
}

- (void)toggleGlobeMode:(id)sender {
    (void)sender;
    globe_mode_ = !globe_mode_;
    terrain_3d_mode_ = false;
    chase_camera_ = false;
    dragging_waypoint_ = false;
    dragging_globe_ = false;
    dragging_3d_camera_ = false;
    globe_hover_coordinate_.reset();
    if (globe_mode_) {
        if (simulation_ && simulation_->mission().home_geo) {
            const GeoCoordinate home = *simulation_->mission().home_geo;
            globe_center_latitude_ = home.latitude;
            globe_center_longitude_ = home.longitude;
        }
        [self loadGlobeMapTiles];
    }
    [self setStatusMessage:(globe_mode_ ? "Globe picker" : "Mission map camera")];
    [[self window] makeFirstResponder:self];
}

- (double)defaultTerrain3DDistance {
    if (terrain_mesh_.vertices.empty()) {
        return std::clamp(zoom_m_ * 4.5, 900.0, 18000.0);
    }

    double min_x = terrain_mesh_.vertices.front().position.x;
    double max_x = min_x;
    double min_z = terrain_mesh_.vertices.front().position.z;
    double max_z = min_z;
    double min_y = terrain_mesh_.vertices.front().position.y;
    double max_y = min_y;
    for (const auto& vertex : terrain_mesh_.vertices) {
        min_x = std::min(min_x, vertex.position.x);
        max_x = std::max(max_x, vertex.position.x);
        min_z = std::min(min_z, vertex.position.z);
        max_z = std::max(max_z, vertex.position.z);
        min_y = std::min(min_y, vertex.position.y);
        max_y = std::max(max_y, vertex.position.y);
    }

    const double footprint = std::max(max_x - min_x, max_z - min_z);
    const double elevation = std::max(0.0, max_y - min_y);
    return std::clamp((footprint + elevation) * 1.25, 900.0, 22000.0);
}

- (void)adjustTerrain3DZoomBy:(double)factor {
    terrain3d_distance_m_ = std::clamp(terrain3d_distance_m_ * factor, 120.0, 30000.0);
    std::ostringstream message;
    message << "3D zoom " << std::fixed << std::setprecision(0) << terrain3d_distance_m_ << " m";
    [self setStatusMessage:message.str()];
    [self setNeedsDisplay:YES];
}

- (void)toggle3DMode:(id)sender {
    (void)sender;
    terrain_3d_mode_ = !terrain_3d_mode_;
    globe_mode_ = false;
    dragging_waypoint_ = false;
    dragging_globe_ = false;
    dragging_3d_camera_ = false;
    if (terrain_3d_mode_) {
        [self loadRealWorldTerrainForMission];
        terrain3d_distance_m_ = [self defaultTerrain3DDistance];
    } else {
        chase_camera_ = false;
    }
    [self setStatusMessage:(terrain_3d_mode_ ? "3D terrain view" : "Mission map camera")];
    [[self window] makeFirstResponder:self];
}

- (void)adjustGlobeZoomBy:(double)factor {
    globe_view_zoom_ = std::clamp(globe_view_zoom_ * factor, kMinGlobeViewZoom, kMaxGlobeViewZoom);
    if (globe_mode_) {
        [self loadGlobeMapTiles];
    }

    std::ostringstream message;
    message << "Globe zoom " << std::fixed << std::setprecision(globe_view_zoom_ < 10.0 ? 1 : 0)
            << globe_view_zoom_ << "x";
    [self setStatusMessage:message.str()];
    [self setNeedsDisplay:YES];
}

- (void)fitCameraAction:(id)sender {
    (void)sender;
    globe_mode_ = false;
    [self fitMissionCamera];
    if (terrain_3d_mode_) {
        terrain3d_distance_m_ = [self defaultTerrain3DDistance];
    }
    [self loadMapTilesForMission];
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
        [self loadMapTilesForMission];
        [self loadRealWorldTerrainForMission];
        if (terrain_3d_mode_) {
            terrain3d_distance_m_ = [self defaultTerrain3DDistance];
        }
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

- (void)loadLocationCoordinate:(GeoCoordinate)coordinate areaKm2:(double)area_km2 source:(const std::string&)source {
    if (!simulation_) {
        return;
    }

    try {
        [self finishRecording];
        real_world_area_km2_ = std::clamp(area_km2, 1.0, 400.0);
        Mission mission = mission_for_location(coordinate, real_world_area_km2_);
        simulation_->replace_mission(std::move(mission));
        mission_path_.clear();
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
        [self loadMapTilesForMission];
        [self loadRealWorldTerrainForMission];
        if (terrain_3d_mode_) {
            terrain3d_distance_m_ = [self defaultTerrain3DDistance];
        }
        [self startNewRecording];
        [self setStatusMessage:source + " loaded"];
    } catch (const std::exception& error) {
        std::cerr << "Failed to load location: " << error.what() << "\n";
        [self setStatusMessage:"Location load failed"];
    }
}

- (void)loadLocationAction:(id)sender {
    (void)sender;
    if (!simulation_) {
        return;
    }

    const GeoCoordinate current = simulation_->mission().home_geo.value_or(GeoCoordinate{36.7783, -119.4179, 0.0});

    NSAlert* alert = [[[NSAlert alloc] init] autorelease];
    [alert setMessageText:@"Load Real World Location"];
    [alert setInformativeText:@"Enter latitude, longitude, and total area in square kilometers."];
    [alert addButtonWithTitle:@"Load"];
    [alert addButtonWithTitle:@"Cancel"];

    NSStackView* stack = [[[NSStackView alloc] initWithFrame:NSMakeRect(0.0, 0.0, 280.0, 92.0)] autorelease];
    [stack setOrientation:NSUserInterfaceLayoutOrientationVertical];
    [stack setSpacing:8.0];

    NSTextField* latitude_field = [NSTextField textFieldWithString:[NSString stringWithFormat:@"%.6f", current.latitude]];
    NSTextField* longitude_field = [NSTextField textFieldWithString:[NSString stringWithFormat:@"%.6f", current.longitude]];
    NSTextField* area_field = [NSTextField textFieldWithString:[NSString stringWithFormat:@"%.1f", real_world_area_km2_]];
    [latitude_field setPlaceholderString:@"Latitude"];
    [longitude_field setPlaceholderString:@"Longitude"];
    [area_field setPlaceholderString:@"Area km²"];

    [stack addArrangedSubview:latitude_field];
    [stack addArrangedSubview:longitude_field];
    [stack addArrangedSubview:area_field];
    [alert setAccessoryView:stack];

    if ([alert runModal] != NSAlertFirstButtonReturn) {
        [[self window] makeFirstResponder:self];
        return;
    }

    const double latitude = std::clamp([[latitude_field stringValue] doubleValue], -85.0, 85.0);
    const double longitude = std::clamp([[longitude_field stringValue] doubleValue], -180.0, 180.0);
    const double area_km2 = std::clamp([[area_field stringValue] doubleValue], 1.0, 400.0);

    [self loadLocationCoordinate:{latitude, longitude, 0.0} areaKm2:area_km2 source:"Location"];

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

- (double)terrainHeightAtX:(double)x z:(double)z {
    if (terrain_mesh_.vertices.empty()) {
        return 0.0;
    }

    const int resolution = static_cast<int>(std::lround(std::sqrt(static_cast<double>(terrain_mesh_.vertices.size()))));
    if (resolution < 2 || resolution * resolution != static_cast<int>(terrain_mesh_.vertices.size())) {
        return 0.0;
    }

    const double min_x = terrain_mesh_.vertices.front().position.x;
    const double max_x = terrain_mesh_.vertices[static_cast<std::size_t>(resolution - 1)].position.x;
    const double min_z = terrain_mesh_.vertices.front().position.z;
    const double max_z = terrain_mesh_.vertices[static_cast<std::size_t>((resolution - 1) * resolution)].position.z;
    if (std::abs(max_x - min_x) <= 1e-9 || std::abs(max_z - min_z) <= 1e-9) {
        return 0.0;
    }

    const double grid_x = std::clamp((x - min_x) / (max_x - min_x), 0.0, 1.0) * static_cast<double>(resolution - 1);
    const double grid_z = std::clamp((z - min_z) / (max_z - min_z), 0.0, 1.0) * static_cast<double>(resolution - 1);
    const int x0 = static_cast<int>(std::floor(grid_x));
    const int z0 = static_cast<int>(std::floor(grid_z));
    const int x1 = std::min(resolution - 1, x0 + 1);
    const int z1 = std::min(resolution - 1, z0 + 1);
    const double tx = grid_x - static_cast<double>(x0);
    const double tz = grid_z - static_cast<double>(z0);

    auto height_at = [&](int sample_x, int sample_z) {
        return terrain_mesh_.vertices[static_cast<std::size_t>(sample_z * resolution + sample_x)].position.y;
    };

    const double h00 = height_at(x0, z0);
    const double h10 = height_at(x1, z0);
    const double h01 = height_at(x0, z1);
    const double h11 = height_at(x1, z1);
    const double h0 = h00 + (h10 - h00) * tx;
    const double h1 = h01 + (h11 - h01) * tx;
    return h0 + (h1 - h0) * tz;
}

- (Vec3)renderPositionForFlightPosition:(Vec3)position {
    return Vec3(
        position.x,
        position.y + [self terrainHeightAtX:position.x z:position.z],
        position.z
    );
}

- (void)addWaypointAt:(Vec3)point {
    if (!simulation_) {
        return;
    }

    Waypoint waypoint;
    waypoint.name = "edited_waypoint_" + std::to_string(simulation_->mission().waypoints.size() + 1);
    waypoint.position = point;
    if (simulation_->mission().home_geo) {
        waypoint.geo = geo_from_local(point, *simulation_->mission().home_geo);
    }
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

- (void)drawMapTiles {
    if (map_tiles_.empty()) {
        return;
    }

    glEnable(GL_TEXTURE_2D);
    set_color(1.0, 1.0, 1.0, 0.88);
    for (const MapTile& tile : map_tiles_) {
        glBindTexture(GL_TEXTURE_2D, tile.texture_id);
        glBegin(GL_QUADS);
        glTexCoord2d(0.0, 1.0);
        glVertex2d(tile.min_x, tile.min_z);
        glTexCoord2d(1.0, 1.0);
        glVertex2d(tile.max_x, tile.min_z);
        glTexCoord2d(1.0, 0.0);
        glVertex2d(tile.max_x, tile.max_z);
        glTexCoord2d(0.0, 0.0);
        glVertex2d(tile.min_x, tile.max_z);
        glEnd();
    }
    glBindTexture(GL_TEXTURE_2D, 0);
    glDisable(GL_TEXTURE_2D);
}

- (void)drawTerrain {
    if (terrain_mesh_.vertices.empty() || terrain_mesh_.indices.empty()) {
        return;
    }

    const double elevation_span = std::max(
        1.0,
        static_cast<double>(terrain_mesh_.max_elevation_m - terrain_mesh_.min_elevation_m)
    );

    glLineWidth(1.0f);
    glBegin(GL_TRIANGLES);
    for (const std::uint32_t index : terrain_mesh_.indices) {
        if (index >= terrain_mesh_.vertices.size()) {
            continue;
        }
        const auto& vertex = terrain_mesh_.vertices[index];
        const double normalized_height = std::clamp(vertex.position.y / elevation_span, 0.0, 1.0);
        const double r = 0.16 + normalized_height * 0.54;
        const double g = 0.30 + normalized_height * 0.38;
        const double b = 0.22 + normalized_height * 0.18;
        set_color(r, g, b, terrain_mesh_.has_elevation ? 0.52 : 0.18);
        glVertex2d(vertex.position.x, vertex.position.z);
    }
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

- (Vec3)terrain3DTarget {
    if (chase_camera_ && simulation_) {
        return [self renderPositionForFlightPosition:[self displayState].position];
    }
    return Vec3(pan_x_, [self terrainHeightAtX:pan_x_ z:pan_z_] + 30.0, pan_z_);
}

- (void)apply3DCameraForScene:(NSRect)scene {
    const double aspect = std::max(0.1, scene.size.width / std::max(1.0, scene.size.height));
    constexpr double near_plane = 2.0;
    const double far_plane = std::max(12000.0, terrain3d_distance_m_ * 5.0);
    const double fov_rad = 55.0 * M_PI / 180.0;
    const double top = std::tan(fov_rad * 0.5) * near_plane;
    const double right = top * aspect;

    glMatrixMode(GL_PROJECTION);
    glLoadIdentity();
    glFrustum(-right, right, -top, top, near_plane, far_plane);

    glMatrixMode(GL_MODELVIEW);
    glLoadIdentity();
    const Vec3 target = [self terrain3DTarget];
    const double pitch = std::clamp(terrain3d_pitch_rad_, 0.16, 1.38);
    const double cp = std::cos(pitch);
    const Vec3 eye = target + Vec3(
        std::sin(terrain3d_yaw_rad_) * cp * terrain3d_distance_m_,
        std::sin(pitch) * terrain3d_distance_m_,
        std::cos(terrain3d_yaw_rad_) * cp * terrain3d_distance_m_
    );
    apply_look_at(eye, target, Vec3(0.0, 1.0, 0.0));
}

- (void)drawMapTiles3D {
    if (map_tiles_.empty()) {
        return;
    }

    glEnable(GL_TEXTURE_2D);
    set_color(1.0, 1.0, 1.0, 0.78);
    for (const MapTile& tile : map_tiles_) {
        glBindTexture(GL_TEXTURE_2D, tile.texture_id);
        glBegin(GL_QUADS);
        glTexCoord2d(0.0, 1.0);
        glVertex3d(tile.min_x, -0.3, tile.min_z);
        glTexCoord2d(1.0, 1.0);
        glVertex3d(tile.max_x, -0.3, tile.min_z);
        glTexCoord2d(1.0, 0.0);
        glVertex3d(tile.max_x, -0.3, tile.max_z);
        glTexCoord2d(0.0, 0.0);
        glVertex3d(tile.min_x, -0.3, tile.max_z);
        glEnd();
    }
    glBindTexture(GL_TEXTURE_2D, 0);
    glDisable(GL_TEXTURE_2D);
}

- (void)drawGroundGrid3D {
    double min_x = -500.0;
    double max_x = 500.0;
    double min_z = -500.0;
    double max_z = 500.0;
    if (!terrain_mesh_.vertices.empty()) {
        min_x = max_x = terrain_mesh_.vertices.front().position.x;
        min_z = max_z = terrain_mesh_.vertices.front().position.z;
        for (const auto& vertex : terrain_mesh_.vertices) {
            min_x = std::min(min_x, vertex.position.x);
            max_x = std::max(max_x, vertex.position.x);
            min_z = std::min(min_z, vertex.position.z);
            max_z = std::max(max_z, vertex.position.z);
        }
    }

    const double step = 250.0;
    min_x = std::floor(min_x / step) * step;
    max_x = std::ceil(max_x / step) * step;
    min_z = std::floor(min_z / step) * step;
    max_z = std::ceil(max_z / step) * step;

    set_color(0.12, 0.16, 0.20, 0.75);
    glLineWidth(1.0f);
    glBegin(GL_LINES);
    for (double x = min_x; x <= max_x; x += step) {
        glVertex3d(x, 0.12, min_z);
        glVertex3d(x, 0.12, max_z);
    }
    for (double z = min_z; z <= max_z; z += step) {
        glVertex3d(min_x, 0.12, z);
        glVertex3d(max_x, 0.12, z);
    }
    glEnd();
}

- (void)drawTerrain3D {
    if (terrain_mesh_.vertices.empty() || terrain_mesh_.indices.empty()) {
        return;
    }

    const double elevation_span = std::max(
        1.0,
        static_cast<double>(terrain_mesh_.max_elevation_m - terrain_mesh_.min_elevation_m)
    );

    glBegin(GL_TRIANGLES);
    for (const std::uint32_t index : terrain_mesh_.indices) {
        if (index >= terrain_mesh_.vertices.size()) {
            continue;
        }
        const auto& vertex = terrain_mesh_.vertices[index];
        const double normalized_height = std::clamp(vertex.position.y / elevation_span, 0.0, 1.0);
        const double light = std::clamp(
            vertex.normal.x * -0.25 + vertex.normal.y * 0.78 + vertex.normal.z * 0.30,
            0.38,
            1.0
        );
        const double r = (0.13 + normalized_height * 0.50) * light;
        const double g = (0.34 + normalized_height * 0.34) * light;
        const double b = (0.20 + normalized_height * 0.18) * light;
        set_color(r, g, b, terrain_mesh_.has_elevation ? 0.70 : 0.28);
        glNormal3d(vertex.normal.x, vertex.normal.y, vertex.normal.z);
        glVertex3d(vertex.position.x, vertex.position.y, vertex.position.z);
    }
    glEnd();
}

- (void)drawMissionPath3D {
    if (!simulation_) {
        return;
    }

    const auto& mission = simulation_->mission();
    set_color(0.86, 0.90, 1.0, 0.92);
    glLineWidth(3.0f);
    glBegin(GL_LINE_STRIP);
    const Vec3 home = [self renderPositionForFlightPosition:mission.home];
    glVertex3d(home.x, home.y + 0.5, home.z);
    for (const Waypoint& waypoint : mission.waypoints) {
        const Vec3 point = [self renderPositionForFlightPosition:waypoint.position];
        glVertex3d(point.x, point.y, point.z);
    }
    glEnd();

    glPointSize(8.0f);
    glBegin(GL_POINTS);
    set_color(0.25, 1.0, 0.72, 1.0);
    glVertex3d(home.x, home.y + 0.5, home.z);
    for (const Waypoint& waypoint : mission.waypoints) {
        color_for_action(waypoint.action);
        const Vec3 point = [self renderPositionForFlightPosition:waypoint.position];
        glVertex3d(point.x, point.y, point.z);
    }
    glEnd();
    glPointSize(1.0f);
}

- (void)drawTrail3D {
    if (trail_.size() < 2) {
        return;
    }

    set_color(0.2, 0.88, 1.0, 0.95);
    glLineWidth(2.5f);
    glBegin(GL_LINE_STRIP);
    for (const Vec3& point : trail_) {
        const Vec3 rendered = [self renderPositionForFlightPosition:point];
        glVertex3d(rendered.x, rendered.y, rendered.z);
    }
    glEnd();
}

- (void)drawDrone3D {
    if (!simulation_) {
        return;
    }

    const DroneState& state = [self displayState];
    const Vec3 position = [self renderPositionForFlightPosition:state.position];
    const double yaw = state.yaw_rad;
    const Vec3 forward(std::sin(yaw), 0.0, std::cos(yaw));
    const Vec3 right(std::cos(yaw), 0.0, -std::sin(yaw));

    const Vec3 nose = position + forward * 12.0 + Vec3(0.0, 1.5, 0.0);
    const Vec3 left = position - forward * 7.0 - right * 6.0;
    const Vec3 right_point = position - forward * 7.0 + right * 6.0;
    const double ground_y = [self terrainHeightAtX:state.position.x z:state.position.z] + 0.3;

    set_color(0.25, 0.95, 1.0, 0.55);
    glLineWidth(1.5f);
    glBegin(GL_LINES);
    glVertex3d(position.x, ground_y, position.z);
    glVertex3d(position.x, position.y, position.z);
    glEnd();

    set_color(1.0, 0.88, 0.18, 1.0);
    glBegin(GL_TRIANGLES);
    glVertex3d(nose.x, nose.y, nose.z);
    glVertex3d(left.x, left.y, left.z);
    glVertex3d(right_point.x, right_point.y, right_point.z);
    glEnd();

    set_color(0.02, 0.025, 0.03, 1.0);
    glLineWidth(2.0f);
    glBegin(GL_LINE_LOOP);
    glVertex3d(nose.x, nose.y, nose.z);
    glVertex3d(left.x, left.y, left.z);
    glVertex3d(right_point.x, right_point.y, right_point.z);
    glEnd();
}

- (NSRect)globeFrameForScene:(NSRect)scene {
    const CGFloat size = std::min(scene.size.width, scene.size.height) * 0.78 * globe_view_zoom_;
    return NSMakeRect(
        scene.origin.x + (scene.size.width - size) * 0.5,
        scene.origin.y + (scene.size.height - size) * 0.52,
        size,
        size
    );
}

- (NSPoint)screenPointForGlobePoint:(GlobePoint)point frame:(NSRect)frame {
    return NSMakePoint(
        frame.origin.x + frame.size.width * 0.5 + point.x * frame.size.width * 0.5,
        frame.origin.y + frame.size.height * 0.5 + point.y * frame.size.height * 0.5
    );
}

- (void)drawGlobeLineLatitude:(double)latitude frame:(NSRect)frame {
    bool drawing = false;
    glBegin(GL_LINE_STRIP);
    for (double lon = -180.0; lon <= 180.0; lon += 3.0) {
        const GlobePoint point = project_globe_point(latitude, lon, globe_center_latitude_, globe_center_longitude_);
        if (!point.visible) {
            if (drawing) {
                glEnd();
                glBegin(GL_LINE_STRIP);
                drawing = false;
            }
            continue;
        }
        const NSPoint screen = [self screenPointForGlobePoint:point frame:frame];
        glVertex2d(screen.x, screen.y);
        drawing = true;
    }
    glEnd();
}

- (void)drawGlobeLineLongitude:(double)longitude frame:(NSRect)frame {
    bool drawing = false;
    glBegin(GL_LINE_STRIP);
    for (double lat = -85.0; lat <= 85.0; lat += 3.0) {
        const GlobePoint point = project_globe_point(lat, longitude, globe_center_latitude_, globe_center_longitude_);
        if (!point.visible) {
            if (drawing) {
                glEnd();
                glBegin(GL_LINE_STRIP);
                drawing = false;
            }
            continue;
        }
        const NSPoint screen = [self screenPointForGlobePoint:point frame:frame];
        glVertex2d(screen.x, screen.y);
        drawing = true;
    }
    glEnd();
}

- (void)drawGlobeMapTiles:(NSRect)frame {
    if (globe_map_tiles_.empty()) {
        return;
    }

    glEnable(GL_TEXTURE_2D);
    set_color(1.0, 1.0, 1.0, 0.96);
    for (const GlobeMapTile& tile : globe_map_tiles_) {
        glBindTexture(GL_TEXTURE_2D, tile.texture_id);

        constexpr int kSubdivisions = 10;
        for (int row = 0; row < kSubdivisions; ++row) {
            const double v0 = static_cast<double>(row) / static_cast<double>(kSubdivisions);
            const double v1 = static_cast<double>(row + 1) / static_cast<double>(kSubdivisions);
            const double lat0 = tile.bounds.max_latitude - v0 * (tile.bounds.max_latitude - tile.bounds.min_latitude);
            const double lat1 = tile.bounds.max_latitude - v1 * (tile.bounds.max_latitude - tile.bounds.min_latitude);

            for (int column = 0; column < kSubdivisions; ++column) {
                const double u0 = static_cast<double>(column) / static_cast<double>(kSubdivisions);
                const double u1 = static_cast<double>(column + 1) / static_cast<double>(kSubdivisions);
                const double lon0 = tile.bounds.min_longitude + u0 * (tile.bounds.max_longitude - tile.bounds.min_longitude);
                const double lon1 = tile.bounds.min_longitude + u1 * (tile.bounds.max_longitude - tile.bounds.min_longitude);

                const GlobePoint north_west = project_globe_point(lat0, lon0, globe_center_latitude_, globe_center_longitude_);
                const GlobePoint north_east = project_globe_point(lat0, lon1, globe_center_latitude_, globe_center_longitude_);
                const GlobePoint south_east = project_globe_point(lat1, lon1, globe_center_latitude_, globe_center_longitude_);
                const GlobePoint south_west = project_globe_point(lat1, lon0, globe_center_latitude_, globe_center_longitude_);
                if (!north_west.visible || !north_east.visible || !south_east.visible || !south_west.visible) {
                    continue;
                }

                const NSPoint nw = [self screenPointForGlobePoint:north_west frame:frame];
                const NSPoint ne = [self screenPointForGlobePoint:north_east frame:frame];
                const NSPoint se = [self screenPointForGlobePoint:south_east frame:frame];
                const NSPoint sw = [self screenPointForGlobePoint:south_west frame:frame];

                glBegin(GL_QUADS);
                glTexCoord2d(u0, v0);
                glVertex2d(nw.x, nw.y);
                glTexCoord2d(u1, v0);
                glVertex2d(ne.x, ne.y);
                glTexCoord2d(u1, v1);
                glVertex2d(se.x, se.y);
                glTexCoord2d(u0, v1);
                glVertex2d(sw.x, sw.y);
                glEnd();
            }
        }
    }
    glBindTexture(GL_TEXTURE_2D, 0);
    glDisable(GL_TEXTURE_2D);
}

- (void)drawGlobe:(NSRect)scene {
    const NSRect frame = [self globeFrameForScene:scene];
    const double radius = frame.size.width * 0.5;
    const double center_x = frame.origin.x + radius;
    const double center_y = frame.origin.y + radius;

    set_color(0.03, 0.07, 0.11, 1.0);
    draw_filled_circle(center_x, center_y, radius, 96);

    set_color(0.08, 0.22, 0.32, 1.0);
    draw_filled_circle(center_x, center_y, radius * 0.98, 96);

    [self drawGlobeMapTiles:frame];

    set_color(0.72, 0.82, 0.9, 0.34);
    glLineWidth(1.0f);
    for (double lat = -60.0; lat <= 60.0; lat += 30.0) {
        [self drawGlobeLineLatitude:lat frame:frame];
    }
    for (double lon = -180.0; lon < 180.0; lon += 30.0) {
        [self drawGlobeLineLongitude:lon frame:frame];
    }

    set_color(0.84, 0.92, 1.0, 0.9);
    glLineWidth(2.0f);
    draw_circle(center_x, center_y, radius, 128);

    if (simulation_ && simulation_->mission().home_geo) {
        const GeoCoordinate home = *simulation_->mission().home_geo;
        const GlobePoint marker = project_globe_point(home.latitude, home.longitude, globe_center_latitude_, globe_center_longitude_);
        if (marker.visible) {
            const NSPoint screen = [self screenPointForGlobePoint:marker frame:frame];
            set_color(1.0, 0.85, 0.2, 1.0);
            draw_filled_circle(screen.x, screen.y, 6.0, 24);
            set_color(0.02, 0.025, 0.03, 1.0);
            draw_circle(screen.x, screen.y, 8.0, 24);
        }
    }

    set_color(0.0, 0.0, 0.0, 0.24);
    draw_rect(18.0, 18.0, 365.0, 42.0);
}

- (std::optional<GeoCoordinate>)globeCoordinateFromPoint:(NSPoint)location {
    const NSRect scene = [self sceneRect];
    const NSRect frame = [self globeFrameForScene:scene];
    const double radius = frame.size.width * 0.5;
    const double x = (location.x - (frame.origin.x + radius)) / radius;
    const double y = (location.y - (frame.origin.y + radius)) / radius;
    const double r2 = x * x + y * y;
    if (r2 > 1.0) {
        return std::nullopt;
    }

    const double z = std::sqrt(std::max(0.0, 1.0 - r2));
    const double center_lat = globe_center_latitude_ * M_PI / 180.0;

    const double sin_lat = y * std::cos(center_lat) + z * std::sin(center_lat);
    const double latitude = std::asin(std::clamp(sin_lat, -1.0, 1.0)) * 180.0 / M_PI;
    const double lon_delta = std::atan2(x, z * std::cos(center_lat) - y * std::sin(center_lat)) * 180.0 / M_PI;
    double longitude = globe_center_longitude_ + lon_delta;
    while (longitude > 180.0) {
        longitude -= 360.0;
    }
    while (longitude < -180.0) {
        longitude += 360.0;
    }
    return GeoCoordinate{std::clamp(latitude, -85.0, 85.0), longitude, 0.0};
}

- (std::optional<GeoCoordinate>)globeCoordinateFromEvent:(NSEvent*)event {
    return [self globeCoordinateFromPoint:[self convertPoint:[event locationInWindow] fromView:nil]];
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
    const NSRect backing_scene = [self convertRectToBacking:scene];
    glViewport(
        static_cast<GLint>(backing_scene.origin.x),
        static_cast<GLint>(backing_scene.origin.y),
        static_cast<GLsizei>(backing_scene.size.width),
        static_cast<GLsizei>(backing_scene.size.height)
    );
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

    if (globe_mode_) {
        glDisable(GL_DEPTH_TEST);
        glMatrixMode(GL_PROJECTION);
        glLoadIdentity();
        glOrtho(0.0, scene.size.width, 0.0, scene.size.height, -1.0, 1.0);
        glMatrixMode(GL_MODELVIEW);
        glLoadIdentity();
        [self drawGlobe:scene];
    } else if (terrain_3d_mode_) {
        glEnable(GL_DEPTH_TEST);
        glDepthFunc(GL_LEQUAL);
        glDepthMask(GL_TRUE);
        glDisable(GL_TEXTURE_2D);
        glDisable(GL_CULL_FACE);
        glDisable(GL_LIGHTING);
        glPolygonMode(GL_FRONT_AND_BACK, GL_FILL);
        [self apply3DCameraForScene:scene];
        glDepthMask(GL_FALSE);
        [self drawMapTiles3D];
        glDepthMask(GL_TRUE);
        [self drawTerrain3D];
        [self drawGroundGrid3D];
        [self drawMissionPath3D];
        [self drawTrail3D];
        [self drawDrone3D];
        glDisable(GL_DEPTH_TEST);
        [self drawHud:scene];
    } else {
        glDisable(GL_DEPTH_TEST);
        const ViewBounds viewBounds = [self viewBoundsForRect:scene];

        glMatrixMode(GL_PROJECTION);
        glLoadIdentity();
        glOrtho(viewBounds.min_x, viewBounds.max_x, viewBounds.min_z, viewBounds.max_z, -1.0, 1.0);
        glMatrixMode(GL_MODELVIEW);
        glLoadIdentity();

        [self drawTerrain];
        [self drawMapTiles];
        [self drawGrid:viewBounds];
        [self drawMissionPath];
        [self drawTrail];
        [self drawDrone];
        [self drawHud:scene];
    }

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
        if (key == 'b') {
            [self toggleGlobeMode:nil];
            return;
        }
        if (key == 'v') {
            [self toggle3DMode:nil];
            return;
        }
        if (key == 'f') {
            [self fitCameraAction:nil];
            return;
        }
        if (key == '-' || key == '_') {
            if (globe_mode_) {
                [self adjustGlobeZoomBy:0.74];
                return;
            }
            if (terrain_3d_mode_) {
                [self adjustTerrain3DZoomBy:1.15];
                return;
            }
            zoom_m_ = std::min(1000.0, zoom_m_ * 1.15);
            return;
        }
        if (key == '=' || key == '+') {
            if (globe_mode_) {
                [self adjustGlobeZoomBy:1.35];
                return;
            }
            if (terrain_3d_mode_) {
                [self adjustTerrain3DZoomBy:0.87];
                return;
            }
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

- (void)mouseMoved:(NSEvent*)event {
    if (!globe_mode_) {
        globe_hover_coordinate_.reset();
        return;
    }

    const NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    if (location.x > [self sceneRect].size.width) {
        globe_hover_coordinate_.reset();
    } else {
        globe_hover_coordinate_ = [self globeCoordinateFromPoint:location];
    }
    [self updatePanelText];
}

- (void)mouseDown:(NSEvent*)event {
    if (!simulation_) {
        return;
    }

    const NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    if (location.x > [self sceneRect].size.width) {
        return;
    }

    if (terrain_3d_mode_) {
        terrain3d_drag_start_ = location;
        terrain3d_drag_start_yaw_ = terrain3d_yaw_rad_;
        terrain3d_drag_start_pitch_ = terrain3d_pitch_rad_;
        dragging_3d_camera_ = true;
        return;
    }

    if (replay_mode_) {
        return;
    }

    if (globe_mode_) {
        globe_drag_start_ = location;
        globe_drag_start_latitude_ = globe_center_latitude_;
        globe_drag_start_longitude_ = globe_center_longitude_;
        globe_hover_coordinate_ = [self globeCoordinateFromPoint:location];
        dragging_globe_ = true;
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
    if (terrain_3d_mode_ && dragging_3d_camera_) {
        const NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
        terrain3d_yaw_rad_ = terrain3d_drag_start_yaw_ - (location.x - terrain3d_drag_start_.x) * 0.006;
        terrain3d_pitch_rad_ = std::clamp(
            terrain3d_drag_start_pitch_ + (location.y - terrain3d_drag_start_.y) * 0.004,
            0.16,
            1.38
        );
        [self setNeedsDisplay:YES];
        return;
    }

    if (globe_mode_ && dragging_globe_) {
        const NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
        const NSRect scene = [self sceneRect];
        const double degrees_per_pixel = 180.0 / std::max(1.0, static_cast<double>(std::min(scene.size.width, scene.size.height)));
        globe_center_longitude_ = globe_drag_start_longitude_ - (location.x - globe_drag_start_.x) * degrees_per_pixel;
        globe_center_latitude_ = std::clamp(
            globe_drag_start_latitude_ - (location.y - globe_drag_start_.y) * degrees_per_pixel,
            -80.0,
            80.0
        );
        while (globe_center_longitude_ > 180.0) {
            globe_center_longitude_ -= 360.0;
        }
        while (globe_center_longitude_ < -180.0) {
            globe_center_longitude_ += 360.0;
        }
        globe_hover_coordinate_ = [self globeCoordinateFromPoint:location];
        [self setNeedsDisplay:YES];
        return;
    }

    if (!simulation_ || selected_waypoint_ < 0 || !dragging_waypoint_) {
        return;
    }

    auto& waypoints = simulation_->mutable_mission().waypoints;
    if (static_cast<std::size_t>(selected_waypoint_) >= waypoints.size()) {
        return;
    }

    const double altitude = waypoints[static_cast<std::size_t>(selected_waypoint_)].position.y;
    Waypoint& waypoint = waypoints[static_cast<std::size_t>(selected_waypoint_)];
    waypoint.position = [self worldPointFromEvent:event altitude:altitude];
    if (simulation_->mission().home_geo) {
        waypoint.geo = geo_from_local(waypoint.position, *simulation_->mission().home_geo);
    }
}

- (void)mouseUp:(NSEvent*)event {
    if (terrain_3d_mode_ && dragging_3d_camera_) {
        dragging_3d_camera_ = false;
        [[self window] makeFirstResponder:self];
        return;
    }

    if (globe_mode_ && dragging_globe_) {
        const NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
        const double dx = location.x - globe_drag_start_.x;
        const double dy = location.y - globe_drag_start_.y;
        dragging_globe_ = false;

        if (std::sqrt(dx * dx + dy * dy) <= 4.0) {
            if (auto coordinate = [self globeCoordinateFromEvent:event]) {
                globe_center_latitude_ = coordinate->latitude;
                globe_center_longitude_ = coordinate->longitude;
                globe_mode_ = false;
                globe_hover_coordinate_.reset();
                [self loadLocationCoordinate:*coordinate areaKm2:real_world_area_km2_ source:"Globe location"];
            }
        } else {
            [self loadGlobeMapTiles];
        }
        [[self window] makeFirstResponder:self];
        return;
    }
    dragging_waypoint_ = false;
}

- (void)scrollWheel:(NSEvent*)event {
    if (globe_mode_) {
        if ([event scrollingDeltaY] > 0.0) {
            [self adjustGlobeZoomBy:1.35];
        } else if ([event scrollingDeltaY] < 0.0) {
            [self adjustGlobeZoomBy:0.74];
        }
        return;
    }

    if (terrain_3d_mode_) {
        if ([event scrollingDeltaY] > 0.0) {
            [self adjustTerrain3DZoomBy:0.92];
        } else if ([event scrollingDeltaY] < 0.0) {
            [self adjustTerrain3DZoomBy:1.08];
        }
        return;
    }

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
