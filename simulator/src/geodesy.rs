use bevy::prelude::*;
use nalgebra::{Matrix3, Vector3};

// WGS84 ellipsoid constants (meters)
pub const WGS84_A: f64 = 6378137.0; // semi-major axis
pub const WGS84_F: f64 = 1.0 / 298.257_223_563; // flattening
pub const WGS84_B: f64 = WGS84_A * (1.0 - WGS84_F); // semi-minor axis
pub const WGS84_E2: f64 = 1.0 - (WGS84_B * WGS84_B) / (WGS84_A * WGS84_A); // eccentricity squared

#[inline]
pub fn deg2rad(d: f64) -> f64 { d.to_radians() }

// Latitude (deg), Longitude (deg), height (m) -> ECEF (m)
pub fn lla_to_ecef(lat_deg: f64, lon_deg: f64, h_m: f64) -> Vector3<f64> {
    let lat = deg2rad(lat_deg);
    let lon = deg2rad(lon_deg);
    let sin_lat = lat.sin();
    let cos_lat = lat.cos();
    let sin_lon = lon.sin();
    let cos_lon = lon.cos();

    // Prime vertical radius of curvature
    let n = WGS84_A / (1.0 - WGS84_E2 * sin_lat * sin_lat).sqrt();

    let x = (n + h_m) * cos_lat * cos_lon;
    let y = (n + h_m) * cos_lat * sin_lon;
    let z = (n * (1.0 - WGS84_E2) + h_m) * sin_lat;
    Vector3::new(x, y, z)
}

// A local tangent frame centered at (lat, lon, h)
#[derive(Debug, Clone, Copy)]
pub struct LocalFrame {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub h_m: f64,
    pub origin_ecef: Vector3<f64>,
    pub ecef_to_enu: Matrix3<f64>,
    pub enu_to_ecef: Matrix3<f64>,
}

impl LocalFrame {
    pub fn new(lat_deg: f64, lon_deg: f64, h_m: f64) -> Self {
        let origin_ecef = lla_to_ecef(lat_deg, lon_deg, h_m);
        let lat = deg2rad(lat_deg);
        let lon = deg2rad(lon_deg);
        let (sin_lat, cos_lat) = (lat.sin(), lat.cos());
        let (sin_lon, cos_lon) = (lon.sin(), lon.cos());

        // Rotation from ECEF to ENU
        let r = Matrix3::from_rows(&[
            // East
            nalgebra::RowVector3::new(-sin_lon, cos_lon, 0.0),
            // North
            nalgebra::RowVector3::new(-sin_lat * cos_lon, -sin_lat * sin_lon, cos_lat),
            // Up
            nalgebra::RowVector3::new(cos_lat * cos_lon, cos_lat * sin_lon, sin_lat),
        ]);

        let r_t = r.transpose(); // ENU -> ECEF

        Self { lat_deg, lon_deg, h_m, origin_ecef, ecef_to_enu: r, enu_to_ecef: r_t }
    }

    pub fn ecef_to_enu_vec(&self, p_ecef: Vector3<f64>) -> Vector3<f64> {
        let d = p_ecef - self.origin_ecef;
        self.ecef_to_enu * d
    }

    pub fn enu_to_ecef_vec(&self, p_enu: Vector3<f64>) -> Vector3<f64> {
        self.origin_ecef + self.enu_to_ecef * p_enu
    }
}

// Simple Bevy plugin that demonstrates usage at startup
pub struct GeodesyPlugin;

impl Plugin for GeodesyPlugin {
    fn build(&self, app: &mut App) {
        app
            // Resource defaults
            .init_resource::<GeoOrigin>()
            // Event to navigate/update origin
            .add_event::<NavigateToGeo>()
            // Systems
            .add_systems(Startup, log_camera_enu)
            .add_systems(Update, handle_navigate_to_geo);
    }
}

