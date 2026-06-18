use crate::models::{
    ForceFeedback, HunyiError, VirtualRotationRequest,
    VirtualRotationResponse, VisibleStar,
};
use crate::pointing_analyzer::PointingAnalyzer;
use crate::transmission_simulator::TransmissionSimulator;
use chrono::Utc;
use std::path::PathBuf;

const DEG_TO_ARCMIN: f64 = 60.0;
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;

const VISCOUS_DAMPING_ANCIENT: f64 = 0.35;
const VISCOUS_DAMPING_MODERN: f64 = 0.08;
const COULOMB_FRICTION_ANCIENT: f64 = 0.15;
const COULOMB_FRICTION_MODERN: f64 = 0.02;
const INERTIA_ANCIENT: f64 = 0.50;
const INERTIA_MODERN: f64 = 0.05;

fn compute_force_feedback(
    is_ancient: bool,
    angular_velocity_deg_s: f64,
    wear_level: f64,
    total_backlash_arcmin: f64,
) -> ForceFeedback {
    let (base_viscous, base_coulomb, base_inertia) = if is_ancient {
        (VISCOUS_DAMPING_ANCIENT, COULOMB_FRICTION_ANCIENT, INERTIA_ANCIENT)
    } else {
        (VISCOUS_DAMPING_MODERN, COULOMB_FRICTION_MODERN, INERTIA_MODERN)
    };

    let wear_factor = 1.0 + wear_level * 1.5;
    let viscous = base_viscous * angular_velocity_deg_s.abs() * wear_factor;
    let coulomb = base_coulomb * wear_factor * if angular_velocity_deg_s.abs() > 0.01 { 1.0 } else { 0.3 };
    let inertia = base_inertia * wear_factor;

    let total_resistance = viscous + coulomb;
    let angular_acceleration = if inertia > 1e-9 { total_resistance / inertia } else { 0.0 };

    let backlash_deadband = total_backlash_arcmin / 60.0;
    let is_backlash_zone = angular_velocity_deg_s.abs() < 0.05 && total_backlash_arcmin > 0.1;

    ForceFeedback {
        viscous_damping_nm: viscous,
        coulomb_friction_nm: coulomb,
        inertia_nm_s2: inertia,
        total_resistance_nm: total_resistance,
        angular_velocity_deg_s,
        angular_acceleration_deg_s2: angular_acceleration,
        is_backlash_zone,
        backlash_deadband_deg: backlash_deadband,
    }
}

struct StarEntry {
    name: &'static str,
    ra: f64,
    dec: f64,
    magnitude: f64,
    constellation: &'static str,
}

