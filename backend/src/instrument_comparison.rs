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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/config"))
    }

    mod instrument_type_parsing {
        use super::*;

        #[test]
        fn test_parse_hunyi_normal() {
            let inst = parse_instrument_type("hunyi");
            assert!(inst.is_some());
            let inst = inst.unwrap();
            assert_eq!(inst.display_name(), "浑仪");
            assert_eq!(inst.era(), "古代");
        }

        #[test]
        fn test_parse_jianyi_normal() {
            let inst = parse_instrument_type("jianyi");
            assert!(inst.is_some());
            let inst = inst.unwrap();
            assert_eq!(inst.display_name(), "简仪");
            assert_eq!(inst.era(), "古代");
        }

        #[test]
        fn test_parse_xiangyiyi_normal() {
            let inst = parse_instrument_type("xiangyiyi");
            assert!(inst.is_some());
            let inst = inst.unwrap();
            assert_eq!(inst.display_name(), "象限仪");
            assert_eq!(inst.era(), "古代");
        }

        #[test]
        fn test_parse_modern_eq_normal() {
            let inst = parse_instrument_type("modern_eq");
            assert!(inst.is_some());
            let inst = inst.unwrap();
            assert_eq!(inst.display_name(), "现代赤道仪");
            assert_eq!(inst.era(), "现代");
        }

        #[test]
        fn test_parse_chinese_name_alias() {
            assert!(parse_instrument_type("浑仪").is_some());
            assert!(parse_instrument_type("简仪").is_some());
            assert!(parse_instrument_type("象限仪").is_some());
            assert!(parse_instrument_type("现代赤道仪").is_some());
        }

        #[test]
        fn test_parse_case_insensitive() {
            assert!(parse_instrument_type("HUNYI").is_some());
            assert!(parse_instrument_type("JianYi").is_some());
        }

        #[test]
        fn test_parse_unknown_returns_none() {
            assert!(parse_instrument_type("").is_none());
            assert!(parse_instrument_type("unknown").is_none());
            assert!(parse_instrument_type("guzhenyi").is_none());
        }
    }

    mod config_loading {
        use super::*;

        #[test]
        fn test_load_hunyi_config() {
            let inst = parse_instrument_type("hunyi").unwrap();
            let cfg = load_instrument_config(&inst, &test_config_dir());
            assert!(cfg.is_ok(), "浑仪配置加载失败: {:?}", cfg.err());
            let cfg = cfg.unwrap();
            assert_eq!(cfg.axes.len(), 3, "浑仪应有3根轴");
            assert!(cfg.axes.iter().all(|a| !a.gear_stages.is_empty()), "每根轴应有齿轮级");
        }

        #[test]
        fn test_load_jianyi_config() {
            let inst = parse_instrument_type("jianyi").unwrap();
            let cfg = load_instrument_config(&inst, &test_config_dir());
            assert!(cfg.is_ok(), "简仪配置加载失败");
            let cfg = cfg.unwrap();
            assert_eq!(cfg.axes.len(), 3, "简仪应有3根轴");
            assert!(cfg.axes.iter().all(|a| a.gear_stages.len() == 1), "简仪应为单级传动");
        }

        #[test]
        fn test_load_xiangyiyi_config() {
            let inst = parse_instrument_type("xiangyiyi").unwrap();
            let cfg = load_instrument_config(&inst, &test_config_dir());
            assert!(cfg.is_ok(), "象限仪配置加载失败");
            let cfg = cfg.unwrap();
            assert_eq!(cfg.axes.len(), 3);
        }

        #[test]
        fn test_load_modern_eq_config() {
            let inst = parse_instrument_type("modern_eq").unwrap();
            let cfg = load_instrument_config(&inst, &test_config_dir());
            assert!(cfg.is_ok(), "现代赤道仪配置加载失败");
            let cfg = cfg.unwrap();
            assert_eq!(cfg.axes.len(), 3);
        }
    }

    mod comparison_run {
        use super::*;
        use crate::models::ComparisonRequest;

        fn make_request(instruments: Vec<&str>) -> ComparisonRequest {
            ComparisonRequest {
                instruments: instruments.iter().map(|s| s.to_string()).collect(),
                azimuth_angle: 45.0,
                elevation_angle: 60.0,
                equatorial_angle: 30.0,
                temperature: 20.0,
                wear_level: 0.1,
            }
        }

        #[test]
        fn test_single_instrument_comparison() {
            let req = make_request(vec!["hunyi"]);
            let result = run_comparison(&req, &test_config_dir());
            assert!(result.is_ok());
            let result = result.unwrap();
            assert_eq!(result.results.len(), 1);
            assert!(result.results[0].cumulative_error > 0.0, "累积误差应大于0");
        }

        #[test]
        fn test_two_ancient_instruments() {
            let req = make_request(vec!["hunyi", "jianyi"]);
            let result = run_comparison(&req, &test_config_dir());
            assert!(result.is_ok());
            let result = result.unwrap();
            assert_eq!(result.results.len(), 2);
            assert_eq!(result.results[0].era, "古代");
            assert_eq!(result.results[1].era, "古代");
        }

        #[test]
        fn test_cross_era_comparison() {
            let req = make_request(vec!["hunyi", "modern_eq"]);
            let result = run_comparison(&req, &test_config_dir());
            assert!(result.is_ok());
            let result = result.unwrap();
            assert_eq!(result.results.len(), 2);

            let ancient = &result.results[0];
            let modern = &result.results[1];
            assert_eq!(ancient.era, "古代");
            assert_eq!(modern.era, "现代");

            assert!(
                modern.cumulative_error < ancient.cumulative_error,
                "现代仪器累积误差应小于古代: 现代={} < 古代={}",
                modern.cumulative_error, ancient.cumulative_error
            );
        }

        #[test]
        fn test_all_four_instruments() {
            let req = make_request(vec!["hunyi", "jianyi", "xiangyiyi", "modern_eq"]);
            let result = run_comparison(&req, &test_config_dir());
            assert!(result.is_ok());
            let result = result.unwrap();
            assert_eq!(result.results.len(), 4);
            assert!(result.results.iter().all(|r| r.cumulative_error > 0.0));
        }

        #[test]
        fn test_invalid_instrument_returns_error() {
            let req = make_request(vec!["hunyi", "invalid"]);
            let result = run_comparison(&req, &test_config_dir());
            assert!(result.is_err(), "无效仪器类型应返回错误");
        }

        #[test]
        fn test_empty_instruments() {
            let req = ComparisonRequest {
                instruments: vec![],
                azimuth_angle: 0.0,
                elevation_angle: 0.0,
                equatorial_angle: 0.0,
                temperature: 20.0,
                wear_level: 0.0,
            };
            let result = run_comparison(&req, &test_config_dir());
            assert!(result.is_ok());
            assert_eq!(result.unwrap().results.len(), 0);
        }

        #[test]
        fn test_wear_increases_error() {
            let mut req_low = make_request(vec!["hunyi"]);
            req_low.wear_level = 0.01;
            let result_low = run_comparison(&req_low, &test_config_dir()).unwrap();

            let mut req_high = make_request(vec!["hunyi"]);
            req_high.wear_level = 0.8;
            let result_high = run_comparison(&req_high, &test_config_dir()).unwrap();

            assert!(
                result_high.results[0].cumulative_error > result_low.results[0].cumulative_error,
                "高磨损应导致更高误差: 低磨损={}, 高磨损={}",
                result_low.results[0].cumulative_error, result_high.results[0].cumulative_error
            );
        }

        #[test]
        fn test_temperature_effect() {
            let mut req_cold = make_request(vec!["hunyi"]);
            req_cold.temperature = -20.0;
            let result_cold = run_comparison(&req_cold, &test_config_dir()).unwrap();

            let mut req_hot = make_request(vec!["hunyi"]);
            req_hot.temperature = 40.0;
            let result_hot = run_comparison(&req_hot, &test_config_dir()).unwrap();

            assert!(
                (result_hot.results[0].avg_temp_effect - result_cold.results[0].avg_temp_effect).abs() > 0.001,
                "不同温度下温度效应应不同"
            );
        }

        #[test]
        fn test_error_components_positive() {
            let req = make_request(vec!["hunyi"]);
            let result = run_comparison(&req, &test_config_dir()).unwrap();
            let r = &result.results[0];
            assert!(r.avg_backlash >= 0.0, "齿隙误差非负");
            assert!(r.avg_elastic >= 0.0, "弹性误差非负");
            assert!(r.avg_wear_error >= 0.0, "磨损误差非负");
            assert!(r.cumulative_error > 0.0, "累积误差为正");
        }

        #[test]
        fn test_result_consistency_same_input() {
            let req = make_request(vec!["hunyi", "jianyi"]);
            let r1 = run_comparison(&req, &test_config_dir()).unwrap();
            let r2 = run_comparison(&req, &test_config_dir()).unwrap();

            assert_eq!(r1.results.len(), r2.results.len());
            for i in 0..r1.results.len() {
                let e1 = r1.results[i].cumulative_error;
                let e2 = r2.results[i].cumulative_error;
                let rel_err = (e1 - e2).abs() / e1.max(e2);
                assert!(rel_err < 0.15,
                        "相同输入结果应在15%以内（含随机噪声）: {} vs {} ({}%)",
                        e1, e2, rel_err * 100.0);
            }

            assert_eq!(r1.results[0].instrument_type, r2.results[0].instrument_type,
                       "仪器排序应一致");
        }

        #[test]
        fn test_boundary_zero_azimuth() {
            let mut req = make_request(vec!["hunyi"]);
            req.azimuth_angle = 0.0;
            let result = run_comparison(&req, &test_config_dir());
            assert!(result.is_ok(), "0°方位角边界应正常");
        }

        #[test]
        fn test_boundary_90_elevation() {
            let mut req = make_request(vec!["hunyi"]);
            req.elevation_angle = 90.0;
            let result = run_comparison(&req, &test_config_dir());
            assert!(result.is_ok(), "90°高度角边界应正常");
        }

        #[test]
        fn test_boundary_full_wear() {
            let mut req = make_request(vec!["hunyi"]);
            req.wear_level = 1.0;
            let result = run_comparison(&req, &test_config_dir());
            assert!(result.is_ok(), "100%磨损边界应正常");
            assert!(result.unwrap().results[0].cumulative_error > 0.0);
        }
    }
}
