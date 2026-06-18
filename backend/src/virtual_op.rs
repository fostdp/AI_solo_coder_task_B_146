use crate::models::{
    HunyiError, VirtualRotationRequest,
    VirtualRotationResponse, VisibleStar,
};
use crate::pointing_analyzer::PointingAnalyzer;
use crate::transmission_simulator::TransmissionSimulator;
use chrono::Utc;
use std::path::PathBuf;

const DEG_TO_ARCMIN: f64 = 60.0;
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;

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
    }

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
    })
}
