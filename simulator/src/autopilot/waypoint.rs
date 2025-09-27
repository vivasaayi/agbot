use bevy::prelude::*;
use bevy::MinimalPlugins;
use crate::components::Drone;
use crate::resources::{MissionData, DroneRegistry};
use crate::drone_controller::spawn_drone;

#[derive(Resource, Debug, Clone)]
pub struct WaypointConfig {
    pub speed_mps: f32,
    pub altitude_m: f32,
    pub pos_tolerance_m: f32,
}

impl Default for WaypointConfig {
    fn default() -> Self {
        Self { speed_mps: 8.0, altitude_m: 20.0, pos_tolerance_m: 1.0 }
    }
}

pub struct WaypointAutopilotPlugin;

impl Plugin for WaypointAutopilotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WaypointConfig::default())
            .add_systems(Startup, spawn_demo_drone_and_waypoints)
            .add_systems(Update, drive_to_waypoints_system);
    }
}

pub struct TestWaypointAutopilotPlugin;

impl Plugin for TestWaypointAutopilotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WaypointConfig::default())
            .add_systems(Update, drive_to_waypoints_system);
    }
}

fn drive_to_waypoints_system(
    cfg: Res<WaypointConfig>,
    mut mission: ResMut<MissionData>,
    mut q: Query<(&mut Transform, &Drone)>,
    time: Res<Time>,
) {
    if mission.waypoints.is_empty() { return; }

    // Follow a single active waypoint index in MissionData
    let idx = mission.replay_index.min(mission.waypoints.len() - 1);
    let target = mission.waypoints[idx];

    for (mut t, _drone) in q.iter_mut() {
        let mut tgt = target;
        tgt.y = cfg.altitude_m; // enforce target altitude
        let delta = tgt - t.translation;
        let dist = delta.length();
        if dist < cfg.pos_tolerance_m {
            // advance waypoint
            if mission.replay_index + 1 < mission.waypoints.len() {
                mission.replay_index += 1;
            }
            continue;
        }

        let dir = delta / dist.max(1e-3);
        let step = cfg.speed_mps * time.delta_seconds();
        t.translation += dir * step.min(dist);
        // orient toward motion in XZ plane
        let fwd = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
        if fwd.length_squared() > 0.0 { t.look_to(fwd, Vec3::Y); }
    }
}

