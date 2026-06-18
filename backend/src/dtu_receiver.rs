use crate::clickhouse::ClickHouseClient;
use crate::metrics::{CURRENT_CUMULATIVE_ERROR, CURRENT_GEAR_WEAR, SENSOR_READINGS_INVALID, SENSOR_READINGS_RECEIVED, SENSOR_READINGS_VALID};
use crate::models::{GearParamsConfig, HunyiError, PipelineChannels, PipelineMessage, SensorReading, validate_reading};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub struct DtuReceiver {
    ch_client: Arc<ClickHouseClient>,
    gear_cfg: Arc<GearParamsConfig>,
    channels: PipelineChannels,
}

impl DtuReceiver {
    pub fn new(
        ch_client: Arc<ClickHouseClient>,
        gear_cfg: Arc<GearParamsConfig>,
        channels: PipelineChannels,
    ) -> Self {
        DtuReceiver { ch_client, gear_cfg, channels }
    }

    pub fn compute_cumulative_error(&self, r: &SensorReading) -> f64 {
        let gear_errors = [
            r.gear_meshing_error_1,
            r.gear_meshing_error_2,
            r.gear_meshing_error_3,
        ];
        let bearing_errors = [
            r.bearing_clearance_1,
            r.bearing_clearance_2,
            r.bearing_clearance_3,
        ];
        let wear_levels = [
            r.gear_wear_level_1,
            r.gear_wear_level_2,
            r.gear_wear_level_3,
        ];

        let mut cumulative = 0.0;
        for i in 0..3 {
            cumulative += gear_errors[i] * (1.0 + wear_levels[i] * 2.0);
            cumulative += bearing_errors[i] * 0.5;
        }
        cumulative += (r.temperature - 20.0).abs() * 0.02;
        cumulative
    }

    pub async fn ingest(&self, mut reading: SensorReading) -> Result<Arc<SensorReading>, HunyiError> {
        SENSOR_READINGS_RECEIVED.with_label_values(&["http"]).inc();

        if let Err(e) = validate_reading(&reading, &self.gear_cfg.validation) {
            SENSOR_READINGS_INVALID
                .with_label_values(&[&reading.device_id, &e.to_string()])
                .inc();
            warn!(device_id = %reading.device_id, error = %e, "Invalid sensor reading");
            return Err(e);
        }

        reading.cumulative_transmission_error = self.compute_cumulative_error(&reading);

        self.ch_client
            .insert_sensor_reading(&reading)
            .await
            .map_err(|e| HunyiError::ClickHouse(e.to_string()))?;

        let arc = Arc::new(reading);

        SENSOR_READINGS_VALID
            .with_label_values(&[&arc.device_id])
            .inc();
        CURRENT_CUMULATIVE_ERROR.set((arc.cumulative_transmission_error * 1000.0) as i64);
        let avg_wear = (arc.gear_wear_level_1 + arc.gear_wear_level_2 + arc.gear_wear_level_3) / 3.0;
        CURRENT_GEAR_WEAR.set((avg_wear * 1000.0) as i64);

        if let Err(e) = self.channels.to_transmission.send(PipelineMessage::ValidatedReading(arc.clone())).await {
            error!(error = %e, "DTU->transmission channel error");
        }
        if let Err(e) = self.channels.to_pointing.send(PipelineMessage::ValidatedReading(arc.clone())).await {
            error!(error = %e, "DTU->pointing channel error");
        }
        if let Err(e) = self.channels.to_alarm_ws.send(PipelineMessage::ValidatedReading(arc.clone())).await {
            error!(error = %e, "DTU->alarm_ws channel error");
        }

        debug!(
            device_id = %arc.device_id,
            cum_err = arc.cumulative_transmission_error,
            "DTU ingested reading"
        );
        Ok(arc)
    }

    pub fn validation_rules(&self) -> &crate::models::ValidationRanges {
        &self.gear_cfg.validation
    }
}
