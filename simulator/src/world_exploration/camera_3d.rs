use bevy::prelude::*;

pub struct Camera3DPlugin;

impl Plugin for Camera3DPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<Camera3DState>()
            .add_systems(Update, (
                camera_3d_controls,
                smooth_camera_transitions,
            ));
    }
}

#[derive(Resource, Default)]
pub struct Camera3DState {
    pub target_position: Option<Vec3>,
    pub target_look_at: Option<Vec3>,
    pub transition_speed: f32,
    pub is_transitioning: bool,
    pub rotation_sensitivity: f32,
    pub zoom_speed: f32,
    pub movement_speed: f32,
}

impl Camera3DState {
    pub fn new() -> Self {
        Self {
            target_position: None,
            target_look_at: None,
            transition_speed: 2.0,
            is_transitioning: false,
            rotation_sensitivity: 0.003,
            zoom_speed: 2.0,
            movement_speed: 5.0,
        }
    }
    
    /// Start a smooth transition to look at a specific world location
    pub fn transition_to_location(&mut self, location_pos: Vec3, distance: f32) {
        // Calculate camera position to look at the location
        let direction = location_pos.normalize();
        let camera_pos = location_pos + direction * distance;
        
        self.target_position = Some(camera_pos);
        self.target_look_at = Some(location_pos);
        self.is_transitioning = true;
        
        info!("Starting camera transition to location: {:?}", location_pos);
    }
    
    /// Reset camera to default globe viewing position
    pub fn reset_to_default(&mut self) {
        self.target_position = Some(Vec3::new(0.0, 0.0, 5.0));
        self.target_look_at = Some(Vec3::ZERO);
        self.is_transitioning = true;
        
        info!("Resetting camera to default position");
    }
}

#[derive(Component)]
pub struct GlobeCamera;

fn camera_3d_controls(
    mut camera_state: ResMut<Camera3DState>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_wheel: EventReader<bevy::input::mouse::MouseWheel>,
    mut camera_query: Query<&mut Transform, (With<Camera>, With<GlobeCamera>)>,
    time: Res<Time>,
) {
    if let Ok(mut camera_transform) = camera_query.get_single_mut() {
        let dt = time.delta_seconds();
        
        // Handle keyboard movement (WASD)
        let mut movement = Vec3::ZERO;
        if keyboard_input.pressed(KeyCode::KeyW) {
            movement += camera_transform.forward() * camera_state.movement_speed * dt;
        }
        if keyboard_input.pressed(KeyCode::KeyS) {
            movement += camera_transform.back() * camera_state.movement_speed * dt;
        }
        if keyboard_input.pressed(KeyCode::KeyA) {
            movement += camera_transform.left() * camera_state.movement_speed * dt;
        }
        if keyboard_input.pressed(KeyCode::KeyD) {
            movement += camera_transform.right() * camera_state.movement_speed * dt;
        }
        
        // Apply movement if not transitioning
        if !camera_state.is_transitioning && movement != Vec3::ZERO {
            camera_transform.translation += movement;
        }
        
        // Handle mouse wheel zoom
        for wheel_event in mouse_wheel.read() {
            if !camera_state.is_transitioning {
                let zoom_delta = wheel_event.y * camera_state.zoom_speed * dt;
                let forward = camera_transform.forward();
                camera_transform.translation += forward * zoom_delta;
                
                // Clamp zoom distance to prevent going inside the globe
                let distance_to_origin = camera_transform.translation.length();
                if distance_to_origin < 1.5 {
                    camera_transform.translation = camera_transform.translation.normalize() * 1.5;
                }
            }
        }
        
        // Handle space key for reset
        if keyboard_input.just_pressed(KeyCode::Space) {
            camera_state.reset_to_default();
        }
    }
}

fn smooth_camera_transitions(
    mut camera_state: ResMut<Camera3DState>,
    mut camera_query: Query<&mut Transform, (With<Camera>, With<GlobeCamera>)>,
    time: Res<Time>,
) {
    if !camera_state.is_transitioning {
        return;
    }
    
    if let Ok(mut camera_transform) = camera_query.get_single_mut() {
        let dt = time.delta_seconds();
        let transition_factor = 1.0 - (-camera_state.transition_speed * dt).exp();
        
        let mut transition_complete = true;
        
        // Smooth position transition
        if let Some(target_pos) = camera_state.target_position {
            let distance = camera_transform.translation.distance(target_pos);
            if distance > 0.01 {
                camera_transform.translation = camera_transform.translation.lerp(target_pos, transition_factor);
                transition_complete = false;
            } else {
                camera_transform.translation = target_pos;
            }
        }
        
        // Smooth look-at transition
        if let Some(target_look_at) = camera_state.target_look_at {
            let current_forward = camera_transform.forward();
            let desired_forward = (target_look_at - camera_transform.translation).normalize();
            
            let angle_diff = current_forward.dot(desired_forward).acos();
            if angle_diff > 0.01 {
                camera_transform.look_at(target_look_at, Vec3::Y);
                transition_complete = false;
            }
        }
        
        // Check if transition is complete
        if transition_complete {
            camera_state.is_transitioning = false;
            camera_state.target_position = None;
            camera_state.target_look_at = None;
            info!("Camera transition completed");
        }
    }
}

/// Initialize camera for 3D world exploration
pub fn setup_globe_camera(
    commands: &mut Commands,
) -> Entity {
    let camera_entity = commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        GlobeCamera,
        Name::new("GlobeCamera"),
    )).id();
    
    info!("Globe camera initialized");
    camera_entity
}
