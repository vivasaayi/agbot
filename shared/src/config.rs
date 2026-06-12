use crate::{error::AgroError, AgroResult, RuntimeMode};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgroConfig {
    pub runtime_mode: RuntimeMode,
    pub mavlink: MavlinkConfig,
    pub lidar: LidarConfig,
    pub camera: CameraConfig,
    pub storage: StorageConfig,
    pub server: ServerConfig,
    pub gps: GpsConfig,
    pub processing: ProcessingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MavlinkConfig {
    pub serial_port: String,
    pub baud_rate: u32,
    pub timeout_ms: u64,
    pub heartbeat_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarConfig {
    pub serial_port: String,
    pub baud_rate: u32,
    pub timeout_ms: u64,
    pub scan_frequency: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub device: String,
    pub multispectral_bands: u8,
    pub capture_interval_ms: u64,
    pub exposure_time: f32,
    pub gain: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_root_path: PathBuf,
    pub mission_data_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub ws_bind_address: String,
    pub api_bind_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsConfig {
    pub home_latitude: f64,
    pub home_longitude: f64,
    pub home_altitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    pub ndvi_output_format: String,
    pub lidar_grid_resolution: f32,
    // LiDAR processing thresholds
    pub lidar_obstacle_distance_threshold: f32,
    pub lidar_quality_threshold: u8,
    pub lidar_occupancy_threshold: f32,
    // Flip Y axis when saving images (north-up convention)
    pub lidar_image_flip_y: bool,
}

impl AgroConfig {
    pub fn load() -> AgroResult<Self> {
        dotenvy::dotenv().ok();

        let runtime_mode = env_parse("RUNTIME_MODE", RuntimeMode::Simulation)?;

        let config = AgroConfig {
            runtime_mode,
            mavlink: MavlinkConfig {
                serial_port: env_string("MAVLINK_SERIAL_PORT", "/dev/ttyUSB0", runtime_mode, true)?,
                baud_rate: env_parse("MAVLINK_BAUD_RATE", 57600u32)?,
                timeout_ms: env_parse("MAVLINK_TIMEOUT_MS", 1000u64)?,
                heartbeat_interval_ms: env_parse("MAVLINK_HEARTBEAT_INTERVAL_MS", 1000u64)?,
            },
            lidar: LidarConfig {
                serial_port: env_string("LIDAR_SERIAL_PORT", "/dev/ttyUSB1", runtime_mode, true)?,
                baud_rate: env_parse("LIDAR_BAUD_RATE", 230400u32)?,
                timeout_ms: env_parse("LIDAR_TIMEOUT_MS", 1000u64)?,
                scan_frequency: env_parse("LIDAR_SCAN_FREQUENCY", 10.0f32)?,
            },
            camera: CameraConfig {
                device: env_string("CAMERA_DEVICE", "/dev/video0", runtime_mode, true)?,
                multispectral_bands: env_parse("MULTISPECTRAL_BANDS", 4u8)?,
                capture_interval_ms: env_parse("CAMERA_CAPTURE_INTERVAL_MS", 5000u64)?,
                exposure_time: env_parse("CAMERA_EXPOSURE_TIME", 1.0f32 / 60.0f32)?,
                gain: env_parse("CAMERA_GAIN", 1.0f32)?,
            },
            storage: StorageConfig {
                data_root_path: env_string(
                    "DATA_ROOT_PATH",
                    "/tmp/agrodrone/data",
                    runtime_mode,
                    true,
                )?
                .into(),
                mission_data_path: env_string(
                    "MISSION_DATA_PATH",
                    "/tmp/agrodrone/missions",
                    runtime_mode,
                    true,
                )?
                .into(),
            },
            server: ServerConfig {
                ws_bind_address: env_string("WS_BIND_ADDRESS", "0.0.0.0:8080", runtime_mode, true)?,
                api_bind_address: env_string(
                    "API_BIND_ADDRESS",
                    "0.0.0.0:3000",
                    runtime_mode,
                    true,
                )?,
            },
            gps: GpsConfig {
                home_latitude: env_parse("HOME_LATITUDE", 37.7749f64)?,
                home_longitude: env_parse("HOME_LONGITUDE", -122.4194f64)?,
                home_altitude: env_parse("HOME_ALTITUDE", 100.0f64)?,
            },
            processing: ProcessingConfig {
                ndvi_output_format: env_string(
                    "NDVI_OUTPUT_FORMAT",
                    "GEOTIFF",
                    runtime_mode,
                    false,
                )?,
                lidar_grid_resolution: env_parse("LIDAR_GRID_RESOLUTION", 0.1f32)?,
                lidar_obstacle_distance_threshold: env_parse(
                    "LIDAR_OBSTACLE_DISTANCE_THRESHOLD",
                    5.0f32,
                )?,
                lidar_quality_threshold: env_parse("LIDAR_QUALITY_THRESHOLD", 20u8)?,
                lidar_occupancy_threshold: env_parse("LIDAR_OCCUPANCY_THRESHOLD", 0.5f32)?,
                lidar_image_flip_y: env_parse("LIDAR_IMAGE_FLIP_Y", false)?,
            },
        };

        config.validate()?;
        tracing::info!(runtime_mode = ?config.runtime_mode, "agro config loaded");
        Ok(config)
    }

    pub fn validate(&self) -> AgroResult<()> {
        require_non_empty_path("DATA_ROOT_PATH", &self.storage.data_root_path)?;
        require_non_empty_path("MISSION_DATA_PATH", &self.storage.mission_data_path)?;
        require_bind_address("WS_BIND_ADDRESS", &self.server.ws_bind_address)?;
        require_bind_address("API_BIND_ADDRESS", &self.server.api_bind_address)?;

        require_range("MAVLINK_BAUD_RATE", self.mavlink.baud_rate, 1u32, u32::MAX)?;
        require_range(
            "MAVLINK_TIMEOUT_MS",
            self.mavlink.timeout_ms,
            1u64,
            u64::MAX,
        )?;
        require_range(
            "MAVLINK_HEARTBEAT_INTERVAL_MS",
            self.mavlink.heartbeat_interval_ms,
            1u64,
            u64::MAX,
        )?;
        require_range("LIDAR_BAUD_RATE", self.lidar.baud_rate, 1u32, u32::MAX)?;
        require_range("LIDAR_TIMEOUT_MS", self.lidar.timeout_ms, 1u64, u64::MAX)?;
        require_positive_f32("LIDAR_SCAN_FREQUENCY", self.lidar.scan_frequency)?;
        require_range(
            "MULTISPECTRAL_BANDS",
            self.camera.multispectral_bands,
            1u8,
            u8::MAX,
        )?;
        require_range(
            "CAMERA_CAPTURE_INTERVAL_MS",
            self.camera.capture_interval_ms,
            1u64,
            u64::MAX,
        )?;
        require_positive_f32("CAMERA_EXPOSURE_TIME", self.camera.exposure_time)?;
        require_positive_f32("CAMERA_GAIN", self.camera.gain)?;
        require_latitude("HOME_LATITUDE", self.gps.home_latitude)?;
        require_longitude("HOME_LONGITUDE", self.gps.home_longitude)?;
        require_finite_f64("HOME_ALTITUDE", self.gps.home_altitude)?;
        require_non_empty("NDVI_OUTPUT_FORMAT", &self.processing.ndvi_output_format)?;
        require_positive_f32(
            "LIDAR_GRID_RESOLUTION",
            self.processing.lidar_grid_resolution,
        )?;
        require_positive_f32(
            "LIDAR_OBSTACLE_DISTANCE_THRESHOLD",
            self.processing.lidar_obstacle_distance_threshold,
        )?;
        require_range(
            "LIDAR_QUALITY_THRESHOLD",
            self.processing.lidar_quality_threshold,
            1u8,
            100u8,
        )?;
        require_fraction(
            "LIDAR_OCCUPANCY_THRESHOLD",
            self.processing.lidar_occupancy_threshold,
        )?;

        Ok(())
    }
}

fn env_string(
    key: &str,
    fallback: &str,
    runtime_mode: RuntimeMode,
    required_in_flight: bool,
) -> AgroResult<String> {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) if runtime_mode == RuntimeMode::Flight && required_in_flight => {
            Err(missing_required_field(key))
        }
        Ok(_) => Ok(fallback.to_string()),
        Err(std::env::VarError::NotPresent)
            if runtime_mode == RuntimeMode::Flight && required_in_flight =>
        {
            Err(missing_required_field(key))
        }
        Err(std::env::VarError::NotPresent) => Ok(fallback.to_string()),
        Err(error) => Err(AgroError::ConfigValidation(format!(
            "invalid env var `{key}`: {error}"
        ))),
    }
}

fn env_parse<T>(key: &str, fallback: T) -> AgroResult<T>
where
    T: FromStr + Copy,
    T::Err: Display,
{
    match std::env::var(key) {
        Ok(value) if value.trim().is_empty() => Ok(fallback),
        Ok(value) => value.parse::<T>().map_err(|error| {
            AgroError::ConfigValidation(format!(
                "invalid config field `{key}` value `{value}`: {error}"
            ))
        }),
        Err(std::env::VarError::NotPresent) => Ok(fallback),
        Err(error) => Err(AgroError::ConfigValidation(format!(
            "invalid env var `{key}`: {error}"
        ))),
    }
}

fn missing_required_field(key: &str) -> AgroError {
    AgroError::ConfigValidation(format!("missing required flight config field `{key}`"))
}

fn require_non_empty(key: &str, value: &str) -> AgroResult<()> {
    if value.trim().is_empty() {
        return Err(AgroError::ConfigValidation(format!(
            "config field `{key}` cannot be empty"
        )));
    }
    Ok(())
}

fn require_non_empty_path(key: &str, value: &PathBuf) -> AgroResult<()> {
    if value.as_os_str().is_empty() {
        return Err(AgroError::ConfigValidation(format!(
            "config field `{key}` cannot be empty"
        )));
    }
    Ok(())
}

fn require_bind_address(key: &str, value: &str) -> AgroResult<()> {
    require_non_empty(key, value)?;
    if !value.contains(':') {
        return Err(AgroError::ConfigValidation(format!(
            "config field `{key}` must include host:port"
        )));
    }
    Ok(())
}

fn require_range<T>(key: &str, value: T, min: T, max: T) -> AgroResult<()>
where
    T: PartialOrd + Display,
{
    if value < min || value > max {
        return Err(AgroError::ConfigValidation(format!(
            "config field `{key}` must be between {min} and {max}, got {value}"
        )));
    }
    Ok(())
}

fn require_positive_f32(key: &str, value: f32) -> AgroResult<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(AgroError::ConfigValidation(format!(
            "config field `{key}` must be a positive finite number"
        )));
    }
    Ok(())
}

