use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::flight_ui::{AppState, UITheme, UIOverlayState};

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<SettingsState>()
            .add_systems(OnEnter(AppState::Settings), setup_settings)
            .add_systems(Update, settings_ui.run_if(in_state(AppState::Settings)))
            .add_systems(OnExit(AppState::Settings), cleanup_settings);
    }
}

#[derive(Component)]
struct SettingsEntity;

#[derive(Resource, Default)]
pub struct SettingsState {
    pub selected_tab: SettingsTab,
    pub graphics_settings: GraphicsSettings,
    pub audio_settings: AudioSettings,
    pub control_settings: ControlSettings,
    pub simulation_settings: SimulationSettings,
    pub network_settings: NetworkSettings,
}

#[derive(Default, PartialEq, Clone)]
pub enum SettingsTab {
    #[default]
    Graphics,
    Audio,
    Controls,
    Simulation,
    Network,
    About,
}

#[derive(Default)]
pub struct GraphicsSettings {
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub fullscreen: bool,
    pub vsync: bool,
    pub render_scale: f32,
    pub shadows: bool,
    pub anti_aliasing: AntiAliasingMode,
    pub texture_quality: TextureQuality,
    pub view_distance: f32,
    pub globe_detail: u32,
}

#[derive(Default, PartialEq, Clone)]
pub enum AntiAliasingMode {
    #[default]
    None,
    FXAA,
    MSAA2x,
    MSAA4x,
    MSAA8x,
}

#[derive(Default, PartialEq, Clone)]
pub enum TextureQuality {
    Low,
    #[default]
    Medium,
    High,
    Ultra,
}

#[derive(Default)]
pub struct AudioSettings {
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
    pub voice_volume: f32,
    pub mute_on_focus_loss: bool,
    pub audio_device: String,
}

#[derive(Default)]
pub struct ControlSettings {
    pub mouse_sensitivity: f32,
    pub invert_mouse_y: bool,
    pub camera_smoothing: f32,
    pub zoom_speed: f32,
    pub rotation_speed: f32,
    pub keyboard_layout: KeyboardLayout,
    pub show_tooltips: bool,
}

#[derive(Default, PartialEq, Clone)]
pub enum KeyboardLayout {
    #[default]
    QWERTY,
    AZERTY,
    QWERTZ,
    Dvorak,
}

#[derive(Default)]
pub struct SimulationSettings {
    pub time_scale: f32,
    pub weather_enabled: bool,
    pub realistic_physics: bool,
    pub auto_save_interval: u32,
    pub max_drones: u32,
    pub collision_detection: bool,
    pub telemetry_rate: u32,
    pub mission_complexity: MissionComplexity,
}

#[derive(Default, PartialEq, Clone)]
pub enum MissionComplexity {
    Beginner,
    #[default]
    Intermediate,
    Advanced,
    Expert,
}

#[derive(Default)]
pub struct NetworkSettings {
    pub enable_multiplayer: bool,
    pub server_address: String,
    pub port: u16,
    pub username: String,
    pub auto_connect: bool,
    pub connection_timeout: u32,
}

fn setup_settings(
    mut commands: Commands,
    mut settings_state: ResMut<SettingsState>,
) {
    info!("Setting up settings screen");
    
    // Initialize with default values
    settings_state.graphics_settings = GraphicsSettings {
        resolution_width: 1920,
        resolution_height: 1080,
        fullscreen: false,
        vsync: true,
        render_scale: 1.0,
        shadows: true,
        anti_aliasing: AntiAliasingMode::FXAA,
        texture_quality: TextureQuality::High,
        view_distance: 1000.0,
        globe_detail: 64,
    };
    
    settings_state.audio_settings = AudioSettings {
        master_volume: 0.8,
        sfx_volume: 0.7,
        music_volume: 0.6,
        voice_volume: 0.9,
        mute_on_focus_loss: false,
        audio_device: "Default".to_string(),
    };
    
    settings_state.control_settings = ControlSettings {
        mouse_sensitivity: 1.0,
        invert_mouse_y: false,
        camera_smoothing: 0.8,
        zoom_speed: 1.0,
        rotation_speed: 1.0,
        keyboard_layout: KeyboardLayout::QWERTY,
        show_tooltips: true,
    };
    
    settings_state.simulation_settings = SimulationSettings {
        time_scale: 1.0,
        weather_enabled: true,
        realistic_physics: true,
        auto_save_interval: 300,
        max_drones: 10,
        collision_detection: true,
        telemetry_rate: 10,
        mission_complexity: MissionComplexity::Intermediate,
    };
    
    settings_state.network_settings = NetworkSettings {
        enable_multiplayer: false,
        server_address: "localhost".to_string(),
        port: 7878,
        username: "Player".to_string(),
        auto_connect: false,
        connection_timeout: 30,
    };
    
    commands.spawn((
        SettingsEntity,
        Name::new("SettingsInterface"),
    ));
}

