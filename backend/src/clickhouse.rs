use crate::models::{AlarmEvent, GearStatus, PointingAccuracyResult, SensorReading, TransmissionErrorResult};
use reqwest::Client;
use serde_json::Value;
use std::error::Error;

pub struct ClickHouseClient {
    base_url: String,
    database: String,
    user: String,
    password: String,
    client: Client,
}

impl ClickHouseClient {
    pub fn new(base_url: impl Into<String>, user: impl Into<String>,
               password: impl Into<String>, database: impl Into<String>) -> Self {
        ClickHouseClient {
            base_url: base_url.into(),
            database: database.into(),
            user: user.into(),
            password: password.into(),
            client: Client::new(),
        }
    }

    async fn execute_query(&self, query: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let url = format!("{}/?database={}", self.base_url, self.database);
        let mut req = self.client.post(&url).body(query.to_string());
        if !self.user.is_empty() {
            req = req.basic_auth(&self.user, Some(&self.password));
        }
        let resp = req.send().await?;
        Ok(resp.text().await?)
    }

    pub async fn insert_sensor_reading(&self, r: &SensorReading) -> Result<(), Box<dyn Error + Send + Sync>> {
        let sql = format!(
            "INSERT INTO sensor_readings FORMAT JSONEachRow {}",
            serde_json::to_string(r)?
        );
        self.execute_query(&sql).await?;
        Ok(())
    }

    pub async fn insert_transmission_error(&self, r: &TransmissionErrorResult) -> Result<(), Box<dyn Error + Send + Sync>> {
        let sql = format!(
            "INSERT INTO transmission_error_analysis FORMAT JSONEachRow {}",
            serde_json::to_string(r)?
        );
        self.execute_query(&sql).await?;
        Ok(())
    }

    pub async fn insert_pointing_accuracy(&self, r: &PointingAccuracyResult) -> Result<(), Box<dyn Error + Send + Sync>> {
        let sql = format!(
            "INSERT INTO pointing_accuracy_analysis FORMAT JSONEachRow {}",
            serde_json::to_string(r)?
        );
        self.execute_query(&sql).await?;
        Ok(())
    }

    pub async fn insert_alarm(&self, a: &AlarmEvent) -> Result<(), Box<dyn Error + Send + Sync>> {
        let sql = format!(
            "INSERT INTO alarm_events FORMAT JSONEachRow {}",
            serde_json::to_string(a)?
        );
        self.execute_query(&sql).await?;
        Ok(())
    }

    pub async fn insert_gear_status(&self, g: &GearStatus) -> Result<(), Box<dyn Error + Send + Sync>> {
        let sql = format!(
            "INSERT INTO gear_status FORMAT JSONEachRow {}",
            serde_json::to_string(g)?
        );
        self.execute_query(&sql).await?;
        Ok(())
    }

    pub async fn query_recent_sensor_readings(&self, device_id: &str, limit: u32) -> Result<Vec<SensorReading>, Box<dyn Error + Send + Sync>> {
        let sql = format!(
            "SELECT * FROM sensor_readings WHERE device_id = '{}' ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
            device_id, limit
        );
        let resp = self.execute_query(&sql).await?;
        let parsed: Value = serde_json::from_str(&resp)?;
        let empty = Vec::new();
        let data = parsed["data"].as_array().unwrap_or(&empty);
        let mut results = Vec::new();
        for item in data {
            if let Ok(r) = serde_json::from_value::<SensorReading>(item.clone()) {
                results.push(r);
            }
        }
        Ok(results)
    }

    pub async fn query_recent_alarms(&self, limit: u32) -> Result<Vec<AlarmEvent>, Box<dyn Error + Send + Sync>> {
        let sql = format!(
            "SELECT * FROM alarm_events ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
            limit
        );
        let resp = self.execute_query(&sql).await?;
        let parsed: Value = serde_json::from_str(&resp)?;
        let empty = Vec::new();
        let data = parsed["data"].as_array().unwrap_or(&empty);
        let mut results = Vec::new();
        for item in data {
            if let Ok(a) = serde_json::from_value::<AlarmEvent>(item.clone()) {
                results.push(a);
            }
        }
        Ok(results)
    }