fn require_finite_f64(key: &str, value: f64) -> AgroResult<()> {
    if !value.is_finite() {
        return Err(AgroError::ConfigValidation(format!(
            "config field `{key}` must be finite"
        )));
    }
    Ok(())
}

fn require_latitude(key: &str, value: f64) -> AgroResult<()> {
    require_finite_f64(key, value)?;
    if !(-90.0..=90.0).contains(&value) {
        return Err(AgroError::ConfigValidation(format!(
            "config field `{key}` must be between -90 and 90"
        )));
    }
    Ok(())
}

fn require_longitude(key: &str, value: f64) -> AgroResult<()> {
    require_finite_f64(key, value)?;
    if !(-180.0..=180.0).contains(&value) {
        return Err(AgroError::ConfigValidation(format!(
            "config field `{key}` must be between -180 and 180"
        )));
    }
    Ok(())
}

fn require_fraction(key: &str, value: f32) -> AgroResult<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(AgroError::ConfigValidation(format!(
            "config field `{key}` must be between 0 and 1"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::AgroConfig;
    use crate::RuntimeMode;
    use std::sync::{Mutex, OnceLock};

    const CONFIG_ENV_KEYS: &[&str] = &[
        "RUNTIME_MODE",
        "MAVLINK_SERIAL_PORT",
        "MAVLINK_BAUD_RATE",
        "MAVLINK_TIMEOUT_MS",
        "MAVLINK_HEARTBEAT_INTERVAL_MS",
        "LIDAR_SERIAL_PORT",
        "LIDAR_BAUD_RATE",
        "LIDAR_TIMEOUT_MS",
        "LIDAR_SCAN_FREQUENCY",
        "CAMERA_DEVICE",
        "MULTISPECTRAL_BANDS",
        "CAMERA_CAPTURE_INTERVAL_MS",
        "CAMERA_EXPOSURE_TIME",
        "CAMERA_GAIN",
        "DATA_ROOT_PATH",
        "MISSION_DATA_PATH",
        "WS_BIND_ADDRESS",
        "API_BIND_ADDRESS",
        "HOME_LATITUDE",
        "HOME_LONGITUDE",
        "HOME_ALTITUDE",
        "NDVI_OUTPUT_FORMAT",
        "LIDAR_GRID_RESOLUTION",
        "LIDAR_OBSTACLE_DISTANCE_THRESHOLD",
        "LIDAR_QUALITY_THRESHOLD",
        "LIDAR_OCCUPANCY_THRESHOLD",
        "LIDAR_IMAGE_FLIP_Y",
    ];

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvRestore {
        values: Vec<(String, Option<String>)>,
    }

    impl EnvRestore {
        fn clear() -> Self {
            let values = CONFIG_ENV_KEYS
                .iter()
                .map(|key| {
                    let previous = std::env::var(key).ok();
                    std::env::remove_var(key);
                    ((*key).to_string(), previous)
                })
                .collect();
            Self { values }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }

    #[test]
    fn simulation_config_loads_defaults_and_validates() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let _restore = EnvRestore::clear();

        let config = AgroConfig::load().expect("simulation defaults should load");

        assert_eq!(config.runtime_mode, RuntimeMode::Simulation);
        assert_eq!(config.mavlink.serial_port, "/dev/ttyUSB0");
        assert_eq!(config.server.ws_bind_address, "0.0.0.0:8080");
    }

    #[test]
    fn flight_config_missing_hardware_field_fails_fast() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let _restore = EnvRestore::clear();
        std::env::set_var("RUNTIME_MODE", "FLIGHT");
        std::env::set_var("MAVLINK_SERIAL_PORT", "");

        let error = AgroConfig::load().expect_err("flight mode requires hardware fields");

        assert!(error.to_string().contains("MAVLINK_SERIAL_PORT"));
    }

    #[test]
    fn config_rejects_out_of_range_gps_values() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let _restore = EnvRestore::clear();
        std::env::set_var("HOME_LATITUDE", "120.0");

        let error = AgroConfig::load().expect_err("invalid latitude should fail validation");

        assert!(error.to_string().contains("HOME_LATITUDE"));
    }
}
