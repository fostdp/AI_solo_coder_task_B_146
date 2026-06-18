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

const ARCHARD_K_BRONZE: f64 = 2.5e-4;
const ARCHARD_K_STEEL: f64 = 5.0e-5;
const ACCEL_FACTOR: f64 = 8.0;
const LOAD_N: f64 = 15.0;
const SLIDE_RATIO: f64 = 0.04;
const HARDNESS_BRONZE_HV: f64 = 800.0;
const HARDNESS_STEEL_HV: f64 = 2500.0;

pub fn archard_wear_increment(
    is_ancient: bool,
    wear_rate: f64,
    hours_per_step: f64,
    current_wear: f64,
) -> f64 {
    let k = if is_ancient { ARCHARD_K_BRONZE } else { ARCHARD_K_STEEL };
    let hardness = if is_ancient { HARDNESS_BRONZE_HV } else { HARDNESS_STEEL_HV };

    let contact_pressure = LOAD_N * (1.0 + current_wear * 3.0);
    let slide_distance = SLIDE_RATIO * hours_per_step * 3600.0 * ACCEL_FACTOR;
    let base_increment = (k * contact_pressure * slide_distance) / hardness;
    let wear_progression = 1.0 + current_wear * 2.0;

    let mut rng = rand::thread_rng();
    let noise = rng.gen_range(0.85..1.15);

    (base_increment * wear_rate * wear_progression * noise).min(0.02)
}

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
    let is_ancient = inst_type != crate::models::InstrumentType::ModernEQ;

    info!(
        instrument = inst_type.display_name(),
        total_hours = req.total_hours,
        steps = steps,
        wear_model = if is_ancient { "Archard-K(Bronze)" } else { "Archard-K(Steel)" },
        "Starting degradation simulation"
    );

    for step in 0..=steps {
        let elapsed_hours = step as f64 * hours_per_step;

        if step > 0 {
            let wear_increment = archard_wear_increment(
                is_ancient, req.wear_rate, hours_per_step, current_wear,
            );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DegradationRequest;

    fn test_config_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/config"))
    }

    fn default_req() -> DegradationRequest {
        DegradationRequest {
            instrument: "hunyi".to_string(),
            total_hours: 1000,
            steps: 20,
            initial_wear: 0.05,
            wear_rate: 1.0,
            temperature: 20.0,
            azimuth_angle: 45.0,
            elevation_angle: 60.0,
        }
    }

    mod normal_cases {
        use super::*;

        #[test]
        fn test_degradation_basic_structure() {
            let req = default_req();
            let result = run_degradation(&req, &test_config_dir());
            assert!(result.is_ok(), "退化仿真应成功");
            let result = result.unwrap();

            assert_eq!(result.instrument_name, "浑仪");
            assert_eq!(result.total_hours, 1000);
            assert_eq!(result.data_points.len(), 21, "20步应有21个数据点（含起点）");
        }

        #[test]
        fn test_data_points_monotonic_wear() {
            let req = default_req();
            let result = run_degradation(&req, &test_config_dir()).unwrap();
            let pts = &result.data_points;

            for i in 1..pts.len() {
                assert!(
                    pts[i].wear_level >= pts[i-1].wear_level - 1e-9,
                    "磨损应单调不减: step {}: {} >= {} failed",
                    i, pts[i].wear_level, pts[i-1].wear_level
                );
            }
        }

        #[test]
        fn test_wear_starts_at_initial() {
            let mut req = default_req();
            req.initial_wear = 0.15;
            let result = run_degradation(&req, &test_config_dir()).unwrap();
            let first = &result.data_points[0];
            assert!((first.wear_level - 0.15).abs() < 1e-6,
                    "初始磨损应等于输入值: {} != 0.15", first.wear_level);
        }

        #[test]
        fn test_cumulative_error_increases_with_wear() {
            let mut req = default_req();
            req.wear_rate = 50.0;
            req.total_hours = 5000;
            req.initial_wear = 0.01;
            let result = run_degradation(&req, &test_config_dir()).unwrap();
            let pts = &result.data_points;

            let first_err = pts[0].cumulative_error;
            let last_err = pts[pts.len() - 1].cumulative_error;
            assert!(last_err > first_err * 1.1,
                    "显著磨损下累积误差应明显增加: first={}, last={}", first_err, last_err);
        }

        #[test]
        fn test_etc_within_reasonable_range() {
            let req = default_req();
            let result = run_degradation(&req, &test_config_dir()).unwrap();
            let pts = &result.data_points;

            for pt in pts {
                assert!(pt.error_transfer_coefficient >= 1.0,
                        "ETC不应小于1: {}", pt.error_transfer_coefficient);
                assert!(pt.error_transfer_coefficient < 100.0,
                        "ETC不应过大: {}", pt.error_transfer_coefficient);
            }
        }

        #[test]
        fn test_elapsed_hours_linear_progress() {
            let mut req = default_req();
            req.total_hours = 1000;
            req.steps = 10;
            let result = run_degradation(&req, &test_config_dir()).unwrap();
            let pts = &result.data_points;

            assert_eq!(pts.len(), 11);
            assert!((pts[0].elapsed_hours - 0.0).abs() < 1e-6, "起点为0小时");
            assert!((pts[10].elapsed_hours - 1000.0).abs() < 1e-6, "终点为1000小时");
            assert!((pts[5].elapsed_hours - 500.0).abs() < 1e-6, "中点为500小时");
        }
    }

    mod boundary_cases {
        use super::*;

        #[test]
        fn test_zero_hours_multiple_points() {
            let mut req = default_req();
            req.total_hours = 0;
            let result = run_degradation(&req, &test_config_dir());
            assert!(result.is_ok());
            let pts = result.unwrap().data_points;
            assert!(pts.len() >= 2, "0小时也有起点和终点（至少2个点）");
            assert!((pts[0].elapsed_hours - 0.0).abs() < 1e-6);
            assert!((pts.last().unwrap().elapsed_hours - 0.0).abs() < 1e-6, "所有点都在0小时");
        }

        #[test]
        fn test_one_step_two_points() {
            let mut req = default_req();
            req.steps = 1;
            let result = run_degradation(&req, &test_config_dir()).unwrap();
            assert_eq!(result.data_points.len(), 2);
        }

        #[test]
        fn test_zero_wear_rate_stable() {
            let mut req = default_req();
            req.initial_wear = 0.1;
            req.wear_rate = 0.0;
            req.total_hours = 1000;
            let result = run_degradation(&req, &test_config_dir()).unwrap();
            let pts = &result.data_points;

            let first_wear = pts[0].wear_level;
            let last_wear = pts[pts.len()-1].wear_level;
            assert!((last_wear - first_wear).abs() < 0.05,
                    "零磨损速率下磨损应基本不变: {} vs {}", first_wear, last_wear);
        }

        #[test]
        fn test_high_wear_rate_clamps_at_99() {
            let mut req = default_req();
            req.initial_wear = 0.9;
            req.wear_rate = 100.0;
            req.total_hours = 10000;
            let result = run_degradation(&req, &test_config_dir()).unwrap();
            let pts = result.data_points;

            for pt in &pts {
                assert!(pt.wear_level <= 1.0, "磨损不应超过上限: {}", pt.wear_level);
            }
        }

        #[test]
        fn test_high_elevation_high_etc() {
            let mut req_low = default_req();
            req_low.elevation_angle = 10.0;
            let result_low = run_degradation(&req_low, &test_config_dir()).unwrap();

            let mut req_high = default_req();
            req_high.elevation_angle = 80.0;
            let result_high = run_degradation(&req_high, &test_config_dir()).unwrap();

            let etc_low = result_low.data_points[0].error_transfer_coefficient;
            let etc_high = result_high.data_points[0].error_transfer_coefficient;
            assert!(etc_high > etc_low,
                    "高仰角ETC应更高(方位角误差放大效应): low={} high={}", etc_low, etc_high);
        }
    }

    mod error_cases {
        use super::*;

        #[test]
        fn test_invalid_instrument_returns_error() {
            let mut req = default_req();
            req.instrument = "invalid".to_string();
            let result = run_degradation(&req, &test_config_dir());
            assert!(result.is_err(), "无效仪器应返回错误");
        }

        #[test]
        fn test_zero_steps_clamped_to_one() {
            let mut req = default_req();
            req.steps = 0;
            let result = run_degradation(&req, &test_config_dir());
            assert!(result.is_ok(), "0步应被clamp为1步");
            assert_eq!(result.unwrap().data_points.len(), 2, "0步clamp为1步→2个数据点");
        }

        #[test]
        fn test_modern_eq_degrades_slower() {
            let req_ancient = DegradationRequest {
                instrument: "hunyi".to_string(),
                total_hours: 1000,
                steps: 20,
                initial_wear: 0.1,
                wear_rate: 1.0,
                temperature: 20.0,
                azimuth_angle: 45.0,
                elevation_angle: 60.0,
            };
            let req_modern = DegradationRequest {
                instrument: "modern_eq".to_string(),
                total_hours: 1000,
                steps: 20,
                initial_wear: 0.1,
                wear_rate: 1.0,
                temperature: 20.0,
                azimuth_angle: 45.0,
                elevation_angle: 60.0,
            };

            let ancient = run_degradation(&req_ancient, &test_config_dir()).unwrap();
            let modern = run_degradation(&req_modern, &test_config_dir()).unwrap();

            assert!(
                modern.data_points.last().unwrap().cumulative_error
                < ancient.data_points.last().unwrap().cumulative_error,
                "现代仪器相同磨损下误差应更小"
            );
        }
    }

    mod error_components {
        use super::*;

        #[test]
        fn test_all_components_positive() {
            let req = default_req();
            let result = run_degradation(&req, &test_config_dir()).unwrap();

            for pt in &result.data_points {
                assert!(pt.avg_backlash >= 0.0, "齿隙非负");
                assert!(pt.avg_elastic >= 0.0, "弹性非负");
                assert!(pt.avg_wear_error >= 0.0, "磨损误差非负");
                assert!(pt.cumulative_error >= 0.0, "累积误差非负");
                assert!(pt.total_pointing_error >= 0.0, "指向误差非负");
                assert!(pt.error_transfer_coefficient > 0.0, "ETC为正");
            }
        }

        #[test]
        fn test_wear_error_component_grows() {
            let req = default_req();
            let result = run_degradation(&req, &test_config_dir()).unwrap();
            let pts = &result.data_points;

            let first = pts[0].avg_wear_error;
            let last = pts[pts.len()-1].avg_wear_error;
            assert!(last >= first, "磨损误差分量应随时间增长");
        }
    }
}