    pub async fn query_pointing_stats_by_zone(&self, device_id: &str, hours: u32) -> Result<Value, Box<dyn Error + Send + Sync>> {
        let sql = format!(
            "SELECT sky_zone, count() as cnt, avg(total_pointing_error) as avg_err, \
             max(total_pointing_error) as max_err, avg(ra_error) as avg_ra, avg(dec_error) as avg_dec \
             FROM pointing_accuracy_analysis \
             WHERE device_id = '{}' AND timestamp >= now() - INTERVAL {} HOUR \
             GROUP BY sky_zone ORDER BY sky_zone FORMAT JSON",
            device_id, hours
        );
        let resp = self.execute_query(&sql).await?;
        Ok(serde_json::from_str(&resp)?)
    }

    pub async fn query_transmission_errors(&self, device_id: &str, limit: usize)
        -> Result<Vec<TransmissionErrorResult>, Box<dyn Error + Send + Sync>> {
        let dev_filter = if device_id.is_empty() { String::new() }
            else { format!("WHERE device_id = '{}' ", device_id) };
        let sql = format!(
            "SELECT * FROM transmission_error_analysis {}ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
            dev_filter, limit
        );
        let resp = self.execute_query(&sql).await?;
        let parsed: Value = serde_json::from_str(&resp)?;
        let empty = Vec::new();
        let data = parsed["data"].as_array().unwrap_or(&empty);
        let mut out = Vec::new();
        for item in data {
            if let Ok(r) = serde_json::from_value::<TransmissionErrorResult>(item.clone()) {
                out.push(r);
            }
        }
        Ok(out)
    }

    pub async fn query_pointing_accuracy(&self, device_id: &str, limit: usize)
        -> Result<Vec<PointingAccuracyResult>, Box<dyn Error + Send + Sync>> {
        let dev_filter = if device_id.is_empty() { String::new() }
            else { format!("WHERE device_id = '{}' ", device_id) };
        let sql = format!(
            "SELECT * FROM pointing_accuracy_analysis {}ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
            dev_filter, limit
        );
        let resp = self.execute_query(&sql).await?;
        let parsed: Value = serde_json::from_str(&resp)?;
        let empty = Vec::new();
        let data = parsed["data"].as_array().unwrap_or(&empty);
        let mut out = Vec::new();
        for item in data {
            if let Ok(r) = serde_json::from_value::<PointingAccuracyResult>(item.clone()) {
                out.push(r);
            }
        }
        Ok(out)
    }

    pub async fn query_alarms(&self, device_id: &str, limit: usize, acknowledged: i8)
        -> Result<Vec<AlarmEvent>, Box<dyn Error + Send + Sync>> {
        let mut where_clauses = Vec::new();
        if !device_id.is_empty() { where_clauses.push(format!("device_id = '{}'", device_id)); }
        if acknowledged >= 0 { where_clauses.push(format!("is_acknowledged = {}", acknowledged)); }
        let w = if where_clauses.is_empty() { String::new() }
            else { format!("WHERE {} ", where_clauses.join(" AND ")) };
        let sql = format!(
            "SELECT * FROM alarm_events {}ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
            w, limit
        );
        let resp = self.execute_query(&sql).await?;
        let parsed: Value = serde_json::from_str(&resp)?;
        let empty = Vec::new();
        let data = parsed["data"].as_array().unwrap_or(&empty);
        let mut out = Vec::new();
        for item in data {
            if let Ok(r) = serde_json::from_value::<AlarmEvent>(item.clone()) {
                out.push(r);
            }
        }
        Ok(out)
    }

    pub async fn query_gear_status(&self, device_id: &str)
        -> Result<Vec<GearStatus>, Box<dyn Error + Send + Sync>> {
        let dev_filter = if device_id.is_empty() { String::new() }
            else { format!("WHERE device_id = '{}' ", device_id) };
        let sql = format!(
            "SELECT * FROM gear_status {}ORDER BY timestamp DESC LIMIT 100 FORMAT JSON", dev_filter
        );
        let resp = self.execute_query(&sql).await?;
        let parsed: Value = serde_json::from_str(&resp)?;
        let empty = Vec::new();
        let data = parsed["data"].as_array().unwrap_or(&empty);
        let mut out = Vec::new();
        for item in data {
            if let Ok(r) = serde_json::from_value::<GearStatus>(item.clone()) {
                out.push(r);
            }
        }
        Ok(out)
    }
}