static STAR_CATALOG: &[StarEntry] = &[
    StarEntry { name: "北极星", ra: 37.95, dec: 89.26, magnitude: 1.98, constellation: "小熊座" },
    StarEntry { name: "织女星", ra: 279.23, dec: 38.78, magnitude: 0.03, constellation: "天琴座" },
    StarEntry { name: "牛郎星", ra: 297.70, dec: 8.87, magnitude: 0.77, constellation: "天鹰座" },
    StarEntry { name: "天津四", ra: 310.36, dec: 45.28, magnitude: 1.25, constellation: "天鹅座" },
    StarEntry { name: "参宿四", ra: 88.79, dec: 7.41, magnitude: 0.50, constellation: "猎户座" },
    StarEntry { name: "参宿七", ra: 78.63, dec: -8.20, magnitude: 0.13, constellation: "猎户座" },
    StarEntry { name: "天狼星", ra: 101.29, dec: -16.72, magnitude: -1.46, constellation: "大犬座" },
    StarEntry { name: "南河三", ra: 114.83, dec: 5.22, magnitude: 0.34, constellation: "小犬座" },
    StarEntry { name: "北河二", ra: 113.65, dec: 31.89, magnitude: 1.58, constellation: "双子座" },
    StarEntry { name: "北河三", ra: 116.33, dec: 28.03, magnitude: 1.14, constellation: "双子座" },
    StarEntry { name: "轩辕十四", ra: 152.09, dec: 11.97, magnitude: 1.35, constellation: "狮子座" },
    StarEntry { name: "大角星", ra: 213.92, dec: 19.18, magnitude: -0.05, constellation: "牧夫座" },
    StarEntry { name: "角宿一", ra: 201.30, dec: -11.16, magnitude: 0.97, constellation: "室女座" },
    StarEntry { name: "心宿二", ra: 247.35, dec: -26.43, magnitude: 1.09, constellation: "天蝎座" },
    StarEntry { name: "河鼓二", ra: 297.70, dec: 8.87, magnitude: 0.77, constellation: "天鹰座" },
    StarEntry { name: "五车二", ra: 79.17, dec: 45.99, magnitude: 0.08, constellation: "御夫座" },
    StarEntry { name: "毕宿五", ra: 68.98, dec: 16.51, magnitude: 0.85, constellation: "金牛座" },
    StarEntry { name: "北落师门", ra: 344.41, dec: -29.62, magnitude: 1.16, constellation: "南鱼座" },
    StarEntry { name: "天枢", ra: 165.93, dec: 61.75, magnitude: 1.79, constellation: "大熊座" },
    StarEntry { name: "天璇", ra: 165.46, dec: 56.38, magnitude: 2.37, constellation: "大熊座" },
    StarEntry { name: "天玑", ra: 178.46, dec: 53.69, magnitude: 2.44, constellation: "大熊座" },
    StarEntry { name: "天权", ra: 183.86, dec: 57.03, magnitude: 3.31, constellation: "大熊座" },
    StarEntry { name: "玉衡", ra: 193.51, dec: 55.96, magnitude: 1.77, constellation: "大熊座" },
    StarEntry { name: "开阳", ra: 200.98, dec: 54.93, magnitude: 2.27, constellation: "大熊座" },
    StarEntry { name: "摇光", ra: 206.89, dec: 49.31, magnitude: 1.86, constellation: "大熊座" },
];

fn angular_distance(ra1: f64, dec1: f64, ra2: f64, dec2: f64) -> f64 {
    let r1 = ra1 * DEG_TO_RAD;
    let d1 = dec1 * DEG_TO_RAD;
    let r2 = ra2 * DEG_TO_RAD;
    let d2 = dec2 * DEG_TO_RAD;
    let cos_d = d1.sin() * d2.sin() + d1.cos() * d2.cos() * (r1 - r2).cos();
    cos_d.acos().max(0.0) / DEG_TO_RAD * DEG_TO_ARCMIN
}

