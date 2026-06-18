use crate::clickhouse::ClickHouseClient;
use crate::metrics::POINTING_JOBS_TOTAL;
use crate::models::{GearParamsConfig, PipelineMessage, PointingAccuracyResult, SensorReading};
use crate::models::ShaftParams;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn, debug};

const DEG_TO_ARCMIN: f64 = 60.0;
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;

struct FlexibleAxisParams {
    torsion_stiffness: f64,
    bending_stiffness: f64,
    torsion_nat_freq: f64,
    bending_nat_freq: f64,
    mass_moment: f64,
}

pub struct PointingAnalyzer {
    ch_client: Option<Arc<ClickHouseClient>>,
    cfg: Arc<GearParamsConfig>,
    flex: FlexibleAxisParams,
    rx: Option<mpsc::Receiver<PipelineMessage>>,
    alarm_tx: Option<mpsc::Sender<PipelineMessage>>,
    ws_tx: Option<mpsc::Sender<PipelineMessage>>,
    latitude_deg: f64,
}

impl PointingAnalyzer {
    pub fn new(
        ch_client: Arc<ClickHouseClient>,
        cfg: Arc<GearParamsConfig>,
        rx: mpsc::Receiver<PipelineMessage>,
        alarm_tx: mpsc::Sender<PipelineMessage>,
        ws_tx: mpsc::Sender<PipelineMessage>,
    ) -> Self {
        let flex = Self::compute_flexible_params(&cfg.shaft);
        PointingAnalyzer {
            ch_client: Some(ch_client), cfg, flex, rx: Some(rx), alarm_tx: Some(alarm_tx), ws_tx: Some(ws_tx),
            latitude_deg: 34.25,
        }
    }

    pub fn new_standalone(cfg: GearParamsConfig) -> Self {
        let flex = Self::compute_flexible_params(&cfg.shaft);
        PointingAnalyzer {
            ch_client: None, cfg: Arc::new(cfg), flex, rx: None, alarm_tx: None, ws_tx: None,
            latitude_deg: 34.25,
        }
    }

    fn compute_flexible_params(s: &ShaftParams) -> FlexibleAxisParams {
        let r = s.diameter_m / 2.0;
        let area = std::f64::consts::PI * r.powi(2);
        let i_polar = std::f64::consts::PI * r.powi(4) / 2.0;
        let i_area = std::f64::consts::PI * r.powi(4) / 4.0;

        let torsion_stiffness = s.shear_modulus_pa * i_polar / s.length_m;
        let bending_stiffness = s.young_modulus_pa * i_area / s.length_m.powi(3) * 3.0;

        let shaft_mass = s.density_kgm3 * area * s.length_m;
        let mass_moment_shaft = shaft_mass * (3.0 * r.powi(2) + s.length_m.powi(2)) / 12.0;
        let total_moment = mass_moment_shaft + s.end_mass_moment_kgm2;

        let torsion_nat_freq = (torsion_stiffness / total_moment).sqrt() / (2.0 * std::f64::consts::PI);
        let tip_eq = shaft_mass * 0.24 + s.tip_equiv_mass_kg;
        let bending_nat_freq = (3.0 * s.young_modulus_pa * i_area
            / (tip_eq * s.length_m.powi(3))).sqrt()
            / (2.0 * std::f64::consts::PI);

        FlexibleAxisParams {
            torsion_stiffness, bending_stiffness,
            torsion_nat_freq, bending_nat_freq,
            mass_moment: total_moment,
        }
    }

    pub fn determine_sky_zone(dec: f64) -> String {
        if dec >= 60.0 { "北天极".to_string() }
        else if dec >= 30.0 { "北天区".to_string() }
        else if dec >= -23.5 { if dec.abs() <= 23.5 { "黄道带".to_string() } else { "赤道带".to_string() } }
        else if dec >= -60.0 { "南天区".to_string() }
        else { "南天极".to_string() }
    }

