//! Real World Demo
//!
//! Demo system to test loading real-world terrain.
//! Press F5 to load a Nebraska farm, F6 for Iowa, F7 for California.
//! Press N to toggle NDVI overlay, C to toggle CDL overlay.

use bevy::prelude::*;

use crate::gis::LoadRealWorldEvent;
use crate::gis::terrain_mesh::{TerrainMeshConfig, TerrainOverlay, RealTerrain};
use crate::gis::ndvi::NdviConfig;
use crate::gis::cdl::CdlConfig;
use crate::gis::osm::OsmConfig;

pub struct RealWorldDemoPlugin;

impl Plugin for RealWorldDemoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            demo_keyboard_handler,
            overlay_toggle_handler,
        ));
    }
}

fn demo_keyboard_handler(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut load_events: EventWriter<LoadRealWorldEvent>,
) {
    // Support both function keys and top-row digits (Digit1..Digit4)
    if keyboard.just_pressed(KeyCode::F5) || keyboard.just_pressed(KeyCode::Digit1) {
        info!("Loading Nebraska farm (F5 / 1)...");
        load_events.send(LoadRealWorldEvent::nebraska_farm());
    }
    if keyboard.just_pressed(KeyCode::F6) || keyboard.just_pressed(KeyCode::Digit2) {
        info!("Loading Iowa corn belt (F6 / 2)...");
        load_events.send(LoadRealWorldEvent::iowa_cornbelt());
    }
    if keyboard.just_pressed(KeyCode::F7) || keyboard.just_pressed(KeyCode::Digit3) {
        info!("Loading California Central Valley (F7 / 3)...");
        load_events.send(LoadRealWorldEvent::california_valley());
    }
    if keyboard.just_pressed(KeyCode::F8) || keyboard.just_pressed(KeyCode::Digit4) {
        info!("Loading Salinas Valley (F8 / 4)...");
        load_events.send(LoadRealWorldEvent::new(36.677, -121.655, 3000.0));
    }
}

/// Handle overlay toggle keyboard shortcuts
fn overlay_toggle_handler(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut terrain_config: ResMut<TerrainMeshConfig>,
    mut ndvi_config: ResMut<NdviConfig>,
    mut cdl_config: ResMut<CdlConfig>,
    mut osm_config: ResMut<OsmConfig>,
    mut terrain_query: Query<&mut RealTerrain>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    material_query: Query<&Handle<StandardMaterial>, With<RealTerrain>>,
) {
    // Toggle NDVI overlay with N key
    if keyboard.just_pressed(KeyCode::KeyN) {
        terrain_config.show_ndvi = !terrain_config.show_ndvi;
        ndvi_config.enabled = terrain_config.show_ndvi;
        
        info!("NDVI overlay: {}", if terrain_config.show_ndvi { "ON" } else { "OFF" });
        
        // Update terrain overlay mode
        for mut terrain in terrain_query.iter_mut() {
            if terrain_config.show_ndvi {
                terrain.active_overlay = TerrainOverlay::BlendedNdvi;
                
                // Switch material texture to NDVI
                for mat_handle in material_query.iter() {
                    if let Some(material) = materials.get_mut(mat_handle) {
                        if let Some(ref ndvi_tex) = terrain.ndvi_texture {
                            material.base_color_texture = Some(ndvi_tex.clone());
                        }
                    }
                }
            } else {
                terrain.active_overlay = TerrainOverlay::None;
                
                // Switch back to base texture
                for mat_handle in material_query.iter() {
                    if let Some(material) = materials.get_mut(mat_handle) {
                        if let Some(ref base_tex) = terrain.base_texture {
                            material.base_color_texture = Some(base_tex.clone());
                        }
                    }
                }
            }
        }
    }
    
    // Toggle CDL overlay with C key
    if keyboard.just_pressed(KeyCode::KeyC) {
        terrain_config.show_cdl = !terrain_config.show_cdl;
        cdl_config.enabled = terrain_config.show_cdl;
        
        info!("CDL overlay: {}", if terrain_config.show_cdl { "ON" } else { "OFF" });
        
        // Update terrain overlay mode
        for mut terrain in terrain_query.iter_mut() {
            if terrain_config.show_cdl {
                terrain.active_overlay = TerrainOverlay::Cdl;
                
                // Switch material texture to CDL
                for mat_handle in material_query.iter() {
                    if let Some(material) = materials.get_mut(mat_handle) {
                        if let Some(ref cdl_tex) = terrain.cdl_texture {
                            material.base_color_texture = Some(cdl_tex.clone());
                        }
                    }
                }
            } else {
                terrain.active_overlay = TerrainOverlay::None;
                
                // Switch back to base texture
                for mat_handle in material_query.iter() {
                    if let Some(material) = materials.get_mut(mat_handle) {
                        if let Some(ref base_tex) = terrain.base_texture {
                            material.base_color_texture = Some(base_tex.clone());
                        }
                    }
                }
            }
        }
    }
    
    // Toggle OSM features with O key
    if keyboard.just_pressed(KeyCode::KeyO) {
        terrain_config.show_osm = !terrain_config.show_osm;
        osm_config.enabled = terrain_config.show_osm;
        
        info!("OSM features: {}", if terrain_config.show_osm { "ON" } else { "OFF" });
    }
    
    // Adjust overlay opacity with [ and ] keys
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        terrain_config.overlay_opacity = (terrain_config.overlay_opacity - 0.1).max(0.0);
        ndvi_config.opacity = terrain_config.overlay_opacity;
        cdl_config.opacity = terrain_config.overlay_opacity;
        info!("Overlay opacity: {:.1}", terrain_config.overlay_opacity);
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        terrain_config.overlay_opacity = (terrain_config.overlay_opacity + 0.1).min(1.0);
        ndvi_config.opacity = terrain_config.overlay_opacity;
        cdl_config.opacity = terrain_config.overlay_opacity;
        info!("Overlay opacity: {:.1}", terrain_config.overlay_opacity);
    }
}

