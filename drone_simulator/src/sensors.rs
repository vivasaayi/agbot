use anyhow::Result;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use nalgebra::{Vector3, Point3};
use rand::Rng;
use crate::{DroneCapabilities, environment::Environment, physics::PhysicsState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    pub sensor_type: String,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub quality: f32, // 0.0 to 1.0
}

pub struct SensorSuite {
    pub gps: GpsSensor,
    pub imu: ImuSensor,
    pub barometer: BarometerSensor,
    pub magnetometer: MagnetometerSensor,
    pub camera: Option<CameraSensor>,
    pub lidar: Option<LidarSensor>,
    pub multispectral: Option<MultispectralSensor>,
}

pub struct GpsSensor {
    pub accuracy_m: f32,
    pub update_rate_hz: f32,
    last_update: DateTime<Utc>,
}

pub struct ImuSensor {
    pub accel_noise: f32,
    pub gyro_noise: f32,
    pub update_rate_hz: f32,
    last_update: DateTime<Utc>,
}

pub struct BarometerSensor {
    pub pressure_noise: f32,
    pub altitude_accuracy_m: f32,
    pub update_rate_hz: f32,
    last_update: DateTime<Utc>,
}

pub struct MagnetometerSensor {
    pub heading_accuracy_deg: f32,
    pub update_rate_hz: f32,
    last_update: DateTime<Utc>,
}

pub struct CameraSensor {
    pub resolution: (u32, u32),
    pub fov_degrees: f32,
    pub fps: f32,
    pub quality: f32,
    last_capture: DateTime<Utc>,
}

pub struct LidarSensor {
    pub range_m: f32,
    pub accuracy_cm: f32,
    pub scan_rate_hz: f32,
    pub points_per_scan: u32,
    last_scan: DateTime<Utc>,
}

pub struct MultispectralSensor {
    pub bands: Vec<String>,
    pub resolution: (u32, u32),
    pub capture_rate_hz: f32,
    last_capture: DateTime<Utc>,
}

impl SensorSuite {
    pub fn new(capabilities: &DroneCapabilities) -> Self {
        Self {
            gps: GpsSensor::new(),
            imu: ImuSensor::new(),
            barometer: BarometerSensor::new(),
            magnetometer: MagnetometerSensor::new(),
            camera: if capabilities.has_camera { Some(CameraSensor::new()) } else { None },
            lidar: if capabilities.has_lidar { Some(LidarSensor::new()) } else { None },
            multispectral: if capabilities.has_multispectral { Some(MultispectralSensor::new()) } else { None },
        }
    }

    pub fn update(&mut self, state: &PhysicsState, environment: &Environment) -> Result<Vec<SensorReading>> {
        let mut readings = Vec::new();
        let now = Utc::now();

        // GPS readings
        if Self::should_update(&self.gps.last_update, self.gps.update_rate_hz, now) {
            if let Some(reading) = self.gps.read(state, environment)? {
                readings.push(reading);
            }
            self.gps.last_update = now;
        }

        // IMU readings
        if Self::should_update(&self.imu.last_update, self.imu.update_rate_hz, now) {
            if let Some(reading) = self.imu.read(state, environment)? {
                readings.push(reading);
            }
            self.imu.last_update = now;
        }

        // Barometer readings
        if Self::should_update(&self.barometer.last_update, self.barometer.update_rate_hz, now) {
            if let Some(reading) = self.barometer.read(state, environment)? {
                readings.push(reading);
            }
            self.barometer.last_update = now;
        }

        // Magnetometer readings
        if Self::should_update(&self.magnetometer.last_update, self.magnetometer.update_rate_hz, now) {
            if let Some(reading) = self.magnetometer.read(state, environment)? {
                readings.push(reading);
            }
            self.magnetometer.last_update = now;
        }

        // Camera readings
        if let Some(ref mut camera) = self.camera {
            if Self::should_update(&camera.last_capture, camera.fps, now) {
                if let Some(reading) = camera.capture(state, environment)? {
                    readings.push(reading);
                }
                camera.last_capture = now;
            }
        }

        // LiDAR readings
        if let Some(ref mut lidar) = self.lidar {
            if Self::should_update(&lidar.last_scan, lidar.scan_rate_hz, now) {
                if let Some(reading) = lidar.scan(state, environment)? {
                    readings.push(reading);
                }
                lidar.last_scan = now;
            }
        }

        // Multispectral readings
        if let Some(ref mut multispectral) = self.multispectral {
            if Self::should_update(&multispectral.last_capture, multispectral.capture_rate_hz, now) {
                if let Some(reading) = multispectral.capture(state, environment)? {
                    readings.push(reading);
                }
                multispectral.last_capture = now;
            }
        }

        Ok(readings)
    }

    fn should_update(last_update: &DateTime<Utc>, rate_hz: f32, now: DateTime<Utc>) -> bool {
        let interval_ms = (1000.0 / rate_hz) as i64;
        (now - *last_update).num_milliseconds() >= interval_ms
    }
}

