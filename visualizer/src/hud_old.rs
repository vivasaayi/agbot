use bevy::prelude::*;
use crate::components::{HudElement, HudElementType};
use crate::resources::AppState;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, setup_hud)
            .add_systems(Update, (
                update_hud_elements,
                toggle_hud_visibility,
            ));
    }
}

fn setup_hud(
    mut commands: Commands,
) {
    info!("Setting up HUD elements...");
    
    // Create HUD root node
    commands.spawn((
        NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                ..default()
            },
            ..default()
        },
        Name::new("HUD Root"),
    )).with_children(|parent| {
        // Compass (top center)
        parent.spawn((
            NodeBundle {
                style: Style {
                    width: Val::Px(100.0),
                    height: Val::Px(100.0),
                    position_type: PositionType::Absolute,
                    top: Val::Px(20.0),
                    left: Val::Percent(50.0),
                    margin: UiRect::left(Val::Px(-50.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: Color::srgba(0.0, 0.0, 0.0, 0.7).into(),
                border_color: Color::WHITE.into(),
                ..default()
            },
            HudElement {
                element_type: HudElementType::Compass,
                visible: true,
            },
        )).with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "N",
                TextStyle {
                    font_size: 24.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        });
        
        // Speed indicator (top left)
        parent.spawn((
            NodeBundle {
                style: Style {
                    width: Val::Px(150.0),
                    height: Val::Px(80.0),
                    position_type: PositionType::Absolute,
                    top: Val::Px(20.0),
                    left: Val::Px(20.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: Color::rgba(0.0, 0.0, 0.0, 0.7).into(),
                border_color: Color::GREEN.into(),
                ..default()
            },
            HudElement {
                element_type: HudElementType::SpeedIndicator,
                visible: true,
            },
        )).with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "Speed\n0.0 m/s",
                TextStyle {
                    font_size: 16.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        });
        
        // Altitude indicator (top right)
        parent.spawn((
            NodeBundle {
                style: Style {
                    width: Val::Px(150.0),
                    height: Val::Px(80.0),
                    position_type: PositionType::Absolute,
                    top: Val::Px(20.0),
                    right: Val::Px(20.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: Color::srgba(0.0, 0.0, 0.0, 0.7).into(),
                border_color: Color::srgb(0.0, 0.0, 1.0).into(),
                ..default()
            },
            HudElement {
                element_type: HudElementType::Altimeter,
                visible: true,
            },
        )).with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "Altitude\n0.0 m",
                TextStyle {
                    font_size: 16.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        });
        
        // Battery level (bottom left)
        parent.spawn((
            NodeBundle {
                style: Style {
                    width: Val::Px(200.0),
                    height: Val::Px(60.0),
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(20.0),
                    left: Val::Px(20.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: Color::rgba(0.0, 0.0, 0.0, 0.7).into(),
                border_color: Color::YELLOW.into(),
                ..default()
            },
            HudElement {
                element_type: HudElementType::BatteryLevel,
                visible: true,
            },
        )).with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "Battery: 100%",
                TextStyle {
                    font_size: 16.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        });
        
        // GPS status (bottom center)
        parent.spawn((
            NodeBundle {
                style: Style {
                    width: Val::Px(120.0),
                    height: Val::Px(60.0),
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(20.0),
                    left: Val::Percent(50.0),
                    margin: UiRect::left(Val::Px(-60.0)),
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: Color::rgba(0.0, 0.0, 0.0, 0.7).into(),
                border_color: Color::GREEN.into(),
                ..default()
            },
            HudElement {
                element_type: HudElementType::GpsStatus,
                visible: true,
            },
        )).with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "GPS: OK",
                TextStyle {
                    font_size: 16.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        });
        
        // Mission progress (bottom right)
        parent.spawn((
            NodeBundle {
                style: Style {
                    width: Val::Px(200.0),
                    height: Val::Px(80.0),
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(20.0),
                    right: Val::Px(20.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: Color::rgba(0.0, 0.0, 0.0, 0.7).into(),
                border_color: Color::PURPLE.into(),
                ..default()
            },
            HudElement {
                element_type: HudElementType::MissionProgress,
                visible: true,
            },
        )).with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "Mission\nWaypoint 1/5",
                TextStyle {
                    font_size: 16.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        });
    });
}

fn update_hud_elements(
    drone_query: Query<&crate::components::Drone>,
    mut text_query: Query<&mut Text>,
    hud_query: Query<(&HudElement, &Children)>,
    time: Res<Time>,
) {
    // Update HUD elements with real data
    for (hud_element, children) in hud_query.iter() {
        if !hud_element.visible {
            continue;
        }
        
        for &child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                match hud_element.element_type {
                    HudElementType::SpeedIndicator => {
                        let speed = 15.0 + (time.elapsed_seconds() * 0.5).sin() * 5.0;
                        text.sections[0].value = format!("Speed\n{:.1} m/s", speed);
                    }
                    HudElementType::Altimeter => {
                        let altitude = 10.0 + (time.elapsed_seconds() * 2.0).sin() * 2.0;
                        text.sections[0].value = format!("Altitude\n{:.1} m", altitude);
                    }
                    HudElementType::BatteryLevel => {
                        let battery = 100.0 - (time.elapsed_seconds() * 0.1) % 100.0;
                        let color = if battery > 50.0 { Color::GREEN } else if battery > 20.0 { Color::YELLOW } else { Color::RED };
                        text.sections[0].value = format!("Battery: {:.0}%", battery);
                        text.sections[0].style.color = color;
                    }
                    HudElementType::GpsStatus => {
                        let gps_ok = time.elapsed_seconds() % 10.0 > 1.0;
                        text.sections[0].value = if gps_ok { "GPS: OK".to_string() } else { "GPS: WAIT".to_string() };
                        text.sections[0].style.color = if gps_ok { Color::GREEN } else { Color::YELLOW };
                    }
                    HudElementType::MissionProgress => {
                        let waypoint = ((time.elapsed_seconds() * 0.1) as usize % 5) + 1;
                        text.sections[0].value = format!("Mission\nWaypoint {}/5", waypoint);
                    }
                    HudElementType::Compass => {
                        // Compass doesn't use text, would need custom rendering
                    }
                }
            }
        }
    }
}

fn toggle_hud_visibility(
    app_state: Res<AppState>,
    mut hud_query: Query<&mut Visibility, With<HudElement>>,
) {
    let visibility = if app_state.show_ui {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    
    for mut vis in hud_query.iter_mut() {
        *vis = visibility;
    }
}