/// Add a UI panel to select locations
pub fn real_world_location_ui(
    ui: &mut bevy_egui::egui::Ui,
    load_events: &mut EventWriter<LoadRealWorldEvent>,
) {
    ui.heading("🌍 Load Real World Location");
    
    ui.horizontal(|ui| {
        if ui.button("Nebraska Farm").clicked() {
            load_events.send(LoadRealWorldEvent::nebraska_farm());
        }
        if ui.button("Iowa Corn Belt").clicked() {
            load_events.send(LoadRealWorldEvent::iowa_cornbelt());
        }
    });
    
    ui.horizontal(|ui| {
        if ui.button("California Valley").clicked() {
            load_events.send(LoadRealWorldEvent::california_valley());
        }
        if ui.button("Salinas Valley").clicked() {
            load_events.send(LoadRealWorldEvent::new(36.677, -121.655, 3000.0));
        }
    });
    
    ui.separator();
    ui.label("Shortcuts: 1=Nebraska, 2=Iowa, 3=California, 4=Salinas (also F5-F8)");
}

/// Add a UI panel for overlay controls
pub fn overlay_controls_ui(
    ui: &mut bevy_egui::egui::Ui,
    terrain_config: &mut TerrainMeshConfig,
    ndvi_config: &mut NdviConfig,
    cdl_config: &mut CdlConfig,
    osm_config: &mut OsmConfig,
) {
    ui.heading("🗺️ Overlay Controls");
    
    // NDVI toggle
    let mut show_ndvi = terrain_config.show_ndvi;
    if ui.checkbox(&mut show_ndvi, "🌿 NDVI Vegetation Index").changed() {
        terrain_config.show_ndvi = show_ndvi;
        ndvi_config.enabled = show_ndvi;
    }
    
    // CDL toggle
    let mut show_cdl = terrain_config.show_cdl;
    if ui.checkbox(&mut show_cdl, "🌾 Crop Classification (CDL)").changed() {
        terrain_config.show_cdl = show_cdl;
        cdl_config.enabled = show_cdl;
    }
    
    // OSM toggle
    let mut show_osm = terrain_config.show_osm;
    if ui.checkbox(&mut show_osm, "🏠 OSM Features").changed() {
        terrain_config.show_osm = show_osm;
        osm_config.enabled = show_osm;
    }
    
    ui.separator();
    
    // Opacity slider
    ui.horizontal(|ui| {
        ui.label("Opacity:");
        let mut opacity = terrain_config.overlay_opacity;
        if ui.add(bevy_egui::egui::Slider::new(&mut opacity, 0.0..=1.0)).changed() {
            terrain_config.overlay_opacity = opacity;
            ndvi_config.opacity = opacity;
            cdl_config.opacity = opacity;
        }
    });
    
    // NDVI color scheme
    if terrain_config.show_ndvi {
        ui.horizontal(|ui| {
            ui.label("NDVI Style:");
            bevy_egui::egui::ComboBox::from_label("")
                .selected_text(match ndvi_config.color_scheme {
                    super::ndvi::NdviColorScheme::Agriculture => "Agriculture",
                    super::ndvi::NdviColorScheme::Scientific => "Scientific",
                    super::ndvi::NdviColorScheme::StressDetection => "Stress Detection",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut ndvi_config.color_scheme, super::ndvi::NdviColorScheme::Agriculture, "Agriculture");
                    ui.selectable_value(&mut ndvi_config.color_scheme, super::ndvi::NdviColorScheme::Scientific, "Scientific");
                    ui.selectable_value(&mut ndvi_config.color_scheme, super::ndvi::NdviColorScheme::StressDetection, "Stress Detection");
                });
        });
    }
    
    ui.separator();
    ui.label("Shortcuts: N=NDVI, C=CDL, O=OSM, [/]=opacity");
}