impl GpsSensor {
    pub fn new() -> Self {
        Self {
            accuracy_m: 2.0,
            update_rate_hz: 10.0,
            last_update: Utc::now() - chrono::Duration::seconds(1),
        }
    }

    pub fn read(&self, state: &PhysicsState, _environment: &Environment) -> Result<Option<SensorReading>> {
        let mut rng = rand::thread_rng();
        
        // Add GPS noise
        let noise_x = rng.gen_range(-self.accuracy_m..self.accuracy_m);
        let noise_y = rng.gen_range(-self.accuracy_m..self.accuracy_m);
        let noise_z = rng.gen_range(-self.accuracy_m..self.accuracy_m);

        let gps_position = Point3::new(
            state.position.x + noise_x,
            state.position.y + noise_y,
            state.position.z + noise_z,
        );

        let data = serde_json::json!({
            "latitude": gps_position.x,  // In real implementation, convert to lat/lon
            "longitude": gps_position.z,
            "altitude": gps_position.y,
            "hdop": rng.gen_range(0.8..2.0),
            "satellites": rng.gen_range(8..15),
            "fix_type": "3D"
        });

        Ok(Some(SensorReading {
            sensor_type: "GPS".to_string(),
            data,
            timestamp: Utc::now(),
            quality: 1.0 - (self.accuracy_m / 10.0).min(1.0),
        }))
    }
}

impl ImuSensor {
    pub fn new() -> Self {
        Self {
            accel_noise: 0.1,
            gyro_noise: 0.05,
            update_rate_hz: 100.0,
            last_update: Utc::now() - chrono::Duration::milliseconds(10),
        }
    }

    pub fn read(&self, state: &PhysicsState, _environment: &Environment) -> Result<Option<SensorReading>> {
        let mut rng = rand::thread_rng();

        // Add noise to acceleration and angular velocity
        let accel = Vector3::new(
            state.acceleration.x + rng.gen_range(-self.accel_noise..self.accel_noise),
            state.acceleration.y + rng.gen_range(-self.accel_noise..self.accel_noise),
            state.acceleration.z + rng.gen_range(-self.accel_noise..self.accel_noise),
        );

        let gyro = Vector3::new(
            state.angular_velocity.x + rng.gen_range(-self.gyro_noise..self.gyro_noise),
            state.angular_velocity.y + rng.gen_range(-self.gyro_noise..self.gyro_noise),
            state.angular_velocity.z + rng.gen_range(-self.gyro_noise..self.gyro_noise),
        );

        let data = serde_json::json!({
            "acceleration": {
                "x": accel.x,
                "y": accel.y,
                "z": accel.z
            },
            "angular_velocity": {
                "x": gyro.x,
                "y": gyro.y,
                "z": gyro.z
            },
            "orientation": {
                "roll": state.orientation.x,
                "pitch": state.orientation.y,
                "yaw": state.orientation.z
            }
        });

        Ok(Some(SensorReading {
            sensor_type: "IMU".to_string(),
            data,
            timestamp: Utc::now(),
            quality: 0.95,
        }))
    }
}

impl BarometerSensor {
    pub fn new() -> Self {
        Self {
            pressure_noise: 1.0,
            altitude_accuracy_m: 1.0,
            update_rate_hz: 20.0,
            last_update: Utc::now() - chrono::Duration::milliseconds(50),
        }
    }

    pub fn read(&self, state: &PhysicsState, environment: &Environment) -> Result<Option<SensorReading>> {
        let mut rng = rand::thread_rng();
        let conditions = environment.get_conditions();

        let altitude_noise = rng.gen_range(-self.altitude_accuracy_m..self.altitude_accuracy_m);
        let pressure_noise = rng.gen_range(-self.pressure_noise..self.pressure_noise);

        let measured_altitude = state.position.y + altitude_noise;
        let measured_pressure = conditions.pressure_hpa + pressure_noise;

        let data = serde_json::json!({
            "pressure_hpa": measured_pressure,
            "altitude_m": measured_altitude,
            "temperature_c": conditions.temperature_celsius
        });

        Ok(Some(SensorReading {
            sensor_type: "Barometer".to_string(),
            data,
            timestamp: Utc::now(),
            quality: 0.9,
        }))
    }
}

impl MagnetometerSensor {
    pub fn new() -> Self {
        Self {
            heading_accuracy_deg: 2.0,
            update_rate_hz: 75.0,
            last_update: Utc::now() - chrono::Duration::milliseconds(13),
        }
    }