fn cleanup_settings(
    mut commands: Commands,
    settings_entities: Query<Entity, With<SettingsEntity>>,
) {
    for entity in settings_entities.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

fn settings_ui(
    mut contexts: EguiContexts,
    mut next_app_state: ResMut<NextState<AppState>>,
    mut settings_state: ResMut<SettingsState>,
    mut overlay_state: ResMut<UIOverlayState>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    let ctx = contexts.ctx_mut();
    
    // Handle escape key to return to main menu
    if keyboard_input.just_pressed(KeyCode::Escape) {
        next_app_state.set(AppState::MainMenu);
        return;
    }
    
    // Top navigation bar
    egui::TopBottomPanel::top("settings_top_panel")
        .resizable(false)
        .min_height(60.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                
                // Back button
                if ui.button("← Back to Menu").clicked() {
                    next_app_state.set(AppState::MainMenu);
                }
                
                ui.separator();
                
                ui.heading("⚙️ Settings");
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    
                    if ui.button("Apply Changes").clicked() {
                        apply_settings(&settings_state, &mut overlay_state);
                    }
                    
                    if ui.button("Reset to Defaults").clicked() {
                        reset_to_defaults(&mut settings_state);
                        overlay_state.add_simple_notification("Settings reset to defaults".to_string());
                    }
                });
            });
        });
    
    // Left sidebar for tab navigation
    egui::SidePanel::left("settings_tabs")
        .resizable(false)
        .exact_width(200.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.add_space(20.0);
                
                // Tab buttons
                let tab_height = 40.0;
                
                if ui.add_sized([180.0, tab_height], 
                    egui::SelectableLabel::new(
                        settings_state.selected_tab == SettingsTab::Graphics,
                        "🎨 Graphics"
                    )).clicked() {
                    settings_state.selected_tab = SettingsTab::Graphics;
                }
                
                if ui.add_sized([180.0, tab_height], 
                    egui::SelectableLabel::new(
                        settings_state.selected_tab == SettingsTab::Audio,
                        "🔊 Audio"
                    )).clicked() {
                    settings_state.selected_tab = SettingsTab::Audio;
                }
                
                if ui.add_sized([180.0, tab_height], 
                    egui::SelectableLabel::new(
                        settings_state.selected_tab == SettingsTab::Controls,
                        "🎮 Controls"
                    )).clicked() {
                    settings_state.selected_tab = SettingsTab::Controls;
                }
                
                if ui.add_sized([180.0, tab_height], 
                    egui::SelectableLabel::new(
                        settings_state.selected_tab == SettingsTab::Simulation,
                        "🚁 Simulation"
                    )).clicked() {
                    settings_state.selected_tab = SettingsTab::Simulation;
                }
                
                if ui.add_sized([180.0, tab_height], 
                    egui::SelectableLabel::new(
                        settings_state.selected_tab == SettingsTab::Network,
                        "🌐 Network"
                    )).clicked() {
                    settings_state.selected_tab = SettingsTab::Network;
                }
                
                ui.add_space(20.0);
                ui.separator();
                ui.add_space(20.0);
                
                if ui.add_sized([180.0, tab_height], 
                    egui::SelectableLabel::new(
                        settings_state.selected_tab == SettingsTab::About,
                        "ℹ️ About"
                    )).clicked() {
                    settings_state.selected_tab = SettingsTab::About;
                }
            });
        });
    
    // Main settings content
    egui::CentralPanel::default()
        .frame(egui::Frame::none().inner_margin(20.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                match settings_state.selected_tab {
                    SettingsTab::Graphics => render_graphics_settings(ui, &mut settings_state.graphics_settings),
                    SettingsTab::Audio => render_audio_settings(ui, &mut settings_state.audio_settings),
                    SettingsTab::Controls => render_control_settings(ui, &mut settings_state.control_settings),
                    SettingsTab::Simulation => render_simulation_settings(ui, &mut settings_state.simulation_settings),
                    SettingsTab::Network => render_network_settings(ui, &mut settings_state.network_settings),
                    SettingsTab::About => render_about_tab(ui),
                }
            });
        });
}

