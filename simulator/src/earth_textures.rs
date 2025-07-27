use bevy::prelude::*;

#[derive(Resource)]
pub struct EarthTextures {
    pub daymap: Option<Handle<Image>>,
    pub normalmap: Option<Handle<Image>>,
    pub specular: Option<Handle<Image>>,
    pub nightmap: Option<Handle<Image>>,
    pub clouds: Option<Handle<Image>>,
    pub loading_complete: bool,
}

impl Default for EarthTextures {
    fn default() -> Self {
        Self {
            daymap: None,
            normalmap: None,
            specular: None,
            nightmap: None,
            clouds: None,
            loading_complete: false,
        }
    }
}

pub fn load_earth_textures(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    info!("Loading Earth texture maps...");
    
    // Try to load textures, but only store handles for files that exist
    let earth_textures = EarthTextures {
        daymap: Some(asset_server.load("textures/earth_daymap_2k.png")),
        normalmap: None, // Will try to load but expect it might fail
        specular: None,  // Will try to load but expect it might fail
        nightmap: None,  // Will try to load but expect it might fail
        clouds: None,    // Will try to load but expect it might fail
        loading_complete: false,
    };
    
    commands.insert_resource(earth_textures);
}

pub fn check_texture_loading(
    mut earth_textures: ResMut<EarthTextures>,
    asset_server: Res<AssetServer>,
) {
    if earth_textures.loading_complete {
        return;
    }
    
    // For now, only require the daymap texture to be loaded
    // Other textures are optional and can fail gracefully
    if let Some(ref handle) = earth_textures.daymap {
        if asset_server.get_load_state(handle) == Some(bevy::asset::LoadState::Loaded) {
            earth_textures.loading_complete = true;
            info!("Earth daymap texture loaded successfully!");
        }
    }
}

pub fn update_earth_material_with_textures(
    earth_textures: Res<EarthTextures>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    globe_query: Query<&Handle<StandardMaterial>, With<crate::globe_view::Globe>>,
) {
    if !earth_textures.loading_complete {
        return;
    }
    
    // Update the globe material with loaded textures
    for material_handle in globe_query.iter() {
        if let Some(material) = materials.get_mut(material_handle) {
            // Apply day map if available
            if let Some(ref daymap) = earth_textures.daymap {
                material.base_color_texture = Some(daymap.clone());
                material.base_color = Color::WHITE; // Reset to white to show texture colors
                info!("Applied Earth daymap texture!");
            }
            
            // Apply normal map for surface detail (if available)
            if let Some(ref normalmap) = earth_textures.normalmap {
                material.normal_map_texture = Some(normalmap.clone());
                info!("Applied Earth normal map texture!");
            }
            
            // Apply specular map for water reflection (if available)
            if let Some(ref specular) = earth_textures.specular {
                // Note: In PBR, we use metallic_roughness_texture
                // White areas = metallic (water), black = non-metallic (land)
                material.metallic_roughness_texture = Some(specular.clone());
                info!("Applied Earth specular map texture!");
            }
            
            // Apply night map as emissive for city lights (if available)
            if let Some(ref nightmap) = earth_textures.nightmap {
                material.emissive_texture = Some(nightmap.clone());
                material.emissive = Color::srgb(0.3, 0.3, 0.3).into(); // Subtle night glow
                info!("Applied Earth night map texture!");
            }
            
            info!("Earth material updated with realistic textures!");
        }
    }
}