    pub fn read(&self, state: &PhysicsState, _environment: &Environment) -> Result<Option<SensorReading>> {
        let mut rng = rand::thread_rng();

        let heading_noise = rng.gen_range(-self.heading_accuracy_deg..self.heading_accuracy_deg);
        let magnetic_heading = state.orientation.z.to_degrees() + heading_noise;

        let data = serde_json::json!({
            "heading_deg": magnetic_heading,
            "magnetic_field": {
                "x": rng.gen_range(-50.0..50.0),
                "y": rng.gen_range(-50.0..50.0),
                "z": rng.gen_range(-50.0..50.0)
            }
        });

        Ok(Some(SensorReading {
            sensor_type: "Magnetometer".to_string(),
            data,
            timestamp: Utc::now(),
            quality: 0.85,
        }))
    }
}

impl CameraSensor {
    pub fn new() -> Self {
        Self {
            resolution: (1920, 1080),
            fov_degrees: 90.0,
            fps: 30.0,
            quality: 0.8,
            last_capture: Utc::now() - chrono::Duration::milliseconds(33),
        }
    }

    pub fn capture(&self, state: &PhysicsState, environment: &Environment) -> Result<Option<SensorReading>> {
        let conditions = environment.get_conditions();
        
        // Simulate image capture metadata
        let data = serde_json::json!({
            "image_id": uuid::Uuid::new_v4(),
            "resolution": {
                "width": self.resolution.0,
                "height": self.resolution.1
            },
            "position": {
                "x": state.position.x,
                "y": state.position.y,
                "z": state.position.z
            },
            "orientation": {
                "roll": state.orientation.x.to_degrees(),
                "pitch": state.orientation.y.to_degrees(),
                "yaw": state.orientation.z.to_degrees()
            },
            "camera_settings": {
                "iso": 100,
                "shutter_speed": "1/60",
                "aperture": "f/2.8"
            },
            "lighting_conditions": {
                "brightness": conditions.visibility_m / 10000.0,
                "cloud_cover": conditions.cloud_cover_percent
            }
        });

        Ok(Some(SensorReading {
            sensor_type: "Camera".to_string(),
            data,
            timestamp: Utc::now(),
            quality: self.quality * (conditions.visibility_m / 10000.0).min(1.0),
        }))
    }
}

impl LidarSensor {
    pub fn new() -> Self {
        Self {
            range_m: 100.0,
            accuracy_cm: 2.0,
            scan_rate_hz: 10.0,
            points_per_scan: 1000,
            last_scan: Utc::now() - chrono::Duration::milliseconds(100),
        }
    }

    pub fn scan(&self, state: &PhysicsState, _environment: &Environment) -> Result<Option<SensorReading>> {
        let mut rng = rand::thread_rng();
        
        // Generate simulated point cloud
        let mut points = Vec::new();
        for _ in 0..self.points_per_scan {
            let distance = rng.gen_range(1.0..self.range_m);
            let angle = rng.gen_range(0.0..2.0 * std::f32::consts::PI);
            let elevation = rng.gen_range(-0.5..0.5);
            
            points.push(serde_json::json!({
                "x": distance * angle.cos(),
                "y": distance * elevation,
                "z": distance * angle.sin(),
                "intensity": rng.gen_range(0..255)
            }));
        }

        let data = serde_json::json!({
            "scan_id": uuid::Uuid::new_v4(),
            "drone_position": {
                "x": state.position.x,
                "y": state.position.y,
                "z": state.position.z
            },
            "points": points,
            "point_count": self.points_per_scan,
            "range_m": self.range_m
        });

        Ok(Some(SensorReading {
            sensor_type: "LiDAR".to_string(),
            data,
            timestamp: Utc::now(),
            quality: 0.95,
        }))
    }
}

impl MultispectralSensor {
    pub fn new() -> Self {
        Self {
            bands: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string(), "NIR".to_string()],
            resolution: (640, 480),
            capture_rate_hz: 5.0,
            last_capture: Utc::now() - chrono::Duration::milliseconds(200),
        }
    }

    pub fn capture(&self, state: &PhysicsState, environment: &Environment) -> Result<Option<SensorReading>> {
        let mut rng = rand::thread_rng();
        let conditions = environment.get_conditions();
        
        // Simulate multispectral data for each band
        let mut band_data = Vec::new();
        for band in &self.bands {
            band_data.push(serde_json::json!({
                "band": band,
                "mean_reflectance": rng.gen_range(0.0..1.0),
                "std_reflectance": rng.gen_range(0.0..0.2),
                "pixel_count": self.resolution.0 * self.resolution.1
            }));
        }

        let data = serde_json::json!({
            "capture_id": uuid::Uuid::new_v4(),
            "position": {
                "x": state.position.x,
                "y": state.position.y,
                "z": state.position.z
            },
            "bands": band_data,
            "resolution": {
                "width": self.resolution.0,
                "height": self.resolution.1
            },
            "lighting_conditions": {
                "solar_angle": 45.0, // Simplified
                "atmospheric_transmission": conditions.visibility_m / 15000.0
            }
        });

        Ok(Some(SensorReading {
            sensor_type: "Multispectral".to_string(),
            data,
            timestamp: Utc::now(),
            quality: 0.9 * (conditions.visibility_m / 15000.0).min(1.0),
        }))
    }
}
