use crate::models::{
    DegradationDataPoint, DegradationRequest, DegradationResponse,
    HunyiError,
};
use crate::pointing_analyzer::PointingAnalyzer;
use crate::transmission_simulator::TransmissionSimulator;
use chrono::Utc;
use rand::Rng;
use std::path::PathBuf;
use tracing::info;

pub fn run_degradation(
    req: &DegradationRequest,
    config_dir: &PathBuf,
) -> Result<DegradationResponse, HunyiError> {
    let inst_type = crate::instrument_comparison::parse_instrument_type(&req.instrument)
        .ok_or_else(|| HunyiError::Config(format!("未知仪器类型: {}", req.instrument)))?;

    let cfg = crate::instrument_comparison::load_instrument_config(&inst_type, config_dir)?;
    let sim = TransmissionSimulator::new_standalone(cfg.clone());
    let analyzer = PointingAnalyzer::new_standalone(cfg);

    let mut data_points = Vec::new();
    let steps = req.steps.max(1);
    let hours_per_step = req.total_hours as f64 / steps as f64;
    let mut current_wear = req.initial_wear;
    let mut rng = rand::thread_rng();

    info!(
        instrument = inst_type.display_name(),
        total_hours = req.total_hours,
        steps = steps,
        "Starting degradation simulation"
    );

    for step in 0..=steps {
        let elapsed_hours = step as f64 * hours_per_step;

        if step > 0 {
            let wear_increment = rng.gen_range(0.0001..0.0005) * req.wear_rate * hours_per_step / 100.0;
            current_wear = (current_wear + wear_increment).min(0.99);
        }

        let wear_levels = vec![current_wear; 3];
        let mut total_cumulative = 0.0;
        let mut total_backlash = 0.0;
        let mut total_elastic = 0.0;
        let mut total_wear_err = 0.0;
        let mut total_temp = 0.0;
        let mut total_meshing_avg = 0.0;
        let ts = Utc::now();

        for axis in &sim.get_config().axes {
            let angle = match axis.axis_id {
                1 => req.azimuth_angle,
                2 => req.elevation_angle,
                _ => 30.0_f64,
            };

            let r = sim.simulate_axis(
                axis, angle, 1, &wear_levels,
                req.temperature, 5.0, &req.instrument, ts,
            );

            total_cumulative += r.accumulated_error;
            total_backlash += r.backlash_error;
            total_elastic += r.elastic_deformation_error;
            total_wear_err += r.wear_induced_error;
            total_temp += r.temperature_effect;
            total_meshing_avg += r.single_stage_error;
        }

        let n = sim.get_config().axes.len() as f64;
        let etc = analyzer.compute_etc(req.azimuth_angle, req.elevation_angle, total_cumulative);

        let pointing_err = (total_cumulative * etc * 0.4).sqrt() * 0.5;

        data_points.push(DegradationDataPoint {
            elapsed_hours,
            wear_level: current_wear,
            cumulative_error: total_cumulative,
            avg_backlash: total_backlash / n,
            avg_elastic: total_elastic / n,
            avg_wear_error: total_wear_err / n,
            total_pointing_error: pointing_err,
            error_transfer_coefficient: etc,
            gear_meshing_error_avg: total_meshing_avg / n,
        });
    }

    Ok(DegradationResponse {
        instrument: req.instrument.clone(),
        instrument_name: inst_type.display_name().to_string(),
        total_hours: req.total_hours,
        data_points,
    })
}