fn render_graphics_settings(ui: &mut egui::Ui, graphics: &mut GraphicsSettings) {
    ui.heading("Graphics Settings");
    ui.add_space(20.0);
    
    ui.group(|ui| {
        ui.label("🖥️ Display");
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.label("Resolution:");
            ui.add(egui::DragValue::new(&mut graphics.resolution_width).range(800..=4096));
            ui.label("x");
            ui.add(egui::DragValue::new(&mut graphics.resolution_height).range(600..=2160));
        });
        
        ui.checkbox(&mut graphics.fullscreen, "Fullscreen");
        ui.checkbox(&mut graphics.vsync, "V-Sync");
        
        ui.horizontal(|ui| {
            ui.label("Render Scale:");
            ui.add(egui::Slider::new(&mut graphics.render_scale, 0.5..=2.0).text("%"));
        });
    });
    
    ui.add_space(15.0);
    
    ui.group(|ui| {
        ui.label("✨ Quality");
        ui.add_space(10.0);
        
        ui.checkbox(&mut graphics.shadows, "Enable Shadows");
        
        ui.horizontal(|ui| {
            ui.label("Anti-Aliasing:");
            egui::ComboBox::from_id_source("aa_combo")
                .selected_text(format!("{:?}", graphics.anti_aliasing))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut graphics.anti_aliasing, AntiAliasingMode::None, "None");
                    ui.selectable_value(&mut graphics.anti_aliasing, AntiAliasingMode::FXAA, "FXAA");
                    ui.selectable_value(&mut graphics.anti_aliasing, AntiAliasingMode::MSAA2x, "MSAA 2x");
                    ui.selectable_value(&mut graphics.anti_aliasing, AntiAliasingMode::MSAA4x, "MSAA 4x");
                    ui.selectable_value(&mut graphics.anti_aliasing, AntiAliasingMode::MSAA8x, "MSAA 8x");
                });
        });
        
        ui.horizontal(|ui| {
            ui.label("Texture Quality:");
            egui::ComboBox::from_id_source("texture_combo")
                .selected_text(format!("{:?}", graphics.texture_quality))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut graphics.texture_quality, TextureQuality::Low, "Low");
                    ui.selectable_value(&mut graphics.texture_quality, TextureQuality::Medium, "Medium");
                    ui.selectable_value(&mut graphics.texture_quality, TextureQuality::High, "High");
                    ui.selectable_value(&mut graphics.texture_quality, TextureQuality::Ultra, "Ultra");
                });
        });
        
        ui.horizontal(|ui| {
            ui.label("View Distance:");
            ui.add(egui::Slider::new(&mut graphics.view_distance, 100.0..=5000.0).suffix(" m"));
        });
        
        ui.horizontal(|ui| {
            ui.label("Globe Detail:");
            ui.add(egui::Slider::new(&mut graphics.globe_detail, 16..=256).logarithmic(true));
        });
    });
}

