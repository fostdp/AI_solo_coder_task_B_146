use crate::clickhouse::ClickHouseClient;
use crate::metrics::TRANSMISSION_JOBS_TOTAL;
use crate::models::{
    AxisConfig, GearMaterialParams, GearParamsConfig, PipelineMessage,
    TransmissionErrorResult,
};
use chrono::Utc;
use rand::Rng;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn, debug};

const DEG_TO_ARCMIN: f64 = 60.0;
const ARCMIN_TO_RAD: f64 = std::f64::consts::PI / (180.0 * 60.0);

pub struct TransmissionSimulator {
    ch_client: Option<Arc<ClickHouseClient>>,
    cfg: Arc<GearParamsConfig>,
    rx: Option<mpsc::Receiver<PipelineMessage>>,
    alarm_tx: Option<mpsc::Sender<PipelineMessage>>,
    ws_tx: Option<mpsc::Sender<PipelineMessage>>,
}

impl TransmissionSimulator {
    pub fn new(
        ch_client: Arc<ClickHouseClient>,
        cfg: Arc<GearParamsConfig>,
        rx: mpsc::Receiver<PipelineMessage>,
        alarm_tx: mpsc::Sender<PipelineMessage>,
        ws_tx: mpsc::Sender<PipelineMessage>,
    ) -> Self {
        TransmissionSimulator { ch_client: Some(ch_client), cfg, rx: Some(rx), alarm_tx: Some(alarm_tx), ws_tx: Some(ws_tx) }
    }

    pub fn new_standalone(cfg: GearParamsConfig) -> Self {
        TransmissionSimulator { ch_client: None, cfg: Arc::new(cfg), rx: None, alarm_tx: None, ws_tx: None }
    }

    pub fn get_config(&self) -> &GearParamsConfig {
        &self.cfg
    }

    fn axes(&self) -> &[AxisConfig] {
        &self.cfg.axes
    }

    fn material(&self) -> &GearMaterialParams {
        &self.cfg.gear_material
    }

    pub fn get_axis_config(&self, axis_id: u8) -> Option<&AxisConfig> {
        self.axes().iter().find(|a| a.axis_id == axis_id)
    }

    fn hertz_contact_force(&self, delta: f64, delta_dot: f64, wear_level: f64) -> (f64, f64, f64) {
        if delta <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let m = self.material();
        let wear_correction = 1.0 - wear_level * 0.4;
        let k_eff = m.hertz_k_n_m15 * wear_correction;

        let f_elastic = k_eff * delta.powf(1.5);

        let beta = 3.0 * m.damping_ratio * (1.0 - m.restitution_coeff.powi(2))
            / (m.restitution_coeff * (m.restitution_coeff.powi(2) + 1.0));
        let f_damping = if delta_dot < 0.0 {
            beta * k_eff * delta.powf(1.5) * delta_dot.abs() / 1000.0
        } else {
            beta * k_eff * delta.powf(1.5) * delta_dot / 1000.0
        };

        let f_total = f_elastic + f_damping;
        let u_elastic = (2.0 / 5.0) * k_eff * delta.powf(2.5);
        (f_total, f_elastic, u_elastic)
    }