fn log_camera_enu(camera_q: Query<&Transform, With<Camera3d>>) {
    // Choose an arbitrary geodetic origin (can be made configurable later)
    let origin = LocalFrame::new(37.427_5, -122.169_7, 30.0); // Stanford-ish

    if let Ok(t) = camera_q.get_single() {
        // Interpret world units as meters in local ENU for demo purposes
        let enu = Vector3::new(t.translation.x as f64, t.translation.y as f64, t.translation.z as f64);
        let ecef = origin.enu_to_ecef_vec(enu);
        let enu_rt = origin.ecef_to_enu_vec(ecef);

        info!(
            "camera ENU ~ ({:.2}, {:.2}, {:.2}) m at origin ({:.4}, {:.4}, {:.1}) m | roundtrip err = {:.6} m",
            enu.x, enu.y, enu.z, origin.lat_deg, origin.lon_deg, origin.h_m, (enu_rt - enu).norm()
        );
    }
}

// Lightweight coordinate container
#[derive(Debug, Clone, Copy)]
pub struct GeoCoord {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub h_m: f64,
}

// Resource defining the current local tangent frame and scale
#[derive(Resource, Debug, Clone)]
pub struct GeoOrigin {
    pub frame: LocalFrame,
    pub meters_per_unit: f32, // World units scaling; 1.0 => 1 unit = 1 meter
}

impl Default for GeoOrigin {
    fn default() -> Self {
        // Sensible default: Palo Alto area
        let frame = LocalFrame::new(37.427_5, -122.169_7, 30.0);
        Self { frame, meters_per_unit: 1.0 }
    }
}

impl GeoOrigin {
    pub fn set_origin(&mut self, lat_deg: f64, lon_deg: f64, h_m: f64) {
        self.frame = LocalFrame::new(lat_deg, lon_deg, h_m);
    }

    // Convert geodetic to world Vec3 (ENU scaled to world units)
    pub fn geo_to_world(&self, coord: GeoCoord) -> Vec3 {
        let ecef = lla_to_ecef(coord.lat_deg, coord.lon_deg, coord.h_m);
        let enu = self.frame.ecef_to_enu_vec(ecef);
        Vec3::new(
            (enu.x as f32) / self.meters_per_unit,
            (enu.y as f32) / self.meters_per_unit,
            (enu.z as f32) / self.meters_per_unit,
        )
    }
}

// Event to recenter the simulator at a new geodetic origin
#[derive(Event, Debug, Clone, Copy)]
pub struct NavigateToGeo {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub h_m: f64,
}

fn handle_navigate_to_geo(
    mut geo_origin: ResMut<GeoOrigin>,
    mut evr: EventReader<NavigateToGeo>,
    mut camera_q: Query<&mut Transform, With<Camera3d>>,
) {
    for ev in evr.read() {
        geo_origin.set_origin(ev.lat_deg, ev.lon_deg, ev.h_m);
        // Recenter camera to origin, keep some altitude if already present
        if let Ok(mut t) = camera_q.get_single_mut() {
            let desired_alt = t.translation.y.max(50.0);
            t.translation = Vec3::new(0.0, desired_alt, 0.0);
            t.look_at(Vec3::ZERO, Vec3::Y);
        }
        info!(
            "Recentered GeoOrigin to lat={:.5}, lon={:.5}, h={:.1} m",
            ev.lat_deg, ev.lon_deg, ev.h_m
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enu_ecef_roundtrip_small_error() {
        let frame = LocalFrame::new(48.858_4, 2.294_5, 50.0); // Eiffel Tower area
        let enu = Vector3::new(123.4, -56.7, 890.1);
        let ecef = frame.enu_to_ecef_vec(enu);
        let enu_rt = frame.ecef_to_enu_vec(ecef);
        let err = (enu_rt - enu).norm();
        assert!(err < 1e-6, "roundtrip error too large: {}", err);
    }

    #[test]
    fn lla_to_ecef_basic_sanity() {
        // At equator (lat=0), lon=0, h=0 => x ~ a, y ~ 0, z ~ 0
        let p = lla_to_ecef(0.0, 0.0, 0.0);
        assert!((p.x - WGS84_A).abs() < 1e-3);
        assert!(p.y.abs() < 1e-6);
        assert!(p.z.abs() < 1e-6);
    }
}
