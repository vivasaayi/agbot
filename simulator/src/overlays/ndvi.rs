use crate::components::{SensorOverlay, TerrainTile};
use crate::resources::AppConfig;
use bevy::prelude::*;

#[derive(Resource, Default, Debug, Clone)]
pub struct NdviOverlayConfig {
    pub enabled: bool,
}

pub struct NdviOverlayPlugin;

impl Plugin for NdviOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NdviOverlayConfig { enabled: true })
            .add_systems(Update, ndvi_tint_system);
    }
}

fn ndvi_tint_system(
    cfg: Res<NdviOverlayConfig>,
    app_cfg: Res<AppConfig>,
    mut q: Query<(&SensorOverlay, &mut Handle<StandardMaterial>), With<TerrainTile>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !cfg.enabled || !app_cfg.rendering.show_ndvi_overlay {
        return;
    }
    for (overlay, mat_h) in q.iter_mut() {
        if let Some(mat) = materials.get_mut(&*mat_h) {
            let ndvi = overlay.ndvi_value.clamp(0.0, 1.0);
            mat.base_color = Color::srgb(1.0 - ndvi, ndvi, 0.0);
        }
    }
}