    pub fn simulate_backlash_collision(
        &self,
        stage: &crate::models::GearStage,
        angular_velocity: f64,
        direction_change: bool,
        wear_level: f64,
    ) -> (f64, f64, f64) {
        let mut rng = rand::thread_rng();
        let m = self.material();
        let effective_backlash = stage.backlash * (1.0 + wear_level * 2.5);

        if !direction_change || angular_velocity.abs() < 0.005 {
            let micro = effective_backlash * 0.05 * rng.gen::<f64>();
            return (micro, 0.0, micro * 0.3);
        }

        let velocity_ms = angular_velocity * ARCMIN_TO_RAD * 0.05;

        let mut delta = 0.0_f64;
        let mut delta_dot = velocity_ms;
        let mut max_delta = 0.0_f64;
        let mut dissipated = 0.0_f64;
        let mut peak_force = 0.0_f64;
        let ke_before = 0.5 * m.tooth_equiv_mass_kg * velocity_ms.powi(2);

        for _ in 0..m.contact_iterations {
            let (f_total, _, _) = self.hertz_contact_force(delta, delta_dot, wear_level);
            peak_force = peak_force.max(f_total);

            let accel = -f_total / m.tooth_equiv_mass_kg;
            let ddot_new = delta_dot + accel * m.contact_dt_s;
            let d_new = (delta + delta_dot * m.contact_dt_s).max(0.0_f64);

            let w = if delta_dot.abs() > 1e-9 {
                (f_total - self.hertz_contact_force(delta, 0.0, wear_level).0).abs()
                    * (d_new - delta).abs()
            } else { 0.0_f64 };
            dissipated += w;

            delta = d_new;
            delta_dot = ddot_new;
            max_delta = max_delta.max(delta);
            if delta <= 0.0 && delta_dot > 0.0 { break; }
        }

        let ke_after = 0.5 * m.tooth_equiv_mass_kg * delta_dot.powi(2);
        let (_, _, u_final) = self.hertz_contact_force(delta, delta_dot, wear_level);
        let energy_check = (ke_before - ke_after - dissipated - u_final) / ke_before.max(1e-9);
        if energy_check.abs() > 0.05 {
            let cf = (ke_before - dissipated - u_final) / ke_after.max(1e-9);
            delta_dot *= cf.sqrt().min(1.1).max(0.9);
        }

        let av_out = delta_dot / (ARCMIN_TO_RAD * 0.05);
        let av_loss = (angular_velocity - av_out).abs();
        let ie_arcmin = (av_loss / angular_velocity.abs()).min(1.0)
            * effective_backlash * (0.55 + 0.2 * rng.gen::<f64>());

        let rv = if ie_arcmin > 0.05 {
            let fnat = (m.hertz_k_n_m15 * 1.5 * max_delta.sqrt() / m.tooth_equiv_mass_kg).sqrt();
            let decay = (-m.damping_ratio * fnat * 0.05).exp();
            ie_arcmin * (1.0 - decay) * (0.6 + 0.4 * rng.gen::<f64>())
        } else { 0.0 };

        (ie_arcmin, peak_force, rv)
    }

    pub fn simulate_single_stage(
        &self,
        stage: &crate::models::GearStage,
        input_angle: f64,
        rotation_direction: i32,
        wear_level: f64,
        temperature: f64,
        torque: f64,
    ) -> (f64, f64, f64, f64, f64, f64) {
        let mut rng = rand::thread_rng();
        let theoretical_output = input_angle * stage.theoretical_ratio;

        let wear_mul = 1.0 + wear_level * 3.0;
        let dynamic_meshing_err = stage.base_meshing_error * wear_mul
            * (1.0 + 0.3 * (input_angle * 2.0 * std::f64::consts::PI / 360.0).sin())
            + rng.gen_range(-0.05..0.05);

        let dir = if rotation_direction != 0 { (rotation_direction as f64).signum() } else { 0.0 };
        let backlash_contrib = if dir != 0.0 {
            stage.backlash * (1.0 + wear_level * 2.0) * 0.5 * (1.0 + dir)
                + rng.gen_range(-0.05..0.05) * (1.0 + wear_level)
        } else { 0.0 };

        let elastic_deflection = (torque * 1000.0 / stage.elastic_stiffness) * DEG_TO_ARCMIN
            * (1.0 + wear_level * 1.5);

        let temp_effect = (temperature - 20.0) * 8.5e-4 * DEG_TO_ARCMIN
            * (1.0 + wear_level * 0.5);

        let total_err = dynamic_meshing_err
            + backlash_contrib.abs()
            + elastic_deflection
            + temp_effect.abs();

        let noise = rng.gen_range(-0.03..0.03);
        let actual_output = theoretical_output - total_err / DEG_TO_ARCMIN + noise / DEG_TO_ARCMIN;
        let actual_ratio = if input_angle.abs() > 1e-10 { actual_output / input_angle } else { stage.theoretical_ratio };

        (theoretical_output, actual_output, actual_ratio,
         dynamic_meshing_err, backlash_contrib.abs(), elastic_deflection)
    }