pub fn run_virtual_rotation(
    req: &VirtualRotationRequest,
    config_dir: &PathBuf,
) -> Result<VirtualRotationResponse, HunyiError> {
    let inst_type = crate::instrument_comparison::parse_instrument_type(&req.instrument)
        .ok_or_else(|| HunyiError::Config(format!("未知仪器类型: {}", req.instrument)))?;

    let cfg = crate::instrument_comparison::load_instrument_config(&inst_type, config_dir)?;
    let sim = TransmissionSimulator::new_standalone(cfg.clone());
    let analyzer = PointingAnalyzer::new_standalone(cfg);

    let wear_levels = vec![req.wear_level; 3];
    let mut total_cumulative = 0.0;
    let mut total_backlash = 0.0;
    let ts = Utc::now();

    for axis in &sim.get_config().axes {
        let angle = match axis.axis_id {
            1 => req.azimuth_angle,
            2 => req.elevation_angle,
            _ => req.equatorial_angle,
        };
        let r = sim.simulate_axis(
            axis, angle, 1, &wear_levels,
            req.temperature, 5.0, &req.instrument, ts,
        );
        total_cumulative += r.accumulated_error;
        total_backlash += r.backlash_error;
    }

    let is_ancient = inst_type != crate::models::InstrumentType::ModernEQ;
    let angular_velocity_deg_s = sim.get_config().shaft.operating_speed_rpm * 6.0;
    let force_feedback = compute_force_feedback(
        is_ancient,
        angular_velocity_deg_s,
        req.wear_level,
        total_backlash,
    );

    let lst = 12.0 * 15.0;
    let lat = 34.25;
    let (az, el) = PointingAnalyzer::equatorial_to_altaz(
        req.equatorial_angle, req.elevation_angle, lst, lat,
    );

    let pointing_ra = req.equatorial_angle + total_cumulative / 3600.0;
    let pointing_dec = req.elevation_angle + total_cumulative / 7200.0;

    let etc = analyzer.compute_etc(az, el, total_cumulative);
    let pointing_err = total_cumulative * etc * 0.3;

    let sky_zone = PointingAnalyzer::determine_sky_zone(pointing_dec);

    let visible_stars: Vec<VisibleStar> = STAR_CATALOG
        .iter()
        .filter_map(|star| {
            let dist = angular_distance(pointing_ra, pointing_dec, star.ra, star.dec);
            if dist < 600.0 {
                Some(VisibleStar {
                    name: star.name.to_string(),
                    ra: star.ra,
                    dec: star.dec,
                    magnitude: star.magnitude,
                    constellation: star.constellation.to_string(),
                    angular_distance_arcmin: dist,
                })
            } else {
                None
            }
        })
        .collect();

    tracing::debug!(
        pointing_ra = pointing_ra,
        pointing_dec = pointing_dec,
        visible_count = visible_stars.len(),
        "Virtual rotation computed"
    );

    Ok(VirtualRotationResponse {
        pointing_ra,
        pointing_dec,
        transmission_error: total_cumulative,
        pointing_error: pointing_err,
        error_transfer_coefficient: etc,
        visible_stars,
        sky_zone,
        force_feedback,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::VirtualRotationRequest;

    fn test_config_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/config"))
    }

    fn default_req() -> VirtualRotationRequest {
        VirtualRotationRequest {
            azimuth_angle: 45.0,
            elevation_angle: 60.0,
            equatorial_angle: 30.0,
            instrument: "hunyi".to_string(),
            wear_level: 0.1,
            temperature: 20.0,
        }
    }

    mod normal_cases {
        use super::*;

        #[test]
        fn test_virtual_rotation_basic() {
            let req = default_req();
            let result = run_virtual_rotation(&req, &test_config_dir());
            assert!(result.is_ok(), "虚拟旋转应成功");
            let result = result.unwrap();
            assert!(result.pointing_ra.is_finite());
            assert!(result.pointing_dec.is_finite());
            assert!(result.transmission_error > 0.0);
            assert!(result.pointing_error > 0.0);
            assert!(result.error_transfer_coefficient > 0.0);
            assert!(!result.sky_zone.is_empty());
        }

        #[test]
        fn test_visible_stars_near_polaris() {
            let mut req = default_req();
            req.equatorial_angle = 37.95;
            req.elevation_angle = 89.26;
            let result = run_virtual_rotation(&req, &test_config_dir()).unwrap();
            let polaris = result.visible_stars.iter().find(|s| s.name == "北极星");
            assert!(polaris.is_some(), "指向北极星方向应能看到北极星");
            assert!(polaris.unwrap().angular_distance_arcmin < 180.0,
                    "北极星角距离应较近");
        }

        #[test]
        fn test_visible_stars_sorted_by_distance() {
            let req = default_req();
            let result = run_virtual_rotation(&req, &test_config_dir()).unwrap();
            let stars = &result.visible_stars;
            for i in 1..stars.len() {
                assert!(stars[i].angular_distance_arcmin >= stars[i-1].angular_distance_arcmin - 1e-9,
                        "星体应按角距离递增排列");
            }
        }

        #[test]
        fn test_star_names_not_empty() {
            let req = default_req();
            let result = run_virtual_rotation(&req, &test_config_dir()).unwrap();
            for star in &result.visible_stars {
                assert!(!star.name.is_empty(), "星名不应为空");
                assert!(!star.constellation.is_empty(), "星座名不应为空");
            }
        }

        #[test]
        fn test_sky_zone_valid_values() {
            let valid_zones = vec![
                "北天极", "北天区", "赤道带", "黄道带", "南天区", "南天极"
            ];
            let mut req = default_req();
            for &dec in &[-70.0, -30.0, 0.0, 30.0, 70.0] {
                req.elevation_angle = dec;
                let result = run_virtual_rotation(&req, &test_config_dir()).unwrap();
                assert!(valid_zones.iter().any(|z| *z == result.sky_zone),
                        "天区名称应有效: {}", result.sky_zone);
            }
        }
    }

    mod boundary_cases {
        use super::*;

        #[test]
        fn test_zero_azimuth_boundary() {
            let mut req = default_req();
            req.azimuth_angle = 0.0;
            let result = run_virtual_rotation(&req, &test_config_dir());
            assert!(result.is_ok(), "0°方位角应正常");
        }

        #[test]
        fn test_360_azimuth_boundary() {
            let mut req = default_req();
            req.azimuth_angle = 360.0;
            let result = run_virtual_rotation(&req, &test_config_dir());
            assert!(result.is_ok(), "360°方位角应正常");
        }

        #[test]
        fn test_zero_elevation_boundary() {
            let mut req = default_req();
            req.elevation_angle = 0.0;
            let result = run_virtual_rotation(&req, &test_config_dir());
            assert!(result.is_ok(), "0°高度角应正常");
        }

        #[test]
        fn test_90_elevation_boundary() {
            let mut req = default_req();
            req.elevation_angle = 90.0;
            let result = run_virtual_rotation(&req, &test_config_dir());
            assert!(result.is_ok(), "90°高度角应正常");
        }

        #[test]
        fn test_zero_wear_boundary() {
            let mut req = default_req();
            req.wear_level = 0.0;
            let result = run_virtual_rotation(&req, &test_config_dir());
            assert!(result.is_ok(), "零磨损应正常");
            assert!(result.unwrap().transmission_error > 0.0);
        }

        #[test]
        fn test_full_wear_boundary() {
            let mut req = default_req();
            req.wear_level = 1.0;
            let result = run_virtual_rotation(&req, &test_config_dir());
            assert!(result.is_ok(), "100%磨损应正常");
        }

        #[test]
        fn test_negative_temperature() {
            let mut req = default_req();
            req.temperature = -30.0;
            let result = run_virtual_rotation(&req, &test_config_dir());
            assert!(result.is_ok(), "零下温度应正常");
        }

        #[test]
        fn test_high_temperature() {
            let mut req = default_req();
            req.temperature = 60.0;
            let result = run_virtual_rotation(&req, &test_config_dir());
            assert!(result.is_ok(), "高温应正常");
        }

        #[test]
        fn test_pointing_at_north_polar_zone() {
            let mut req = default_req();
            req.equatorial_angle = 0.0;
            req.elevation_angle = 85.0;
            let result = run_virtual_rotation(&req, &test_config_dir()).unwrap();
            assert_eq!(result.sky_zone, "北天极");
        }
    }

    mod error_cases {
        use super::*;

        #[test]
        fn test_invalid_instrument_returns_error() {
            let mut req = default_req();
            req.instrument = "invalid".to_string();
            let result = run_virtual_rotation(&req, &test_config_dir());
            assert!(result.is_err(), "无效仪器应返回错误");
        }

        #[test]
        fn test_extreme_angles_no_panic() {
            let mut req = default_req();
            req.azimuth_angle = 9999.0;
            req.elevation_angle = -9999.0;
            let result = run_virtual_rotation(&req, &test_config_dir());
            assert!(result.is_ok(), "极端角度不应panic");
        }
    }

    mod instrument_comparison {
        use super::*;

        #[test]
        fn test_modern_eq_lower_error() {
            let mut req_ancient = default_req();
            req_ancient.instrument = "hunyi".to_string();
            let ancient = run_virtual_rotation(&req_ancient, &test_config_dir()).unwrap();

            let mut req_modern = default_req();
            req_modern.instrument = "modern_eq".to_string();
            let modern = run_virtual_rotation(&req_modern, &test_config_dir()).unwrap();

            assert!(
                modern.transmission_error < ancient.transmission_error,
                "现代赤道仪传动误差应小于浑仪: 现代={} < 古代={}",
                modern.transmission_error, ancient.transmission_error
            );
        }

        #[test]
        fn test_wear_increases_error() {
            let mut req_low = default_req();
            req_low.wear_level = 0.01;
            let low = run_virtual_rotation(&req_low, &test_config_dir()).unwrap();

            let mut req_high = default_req();
            req_high.wear_level = 0.9;
            let high = run_virtual_rotation(&req_high, &test_config_dir()).unwrap();

            assert!(high.transmission_error > low.transmission_error,
                    "高磨损应导致更高传动误差");
            assert!(high.error_transfer_coefficient >= low.error_transfer_coefficient - 1e-9,
                    "高磨损ETC不低于低磨损");
        }
    }

    mod star_catalog_integrity {
        use super::*;

        #[test]
        fn test_angular_distance_non_negative() {
            let d = angular_distance(0.0, 0.0, 1.0, 1.0);
            assert!(d >= 0.0, "角距离非负");
        }

        #[test]
        fn test_angular_distance_same_point() {
            let d = angular_distance(45.0, 30.0, 45.0, 30.0);
            assert!(d.abs() < 1e-6, "同一点角距离为0: {}", d);
        }

        #[test]
        fn test_angular_distance_symmetric() {
            let d1 = angular_distance(10.0, 20.0, 30.0, 40.0);
            let d2 = angular_distance(30.0, 40.0, 10.0, 20.0);
            assert!((d1 - d2).abs() < 1e-6, "角距离对称");
        }

        #[test]
        fn test_angular_distance_opposite_points() {
            let d = angular_distance(0.0, 0.0, 180.0, 0.0);
            assert!((d - 10800.0).abs() < 100.0, "对径点约180°=10800角分: {}", d);
        }
    }

    mod virtual_operation_intuitiveness {
        use super::*;

        #[test]
        fn test_azimuth_changes_pointing() {
            let mut req1 = default_req();
            req1.azimuth_angle = 0.0;
            let r1 = run_virtual_rotation(&req1, &test_config_dir()).unwrap();

            let mut req2 = default_req();
            req2.azimuth_angle = 90.0;
            let r2 = run_virtual_rotation(&req2, &test_config_dir()).unwrap();

            assert!(r1.pointing_ra != r2.pointing_ra,
                    "方位角变化应导致指向赤经变化");
        }

        #[test]
        fn test_elevation_changes_declination() {
            let mut req1 = default_req();
            req1.elevation_angle = 20.0;
            let r1 = run_virtual_rotation(&req1, &test_config_dir()).unwrap();

            let mut req2 = default_req();
            req2.elevation_angle = 70.0;
            let r2 = run_virtual_rotation(&req2, &test_config_dir()).unwrap();

            assert!(r1.pointing_dec != r2.pointing_dec,
                    "高度角变化应导致指向赤纬变化");
        }

        #[test]
        fn test_all_four_instruments_work() {
            let instruments = vec!["hunyi", "jianyi", "xiangyiyi", "modern_eq"];
            for inst in instruments {
                let mut req = default_req();
                req.instrument = inst.to_string();
                let result = run_virtual_rotation(&req, &test_config_dir());
                assert!(result.is_ok(), "{} 虚拟操作应正常", inst);
                let r = result.unwrap();
                assert!(r.transmission_error > 0.0, "{} 传动误差应>0", inst);
            }
        }
    }
}