    pub fn atmospheric_refraction(el_deg: f64, t_c: f64) -> f64 {
        if el_deg <= 0.0 { return 0.0; }
        let el = el_deg * std::f64::consts::PI / 180.0;
        (1.02 / el.tan() * (283.0 / (273.0 + t_c))).min(30.0)
    }

    pub fn tube_flexure(el_deg: f64, coeff: f64) -> f64 {
        coeff * (el_deg * std::f64::consts::PI / 180.0).cos()
    }

    fn dmf(r: f64, zeta: f64) -> f64 {
        1.0 / ((1.0 - r.powi(2)).powi(2) + (2.0 * zeta * r).powi(2)).sqrt().max(0.01)
    }

    pub fn compute_etc(&self, az: f64, el: f64, cum_err: f64) -> f64 {
        let az_rad = az * DEG_TO_RAD;
        let el_rad = el * DEG_TO_RAD;
        let az_sens: f64 = 1.0 / el_rad.cos().max(0.01);
        let el_sens: f64 = 1.0;
        let geo = (az_sens.powi(2) + el_sens.powi(2)).sqrt();

        let s = &self.cfg.shaft;
        let op = s.operating_speed_rpm * 2.0 * std::f64::consts::PI / 60.0;
        let w_t = self.flex.torsion_nat_freq * 2.0 * std::f64::consts::PI;
        let w_b = self.flex.bending_nat_freq * 2.0 * std::f64::consts::PI;
        let r_t = op / w_t;
        let r_b = op / w_b;

        let dyn_f = (Self::dmf(r_t, s.modal_damping_ratio).powi(2)
            + Self::dmf(r_b, s.modal_damping_ratio).powi(2) * 0.6).sqrt();
        let coupling = 1.0 + 0.35 * el_rad.sin().powi(2);

        let modes = [1.0, 2.8, 5.3];
        let zetas = [s.modal_damping_ratio, s.modal_damping_ratio * 0.8, s.modal_damping_ratio * 0.6];
        let wts = [0.65, 0.25, 0.10];
        let modal: f64 = (0..3).map(|i| wts[i] * Self::dmf(op / (modes[i] * w_t), zetas[i])).sum();

        let wear_soft = 1.0 + cum_err * 0.08;
        geo * dyn_f * coupling * modal * wear_soft
    }

    pub fn equatorial_to_altaz(ra: f64, dec: f64, lst: f64, lat: f64) -> (f64, f64) {
        let ra_r = ra * DEG_TO_RAD;
        let dec_r = dec * DEG_TO_RAD;
        let lst_r = lst * DEG_TO_RAD;
        let lat_r = lat * DEG_TO_RAD;
        let ha = lst_r - ra_r;

        let sin_el = dec_r.sin() * lat_r.sin() + dec_r.cos() * lat_r.cos() * ha.cos();
        let el = sin_el.asin() / DEG_TO_RAD;

        let cos_az = (dec_r.sin() - sin_el * lat_r.sin())
            / ((1.0 - sin_el.powi(2)).sqrt() * lat_r.cos());
        let sin_az = -dec_r.cos() * ha.sin();
        let mut az = sin_az.atan2(cos_az) / DEG_TO_RAD;
        if az < 0.0 { az += 360.0; }
        (az, el)
    }