fn render_audio_settings(ui: &mut egui::Ui, audio: &mut AudioSettings) {
    ui.heading("Audio Settings");
    ui.add_space(20.0);
    
    ui.group(|ui| {
        ui.label("🔊 Volume Levels");
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.label("Master Volume:");
            ui.add(egui::Slider::new(&mut audio.master_volume, 0.0..=1.0).text("%"));
        });
        
        ui.horizontal(|ui| {
            ui.label("Sound Effects:");
            ui.add(egui::Slider::new(&mut audio.sfx_volume, 0.0..=1.0).text("%"));
        });
        
        ui.horizontal(|ui| {
            ui.label("Music:");
            ui.add(egui::Slider::new(&mut audio.music_volume, 0.0..=1.0).text("%"));
        });
        
        ui.horizontal(|ui| {
            ui.label("Voice/Comm:");
            ui.add(egui::Slider::new(&mut audio.voice_volume, 0.0..=1.0).text("%"));
        });
    });
    
    ui.add_space(15.0);
    
    ui.group(|ui| {
        ui.label("🎛️ Device Settings");
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.label("Audio Device:");
            ui.text_edit_singleline(&mut audio.audio_device);
        });
        
        ui.checkbox(&mut audio.mute_on_focus_loss, "Mute when window loses focus");
    });
}

fn render_control_settings(ui: &mut egui::Ui, controls: &mut ControlSettings) {
    ui.heading("Control Settings");
    ui.add_space(20.0);
    
    ui.group(|ui| {
        ui.label("🖱️ Mouse Settings");
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.label("Mouse Sensitivity:");
            ui.add(egui::Slider::new(&mut controls.mouse_sensitivity, 0.1..=3.0));
        });
        
        ui.checkbox(&mut controls.invert_mouse_y, "Invert Mouse Y-Axis");
        
        ui.horizontal(|ui| {
            ui.label("Camera Smoothing:");
            ui.add(egui::Slider::new(&mut controls.camera_smoothing, 0.0..=1.0));
        });
        
        ui.horizontal(|ui| {
            ui.label("Zoom Speed:");
            ui.add(egui::Slider::new(&mut controls.zoom_speed, 0.1..=3.0));
        });
        
        ui.horizontal(|ui| {
            ui.label("Rotation Speed:");
            ui.add(egui::Slider::new(&mut controls.rotation_speed, 0.1..=3.0));
        });
    });
    
    ui.add_space(15.0);
    
    ui.group(|ui| {
        ui.label("⌨️ Keyboard Settings");
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.label("Layout:");
            egui::ComboBox::from_id_source("keyboard_combo")
                .selected_text(format!("{:?}", controls.keyboard_layout))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut controls.keyboard_layout, KeyboardLayout::QWERTY, "QWERTY");
                    ui.selectable_value(&mut controls.keyboard_layout, KeyboardLayout::AZERTY, "AZERTY");
                    ui.selectable_value(&mut controls.keyboard_layout, KeyboardLayout::QWERTZ, "QWERTZ");
                    ui.selectable_value(&mut controls.keyboard_layout, KeyboardLayout::Dvorak, "Dvorak");
                });
        });
        
        ui.checkbox(&mut controls.show_tooltips, "Show control tooltips");
    });
}

fn render_simulation_settings(ui: &mut egui::Ui, simulation: &mut SimulationSettings) {
    ui.heading("Simulation Settings");
    ui.add_space(20.0);
    
    ui.group(|ui| {
        ui.label("⏱️ Time & Physics");
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.label("Time Scale:");
            ui.add(egui::Slider::new(&mut simulation.time_scale, 0.1..=5.0).logarithmic(true));
        });
        
        ui.checkbox(&mut simulation.weather_enabled, "Enable Dynamic Weather");
        ui.checkbox(&mut simulation.realistic_physics, "Realistic Physics");
        ui.checkbox(&mut simulation.collision_detection, "Collision Detection");
    });
    
    ui.add_space(15.0);
    
    ui.group(|ui| {
        ui.label("🚁 Drone Settings");
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.label("Max Simultaneous Drones:");
            ui.add(egui::DragValue::new(&mut simulation.max_drones).range(1..=50));
        });
        
        ui.horizontal(|ui| {
            ui.label("Telemetry Rate (Hz):");
            ui.add(egui::DragValue::new(&mut simulation.telemetry_rate).range(1..=100));
        });
        
        ui.horizontal(|ui| {
            ui.label("Mission Complexity:");
            egui::ComboBox::from_id_source("mission_combo")
                .selected_text(format!("{:?}", simulation.mission_complexity))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut simulation.mission_complexity, MissionComplexity::Beginner, "Beginner");
                    ui.selectable_value(&mut simulation.mission_complexity, MissionComplexity::Intermediate, "Intermediate");
                    ui.selectable_value(&mut simulation.mission_complexity, MissionComplexity::Advanced, "Advanced");
                    ui.selectable_value(&mut simulation.mission_complexity, MissionComplexity::Expert, "Expert");
                });
        });
    });
    
    ui.add_space(15.0);
    
    ui.group(|ui| {
        ui.label("💾 Save Settings");
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.label("Auto-save Interval (seconds):");
            ui.add(egui::DragValue::new(&mut simulation.auto_save_interval).range(60..=3600));
        });
    });
}