fn spawn_demo_drone_and_waypoints(
    mut commands: Commands,
    mut registry: ResMut<DroneRegistry>,
    mut mission: ResMut<MissionData>,
) {
    // Only run once if no drones exist
    if registry.drones.is_empty() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let _drone = spawn_drone(&mut commands, &mut registry, "demo-1".to_string(), start);
    }

    // If no waypoints, create a simple square
    if mission.waypoints.is_empty() {
        let s = 30.0;
        mission.waypoints = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(s, 0.0, 0.0),
            Vec3::new(s, 0.0, s),
            Vec3::new(0.0, 0.0, s),
            Vec3::new(0.0, 0.0, 0.0),
        ];
        mission.replay_index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::MissionData;

    #[test]
    fn test_waypoint_config_default() {
        let config = WaypointConfig::default();
        assert_eq!(config.speed_mps, 8.0);
        assert_eq!(config.altitude_m, 20.0);
        assert_eq!(config.pos_tolerance_m, 1.0);
    }

    #[test]
    fn test_drive_to_waypoints_basic_movement() {
        let mut app = App::new();

        // Add required Bevy plugins for Time resource
        app.add_plugins(MinimalPlugins);

        // Add the test waypoint autopilot plugin (without demo spawning)
        app.add_plugins(TestWaypointAutopilotPlugin);

        // Add required resources
        let mut mission = MissionData::default();
        mission.waypoints = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
        ];
        mission.replay_index = 0;

        let drone_registry = crate::resources::DroneRegistry::default();

        app.insert_resource(WaypointConfig::default());
        app.insert_resource(mission);
        app.insert_resource(drone_registry);

        // Create a test drone entity at a position away from the first waypoint
        app.world_mut().spawn((
            Transform::from_translation(Vec3::new(-5.0, 0.0, 0.0)), // Start 5 units away
            crate::components::Drone {
                id: "test-drone".to_string(),
                drone_type: crate::components::DroneType::Quadcopter,
                status: crate::components::DroneStatus::Flying,
            }
        ));

        // Run multiple updates to allow sufficient movement
        for _ in 0..50 {
            app.update();
        }

        // Check that at least one drone moved closer to the target
        let mut query = app.world_mut().query::<(&Transform, &Drone)>();
        let mut found_movement = false;
        for (transform, drone) in query.iter(&app.world()) {
            if drone.id == "test-drone" {
                // Check if drone moved closer to target (from -5 toward 0)
                if transform.translation.x > -5.0 {
                    found_movement = true;
                }
                break;
            }
        }
        assert!(found_movement, "Test drone did not move toward waypoint");
    }

    #[test]
    fn test_waypoint_advancement() {
        let mut app = App::new();

        // Add required Bevy plugins for Time resource
        app.add_plugins(MinimalPlugins);

        // Add the test waypoint autopilot plugin (without demo spawning)
        app.add_plugins(TestWaypointAutopilotPlugin);

        // Add required resources
        let mut mission = MissionData::default();
        mission.waypoints = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.5, 0.0, 0.0), // Much closer waypoint
        ];
        mission.replay_index = 0;

    let mut config = WaypointConfig::default();
    config.pos_tolerance_m = 1.0; // Larger tolerance
    config.speed_mps = 5.0; // Faster movement
    config.altitude_m = 0.0; // Match waypoint altitude so tolerance can be met

        let drone_registry = crate::resources::DroneRegistry::default();

        app.insert_resource(config);
        app.insert_resource(mission);
        app.insert_resource(drone_registry);

        // Create a test drone entity at a position near the first waypoint
        app.world_mut().spawn((
            Transform::from_translation(Vec3::new(0.1, 0.0, 0.0)), // Slightly away from first waypoint
            crate::components::Drone {
                id: "test-drone".to_string(),
                drone_type: crate::components::DroneType::Quadcopter,
                status: crate::components::DroneStatus::Flying,
            }
        ));

        // Run multiple updates to ensure waypoint advancement
        for _ in 0..10 {
            app.update();
        }

        // Check that waypoint was advanced
        let mission_data = app.world().resource::<MissionData>();
        assert_eq!(mission_data.replay_index, 1);
    }

    #[test]
    fn test_drone_orientation() {
        let mut app = App::new();

        // Add required Bevy plugins for Time resource
        app.add_plugins(MinimalPlugins);

        // Add the test waypoint autopilot plugin (without demo spawning)
        app.add_plugins(TestWaypointAutopilotPlugin);

        // Add required resources
        let mut mission = MissionData::default();
        mission.waypoints = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
        ];
        mission.replay_index = 0;

        let drone_registry = crate::resources::DroneRegistry::default();

        app.insert_resource(WaypointConfig::default());
        app.insert_resource(mission);
        app.insert_resource(drone_registry);

        // Create a test drone entity at a position away from the first waypoint
        app.world_mut().spawn((
            Transform::from_translation(Vec3::new(-5.0, 0.0, 0.0)), // Start away from waypoint
            crate::components::Drone {
                id: "test-drone".to_string(),
                drone_type: crate::components::DroneType::Quadcopter,
                status: crate::components::DroneStatus::Flying,
            }
        ));

        // Run the system
        app.update();

        // Check that at least one drone is oriented toward the movement direction
        let mut query = app.world_mut().query::<&Transform>();
        let mut found_oriented_drone = false;
        for transform in query.iter(&app.world()) {
            let forward = transform.forward();
            if forward.x > 0.0 && forward.z.abs() < 0.1 {
                found_oriented_drone = true;
                break;
            }
        }
        assert!(found_oriented_drone, "No drone properly oriented toward waypoint");
    }

    #[test]
    fn test_no_movement_when_no_waypoints() {
        let mut app = App::new();

        // Add required Bevy plugins for Time resource
        app.add_plugins(MinimalPlugins);

        // Add the test waypoint autopilot plugin (without demo spawning)
        app.add_plugins(TestWaypointAutopilotPlugin);

        // Add required resources with empty waypoints
        let mission = MissionData::default(); // waypoints is empty by default
        let drone_registry = crate::resources::DroneRegistry::default();

        app.insert_resource(WaypointConfig::default());
        app.insert_resource(mission);
        app.insert_resource(drone_registry);

        // Create a test drone entity
        app.world_mut().spawn((
            Transform::from_translation(Vec3::new(5.0, 5.0, 5.0)),
            crate::components::Drone {
                id: "test-drone".to_string(),
                drone_type: crate::components::DroneType::Quadcopter,
                status: crate::components::DroneStatus::Flying,
            }
        ));

        // Get initial position of our test drone
        let mut query = app.world_mut().query::<(&Transform, &Drone)>();
        let mut initial_position = None;
        for (transform, drone) in query.iter(&app.world()) {
            if drone.id == "test-drone" {
                initial_position = Some(transform.translation);
                break;
            }
        }

        // Run the system
        app.update();

        // Check that our test drone didn't move
        let mut query = app.world_mut().query::<(&Transform, &Drone)>();
        let mut current_position = None;
        for (transform, drone) in query.iter(&app.world()) {
            if drone.id == "test-drone" {
                current_position = Some(transform.translation);
                break;
            }
        }

        assert_eq!(initial_position, current_position, "Test drone moved when no waypoints present");
    }
}