    pub fn analyze(&self, reading: &SensorReading) -> PointingAccuracyResult {
        let lst = 12.0 * 15.0;
        let target_ra = reading.theoretical_ra;
        let target_dec = reading.theoretical_dec;
        let m_ra = reading.observed_star_ra;
        let m_dec = reading.observed_star_dec;

        let ra_err = (m_ra - target_ra) * DEG_TO_ARCMIN * (target_dec * DEG_TO_RAD).cos();
        let dec_err = (m_dec - target_dec) * DEG_TO_ARCMIN;
        let total = (ra_err.powi(2) + dec_err.powi(2)).sqrt();

        let (t_az, t_el) = Self::equatorial_to_altaz(target_ra, target_dec, lst, self.latitude_deg);
        let (m_az, m_el) = Self::equatorial_to_altaz(m_ra, m_dec, lst, self.latitude_deg);
        let ez_az = (m_az - t_az) * DEG_TO_ARCMIN;
        let ez_el = (m_el - t_el) * DEG_TO_ARCMIN;

        let transp = reading.cumulative_transmission_error;
        let refr = Self::atmospheric_refraction(t_el, reading.temperature);
        let flex = Self::tube_flexure(t_el, 0.08);
        let coll: f64 = 0.32;

        let theory_prec = ((transp * 0.4).powi(2) + coll.powi(2) + (refr * 0.3).powi(2) + 0.05_f64).sqrt();
        let etc = self.compute_etc(t_az, t_el, transp);

        PointingAccuracyResult {
            timestamp: Utc::now(),
            device_id: reading.device_id.clone(),
            target_ra, target_dec,
            sky_zone: Self::determine_sky_zone(target_dec),
            measured_ra: m_ra, measured_dec: m_dec,
            ra_error: ra_err, dec_error: dec_err,
            total_pointing_error: total,
            error_azimuth_component: ez_az,
            error_elevation_component: ez_el,
            theoretical_precision: theory_prec,
            achieved_precision: total,
            error_transfer_coefficient: etc,
        }
    }

    pub async fn run(mut self) {
        info!("PointingAnalyzer actor started");
        let rx = match self.rx.take() {
            Some(r) => r,
            None => { warn!("PointingAnalyzer has no rx, stopping"); return; }
        };
        tokio::pin!(rx);
        while let Some(msg) = rx.recv().await {
            match msg {
                PipelineMessage::ValidatedReading(reading) => {
                    let r = self.analyze(&reading);
                    if let Some(ch) = &self.ch_client {
                        if let Err(e) = ch.insert_pointing_accuracy(&r).await {
                            error!(error = %e, "Pointing CH insert error");
                        }
                    }
                    POINTING_JOBS_TOTAL
                        .with_label_values(&[&r.sky_zone])
                        .inc();
                    let arc = Arc::new(r);
                    let m1 = PipelineMessage::PointingResult(arc.clone());
                    if let Some(ws) = &self.ws_tx {
                        let _ = ws.send(m1.clone()).await;
                    }
                    if let Some(alarm) = &self.alarm_tx {
                        let _ = alarm.send(m1).await;
                    }
                }
                _ => {}
            }
        }
        warn!("PointingAnalyzer actor stopped");
    }
}

pub fn analyze_pointing(reading: &SensorReading) -> PointingAccuracyResult {
    PointingAccuracyResult {
        timestamp: Utc::now(),
        device_id: reading.device_id.clone(),
        target_ra: reading.theoretical_ra,
        target_dec: reading.theoretical_dec,
        sky_zone: PointingAnalyzer::determine_sky_zone(reading.theoretical_dec),
        measured_ra: reading.observed_star_ra,
        measured_dec: reading.observed_star_dec,
        ra_error: (reading.observed_star_ra - reading.theoretical_ra) * DEG_TO_ARCMIN
            * (reading.theoretical_dec * DEG_TO_RAD).cos(),
        dec_error: (reading.observed_star_dec - reading.theoretical_dec) * DEG_TO_ARCMIN,
        total_pointing_error: (
            ((reading.observed_star_ra - reading.theoretical_ra) * DEG_TO_ARCMIN
                * (reading.theoretical_dec * DEG_TO_RAD).cos()).powi(2)
            + ((reading.observed_star_dec - reading.theoretical_dec) * DEG_TO_ARCMIN).powi(2)
        ).sqrt(),
        error_azimuth_component: 0.0,
        error_elevation_component: 0.0,
        theoretical_precision: 0.5,
        achieved_precision: 0.0,
        error_transfer_coefficient: 1.5,
    }
}