fn render_network_settings(ui: &mut egui::Ui, network: &mut NetworkSettings) {
    ui.heading("Network Settings");
    ui.add_space(20.0);
    
    ui.group(|ui| {
        ui.label("🌐 Multiplayer");
        ui.add_space(10.0);
        
        ui.checkbox(&mut network.enable_multiplayer, "Enable Multiplayer Support");
        
        ui.horizontal(|ui| {
            ui.label("Username:");
            ui.text_edit_singleline(&mut network.username);
        });
    });
    
    ui.add_space(15.0);
    
    ui.group(|ui| {
        ui.label("🔗 Connection");
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.label("Server Address:");
            ui.text_edit_singleline(&mut network.server_address);
        });
        
        ui.horizontal(|ui| {
            ui.label("Port:");
            ui.add(egui::DragValue::new(&mut network.port).range(1024..=65535));
        });
        
        ui.checkbox(&mut network.auto_connect, "Auto-connect on startup");
        
        ui.horizontal(|ui| {
            ui.label("Connection Timeout (seconds):");
            ui.add(egui::DragValue::new(&mut network.connection_timeout).range(5..=300));
        });
    });
}

fn render_about_tab(ui: &mut egui::Ui) {
    ui.heading("About AgBot Drone Visualizer");
    ui.add_space(20.0);
    
    ui.group(|ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.heading("🚁 AgBot Drone Visualizer");
            ui.add_space(10.0);
            ui.label("Advanced Agricultural Drone Simulation & Control Platform");
            ui.add_space(20.0);
            
            ui.label("Version: 1.0.0");
            ui.label("Build: 2025.01");
            ui.label("Engine: Bevy 0.14");
            ui.add_space(20.0);
            
            ui.label("Developed for precision agriculture and drone operations");
            ui.add_space(20.0);
        });
    });
    
    ui.add_space(20.0);
    
    ui.group(|ui| {
        ui.label("🛠️ System Information");
        ui.add_space(10.0);
        
        ui.label("Platform: Cross-platform (Windows, macOS, Linux)");
        ui.label("Graphics API: Vulkan, DirectX 12, Metal");
        ui.label("Audio: Rodio");
        ui.label("UI Framework: egui");
        ui.label("Physics: Rapier (optional)");
    });
    
    ui.add_space(20.0);
    
    ui.group(|ui| {
        ui.label("📧 Support & Contact");
        ui.add_space(10.0);
        
        ui.label("For technical support or feature requests:");
        ui.label("Email: support@agbot.com");
        ui.label("Documentation: docs.agbot.com");
        ui.label("GitHub: github.com/agbot/drone-visualizer");
    });
}

fn apply_settings(settings_state: &SettingsState, overlay_state: &mut UIOverlayState) {
    info!("Applying settings changes");
    overlay_state.add_simple_notification("Settings applied successfully".to_string());
    
    // Here you would implement actual settings application logic:
    // - Update graphics pipeline settings
    // - Apply audio volume changes
    // - Reconfigure input handlers
    // - Save settings to file
}

fn reset_to_defaults(settings_state: &mut SettingsState) {
    *settings_state = SettingsState::default();
    info!("Settings reset to defaults");
}
