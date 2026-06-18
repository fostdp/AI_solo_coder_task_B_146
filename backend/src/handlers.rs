use crate::alarm_ws::{WsBroadcastServer, WsSession};
use crate::clickhouse::ClickHouseClient;
use crate::dtu_receiver::DtuReceiver;
use crate::models::{ApiResponse, HunyiError, SensorReading};
use actix_web::{web, Error, HttpResponse, Responder, HttpRequest};
use actix_web_actors::ws;
use std::sync::Arc;

pub struct AppState {
    pub dtu_receiver: Arc<DtuReceiver>,
    pub ch_client: Arc<ClickHouseClient>,
    pub ws_server: actix::Addr<WsBroadcastServer>,
}

pub async fn ingest_sensor_reading(
    payload: web::Json<SensorReading>,
    data: web::Data<AppState>,
) -> Result<impl Responder, HunyiError> {
    let reading: SensorReading = payload.into_inner();
    let r = data.dtu_receiver.ingest(reading).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(r.as_ref())))
}

pub async fn query_transmission_errors(
    params: web::Query<std::collections::HashMap<String, String>>,
    data: web::Data<AppState>,
) -> Result<impl Responder, HunyiError> {
    let device_id = params.get("device_id").cloned().unwrap_or_default();
    let limit = params.get("limit")
        .and_then(|s| s.parse::<usize>().ok()).unwrap_or(100);
    let r = data.ch_client.query_transmission_errors(&device_id, limit).await
        .map_err(|e| HunyiError::ClickHouse(e.to_string()))?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(&r)))
}

pub async fn query_pointing_accuracy(
    params: web::Query<std::collections::HashMap<String, String>>,
    data: web::Data<AppState>,
) -> Result<impl Responder, HunyiError> {
    let device_id = params.get("device_id").cloned().unwrap_or_default();
    let limit = params.get("limit")
        .and_then(|s| s.parse::<usize>().ok()).unwrap_or(100);
    let r = data.ch_client.query_pointing_accuracy(&device_id, limit).await
        .map_err(|e| HunyiError::ClickHouse(e.to_string()))?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(&r)))
}

pub async fn query_alarms(
    params: web::Query<std::collections::HashMap<String, String>>,
    data: web::Data<AppState>,
) -> Result<impl Responder, HunyiError> {
    let device_id = params.get("device_id").cloned().unwrap_or_default();
    let limit = params.get("limit")
        .and_then(|s| s.parse::<usize>().ok()).unwrap_or(100);
    let ack = params.get("acknowledged").and_then(|s| s.parse::<i8>().ok()).unwrap_or(-1);
    let r = data.ch_client.query_alarms(&device_id, limit, ack).await
        .map_err(|e| HunyiError::ClickHouse(e.to_string()))?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(&r)))
}

pub async fn query_gear_status(
    params: web::Query<std::collections::HashMap<String, String>>,
    data: web::Data<AppState>,
) -> Result<impl Responder, HunyiError> {
    let device_id = params.get("device_id").cloned().unwrap_or_default();
    let r = data.ch_client.query_gear_status(&device_id).await
        .map_err(|e| HunyiError::ClickHouse(e.to_string()))?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(&r)))
}

pub async fn ws_handshake(
    req: HttpRequest,
    stream: web::Payload,
    data: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let resp = ws::start(
        WsSession { id: 0, addr: data.ws_server.clone() },
        &req, stream,
    );
    resp
}

pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status":"healthy","service":"hunyi-analysis-engine"}))
}