    pub fn simulate_axis(
        &self,
        axis: &AxisConfig,
        input_angle: f64,
        rotation_direction: i32,
        wear_levels: &[f64],
        temperature: f64,
        torque: f64,
        device_id: &str,
        ts: chrono::DateTime<Utc>,
    ) -> TransmissionErrorResult {
        let mut accumulated_error = 0.0;
        let mut current_input = input_angle;
        let mut total_backlash = 0.0;
        let mut total_elastic = 0.0;
        let mut total_wear_err = 0.0;
        let mut total_temp = 0.0;
        let mut theoretical_total_ratio = 1.0;

        for (idx, stage) in axis.gear_stages.iter().enumerate() {
            let wl = wear_levels.get(idx).copied().unwrap_or(0.0);
            theoretical_total_ratio *= stage.theoretical_ratio;

            let (_, actual_out, _, mesh_err, blsh, elastic) =
                self.simulate_single_stage(stage, current_input, rotation_direction, wl, temperature, torque);

            accumulated_error += mesh_err + blsh + elastic;
            total_backlash += blsh;
            total_elastic += elastic;
            total_wear_err += mesh_err * wl * 2.0;
            total_temp += (temperature - 20.0) * 8.5e-4 * DEG_TO_ARCMIN * (1.0 + wl * 0.5);
            current_input = actual_out;
        }

        accumulated_error += axis.bearing_clearance * (1.0 + wear_levels.first().copied().unwrap_or(0.0));
        let final_out = current_input;
        let actual_total_ratio = if input_angle.abs() > 1e-10 { final_out / input_angle } else { theoretical_total_ratio };

        TransmissionErrorResult {
            timestamp: ts,
            device_id: device_id.to_string(),
            axis_id: axis.axis_id,
            input_angle,
            output_angle: final_out,
            theoretical_ratio: theoretical_total_ratio,
            actual_ratio: actual_total_ratio,
            single_stage_error: accumulated_error / axis.gear_stages.len() as f64,
            accumulated_error,
            backlash_error: total_backlash,
            elastic_deformation_error: total_elastic,
            wear_induced_error: total_wear_err,
            temperature_effect: total_temp,
        }
    }

    pub async fn run(mut self) {
        info!("TransmissionSimulator actor started");

        let rx = match self.rx.take() {
            Some(r) => r,
            None => { warn!("TransmissionSimulator has no rx, stopping"); return; }
        };

        tokio::pin!(rx);
        while let Some(msg) = rx.recv().await {
            match msg {
                PipelineMessage::ValidatedReading(reading) => {
                    let mut results = Vec::new();
                    for axis_id in 1u8..=3 {
                        if let Some(axis) = self.get_axis_config(axis_id) {
                            let angle = match axis_id {
                                1 => reading.axis_azimuth_angle,
                                2 => reading.axis_elevation_angle,
                                3 => reading.axis_equatorial_angle,
                                _ => 0.0,
                            };
                            let wear_levels = vec![
                                reading.gear_wear_level_1,
                                reading.gear_wear_level_2,
                                reading.gear_wear_level_3,
                            ];
                            let r = self.simulate_axis(
                                axis, angle, 1, &wear_levels,
                                reading.temperature, 5.0,
                                &reading.device_id, reading.timestamp,
                            );
                            if let Some(ch) = &self.ch_client {
                                if let Err(e) = ch.insert_transmission_error(&r).await {
                                    error!(error = %e, "Transmission CH insert error");
                                }
                            }
                            TRANSMISSION_JOBS_TOTAL
                                .with_label_values(&[&axis_id.to_string()])
                                .inc();
                            let r_arc = Arc::new(r);
                            results.push(r_arc.clone());

                            let m = PipelineMessage::TransmissionResult(r_arc);
                            if let Some(ws) = &self.ws_tx {
                                let _ = ws.send(m.clone()).await;
                            }
                            if let Some(alarm) = &self.alarm_tx {
                                let _ = alarm.send(m).await;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        warn!("TransmissionSimulator actor stopped");
    }
}
