use once_cell::sync::Lazy;
use prometheus::{
    register_histogram_vec, register_int_counter_vec, register_int_gauge,
    HistogramVec, IntCounterVec, IntGauge,
};

pub static HTTP_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "hunyi_http_requests_total",
        "Total number of HTTP requests",
        &["method", "path", "status"]
    )
    .unwrap()
});

pub static HTTP_REQUEST_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "hunyi_http_request_duration_seconds",
        "HTTP request duration in seconds",
        &["method", "path"],
        vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    )
    .unwrap()
});

pub static SENSOR_READINGS_RECEIVED: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "hunyi_sensor_readings_received_total",
        "Total sensor readings received by transport",
        &["transport"]
    )
    .unwrap()
});

pub static SENSOR_READINGS_VALID: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "hunyi_sensor_readings_valid_total",
        "Total valid sensor readings after validation",
        &["device_id"]
    )
    .unwrap()
});

pub static SENSOR_READINGS_INVALID: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "hunyi_sensor_readings_invalid_total",
        "Total invalid sensor readings",
        &["device_id", "reason"]
    )
    .unwrap()
});

pub static TRANSMISSION_JOBS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "hunyi_transmission_jobs_total",
        "Total transmission simulation jobs",
        &["axis_id"]
    )
    .unwrap()
});

pub static POINTING_JOBS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "hunyi_pointing_jobs_total",
        "Total pointing accuracy analysis jobs",
        &["sky_zone"]
    )
    .unwrap()
});

pub static ALARMS_TRIGGERED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "hunyi_alarms_triggered_total",
        "Total alarms triggered",
        &["alarm_type", "alarm_level"]
    )
    .unwrap()
});

pub static CURRENT_CUMULATIVE_ERROR: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "hunyi_current_cumulative_error_arcmin_milli",
        "Current cumulative transmission error in milliarcmins (×1000)"
    )
    .unwrap()
});

pub static CURRENT_GEAR_WEAR: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "hunyi_current_gear_wear_permille",
        "Current gear wear level in permille (×1000)",
    )
    .unwrap()
});

pub static MQTT_MESSAGES_RECEIVED: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "hunyi_mqtt_messages_received_total",
        "Total MQTT messages received",
        &["topic", "status"]
    )
    .unwrap()
});

pub static CHANNEL_QUEUE_SIZE: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "hunyi_channel_queue_size",
        "Current mpsc channel queue size",
    )
    .unwrap()
});

pub fn gather() -> Vec<prometheus::proto::MetricFamily> {
    prometheus::gather()
}

pub fn encode_metrics() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    let metric_families = prometheus::gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
