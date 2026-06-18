use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use uuid::Uuid;
use actix_web::{HttpResponse, ResponseError};
use actix_web::http::StatusCode;

// ================ 业务错误 ================
#[derive(Error, Debug)]
pub enum HunyiError {
    #[error("Validation error on field {field}: {message}")]
    Validation { field: String, message: String },
    #[error("ClickHouse error: {0}")]
    ClickHouse(String),
    #[error("Channel send error: {0}")]
    Channel(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Config error: {0}")]
    Config(String),
}

impl ResponseError for HunyiError {
    fn status_code(&self) -> StatusCode {
        match self {
            HunyiError::Validation { .. } => StatusCode::BAD_REQUEST,
            HunyiError::Config(_) | HunyiError::Json(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(ApiResponse::<()>::error(&self.to_string()))
    }
}

// ================ 基础实体 ================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
    pub device_id: String,
    pub axis_azimuth_angle: f64,
    pub axis_elevation_angle: f64,
    pub axis_equatorial_angle: f64,
    pub gear_meshing_error_1: f64,
    pub gear_meshing_error_2: f64,
    pub gear_meshing_error_3: f64,
    pub bearing_clearance_1: f64,
    pub bearing_clearance_2: f64,
    pub bearing_clearance_3: f64,
    pub observed_star_ra: f64,
    pub observed_star_dec: f64,
    pub theoretical_ra: f64,
    pub theoretical_dec: f64,
    pub ra_deviation: f64,
    pub dec_deviation: f64,
    pub cumulative_transmission_error: f64,
    pub gear_wear_level_1: f64,
    pub gear_wear_level_2: f64,
    pub gear_wear_level_3: f64,
    pub temperature: f64,
    pub humidity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransmissionErrorResult {
    pub timestamp: DateTime<Utc>,
    pub device_id: String,
    pub axis_id: u8,
    pub input_angle: f64,
    pub output_angle: f64,
    pub theoretical_ratio: f64,
    pub actual_ratio: f64,
    pub single_stage_error: f64,
    pub accumulated_error: f64,
    pub backlash_error: f64,
    pub elastic_deformation_error: f64,
    pub wear_induced_error: f64,
    pub temperature_effect: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointingAccuracyResult {
    pub timestamp: DateTime<Utc>,
    pub device_id: String,
    pub target_ra: f64,
    pub target_dec: f64,
    pub sky_zone: String,
    pub measured_ra: f64,
    pub measured_dec: f64,
    pub ra_error: f64,
    pub dec_error: f64,
    pub total_pointing_error: f64,
    pub error_azimuth_component: f64,
    pub error_elevation_component: f64,
    pub theoretical_precision: f64,
    pub achieved_precision: f64,
    pub error_transfer_coefficient: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmEvent {
    pub timestamp: DateTime<Utc>,
    pub device_id: String,
    #[serde(default = "Uuid::new_v4")]
    pub alarm_id: Uuid,
    pub alarm_type: String,
    pub alarm_level: u8,
    pub alarm_message: String,
    pub affected_axis: Option<u8>,
    pub error_value: f64,
    pub threshold_value: f64,
    #[serde(default)]
    pub is_acknowledged: u8,
    pub acknowledged_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GearStatus {
    pub timestamp: DateTime<Utc>,
    pub device_id: String,
    pub gear_id: u8,
    pub wear_level: f64,
    pub tooth_deflection: f64,
    pub lubrication_status: u8,
    pub vibration_amplitude: f64,
    pub rotation_speed: f64,
    pub torque: f64,
    pub estimated_life_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GearStage {
    pub stage_id: u8,
    pub teeth_input: u32,
    pub teeth_output: u32,
    pub theoretical_ratio: f64,
    pub backlash: f64,
    pub base_meshing_error: f64,
    pub wear_factor: f64,
    pub elastic_stiffness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisConfig {
    pub axis_id: u8,
    pub axis_name: String,
    pub gear_stages: Vec<GearStage>,
    pub bearing_clearance: f64,
    pub thermal_expansion_coeff: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub message_type: String,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        ApiResponse { success: true, message: "OK".to_string(), data: Some(data) }
    }
    pub fn success(data: T) -> Self { Self::ok(data) }
    pub fn error(message: &str) -> Self {
        ApiResponse { success: false, message: message.to_string(), data: None }
    }
}

// ================ 配置文件加载 ================
#[derive(Debug, Clone, Deserialize)]
pub struct GearMaterialParams {
    pub hertz_k_n_m15: f64,
    pub restitution_coeff: f64,
    pub damping_ratio: f64,
    pub tooth_equiv_mass_kg: f64,
    pub contact_iterations: usize,
    pub contact_dt_s: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShaftParams {
    pub young_modulus_pa: f64,
    pub shear_modulus_pa: f64,
    pub density_kgm3: f64,
    pub diameter_m: f64,
    pub length_m: f64,
    pub modal_damping_ratio: f64,
    pub operating_speed_rpm: f64,
    pub end_mass_moment_kgm2: f64,
    pub tip_equiv_mass_kg: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValidationRanges {
    pub angle_range_deg: (f64, f64),
    pub elevation_range_deg: (f64, f64),
    pub temperature_range_c: (f64, f64),
    pub humidity_range_pct: (f64, f64),
    pub max_wear_level: f64,
    pub max_error_arcmin: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GearParamsConfig {
    pub shaft: ShaftParams,
    pub gear_material: GearMaterialParams,
    pub axes: Vec<AxisConfig>,
    pub validation: ValidationRanges,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CumulativeErrorThreshold {
    pub warning_threshold_arcmin: f64,
    pub alarm_threshold_arcmin: f64,
    pub debounce_seconds: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GearWearThreshold {
    pub warning_threshold: f64,
    pub alarm_threshold: f64,
    pub debounce_seconds: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlarmConfig {
    pub cumulative_error: CumulativeErrorThreshold,
    pub gear_wear: GearWearThreshold,
}

impl GearParamsConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, HunyiError> {
        let p = path.as_ref();
        let content = std::fs::read_to_string(p)?;
        let cfg: GearParamsConfig = serde_json::from_str(&content)
            .map_err(|e| HunyiError::Config(format!("{} parse error: {}", p.display(), e)))?;
        Ok(cfg)
    }
}

impl AlarmConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, HunyiError> {
        let p = path.as_ref();
        let content = std::fs::read_to_string(p)?;
        let cfg: AlarmConfig = serde_json::from_str(&content)
            .map_err(|e| HunyiError::Config(format!("{} parse error: {}", p.display(), e)))?;
        Ok(cfg)
    }
}

// ================ 消息总线 ================
#[derive(Debug, Clone)]
pub enum PipelineMessage {
    ValidatedReading(Arc<SensorReading>),
    TransmissionResult(Arc<TransmissionErrorResult>),
    PointingResult(Arc<PointingAccuracyResult>),
    AlarmTriggered(Arc<AlarmEvent>),
    AllComputed(Arc<SensorReading>, Vec<Arc<TransmissionErrorResult>>, Arc<PointingAccuracyResult>),
}

// ================ Channel 集合 ================
pub struct PipelineChannels {
    pub to_transmission: mpsc::Sender<PipelineMessage>,
    pub to_pointing: mpsc::Sender<PipelineMessage>,
    pub to_alarm_ws: mpsc::Sender<PipelineMessage>,
}

impl PipelineChannels {
    pub fn new(capacity: usize) -> (Self,
        mpsc::Receiver<PipelineMessage>,   // transmission rx
        mpsc::Receiver<PipelineMessage>,   // pointing rx
        mpsc::Receiver<PipelineMessage>)   // alarm_ws rx
    {
        let (tx_t, rx_t) = mpsc::channel(capacity);
        let (tx_p, rx_p) = mpsc::channel(capacity);
        let (tx_a, rx_a) = mpsc::channel(capacity);
        (PipelineChannels {
            to_transmission: tx_t,
            to_pointing: tx_p,
            to_alarm_ws: tx_a,
        }, rx_t, rx_p, rx_a)
    }
}

impl fmt::Debug for PipelineChannels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PipelineChannels {{ mpsc senders }}")
    }
}

// ================ 校验 ================
pub fn validate_reading(r: &SensorReading, v: &ValidationRanges) -> Result<(), HunyiError> {
    fn check_range(val: f64, (lo, hi): (f64, f64), field: &str) -> Result<(), HunyiError> {
        if val.is_nan() || val.is_infinite() {
            return Err(HunyiError::Validation {
                field: field.to_string(),
                message: format!("值非有限数: {}", val),
            });
        }
        if val < lo || val > hi {
            return Err(HunyiError::Validation {
                field: field.to_string(),
                message: format!("超出范围 [{}, {}]: {}", lo, hi, val),
            });
        }
        Ok(())
    }

    check_range(r.axis_azimuth_angle, v.angle_range_deg, "axis_azimuth_angle")?;
    check_range(r.axis_elevation_angle, v.elevation_range_deg, "axis_elevation_angle")?;
    check_range(r.axis_equatorial_angle, v.angle_range_deg, "axis_equatorial_angle")?;
    check_range(r.temperature, v.temperature_range_c, "temperature")?;
    check_range(r.humidity, v.humidity_range_pct, "humidity")?;
    for (i, w) in [r.gear_wear_level_1, r.gear_wear_level_2, r.gear_wear_level_3].iter().enumerate() {
        check_range(*w, (0.0, v.max_wear_level), &format!("gear_wear_level_{}", i + 1))?;
    }
    for (i, e) in [r.gear_meshing_error_1, r.gear_meshing_error_2, r.gear_meshing_error_3].iter().enumerate() {
        check_range(*e, (0.0, v.max_error_arcmin), &format!("gear_meshing_error_{}", i + 1))?;
    }
    Ok(())
}
