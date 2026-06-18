use crate::models::{
    ComparisonRequest, ComparisonResponse, GearParamsConfig, InstrumentComparisonResult,
    InstrumentType, HunyiError,
};
use crate::transmission_simulator::TransmissionSimulator;
use chrono::Utc;
use std::path::PathBuf;
use tracing::info;

pub fn parse_instrument_type(name: &str) -> Option<InstrumentType> {
    match name.to_lowercase().as_str() {
        "hunyi" | "浑仪" => Some(InstrumentType::Hunyi),
        "jianyi" | "简仪" => Some(InstrumentType::Jianyi),
        "xiangyiyi" | "象限仪" => Some(InstrumentType::Xiangyiyi),
        "modern_eq" | "现代赤道仪" => Some(InstrumentType::ModernEQ),
        _ => None,
    }
}

pub fn load_instrument_config(inst: &InstrumentType, config_dir: &PathBuf) -> Result<GearParamsConfig, HunyiError> {
    let filename = inst.config_filename();
    let path = config_dir.join(filename);
    GearParamsConfig::load_from_file(&path)
}

pub fn run_comparison(
    req: &ComparisonRequest,
    config_dir: &PathBuf,
) -> Result<ComparisonResponse, HunyiError> {
    let mut results = Vec::new();

    for inst_name in &req.instruments {
        let inst_type = parse_instrument_type(inst_name).ok_or_else(|| {
            HunyiError::Config(format!("未知仪器类型: {}", inst_name))
        })?;

        let cfg = load_instrument_config(&inst_type, config_dir)?;
        let sim = TransmissionSimulator::new_standalone(cfg);

        let wear_levels = vec![req.wear_level; 3];
        let mut transmission_results = Vec::new();
        let mut total_cumulative = 0.0;
        let mut total_backlash = 0.0;
        let mut total_elastic = 0.0;
        let mut total_wear_err = 0.0;
        let mut total_temp = 0.0;
        let mut max_single = 0.0_f64;
        let ts = Utc::now();

        for axis in &sim.get_config().axes {
            let angle = match axis.axis_id {
                1 => req.azimuth_angle,
                2 => req.elevation_angle,
                3 => req.equatorial_angle,
                _ => 0.0,
            };

            let r = sim.simulate_axis(
                axis, angle, 1, &wear_levels,
                req.temperature, 5.0, &inst_name, ts,
            );

            max_single = max_single.max(r.accumulated_error);
            total_cumulative += r.accumulated_error;
            total_backlash += r.backlash_error;
            total_elastic += r.elastic_deformation_error;
            total_wear_err += r.wear_induced_error;
            total_temp += r.temperature_effect;

            transmission_results.push(r);
        }

        let n = transmission_results.len() as f64;
        results.push(InstrumentComparisonResult {
            instrument_type: inst_name.clone(),
            instrument_name: inst_type.display_name().to_string(),
            era: inst_type.era().to_string(),
            transmission_results,
            cumulative_error: total_cumulative,
            max_single_axis_error: max_single,
            avg_backlash: total_backlash / n,
            avg_elastic: total_elastic / n,
            avg_wear_error: total_wear_err / n,
            avg_temp_effect: total_temp / n,
        });

        info!(
            instrument = inst_type.display_name(),
            cumulative_error = total_cumulative,
            "Comparison instrument simulated"
        );
    }

    Ok(ComparisonResponse {
        request: req.clone(),
        results,
    })
}
