mod alarm_ws;
mod clickhouse;
mod degradation_sim;
mod dtu_receiver;
mod handlers;
mod instrument_comparison;
mod metrics;
mod models;
mod mqtt_ingest;
mod pointing_analyzer;
mod transmission_simulator;
mod virtual_op;

use actix::Actor;
use actix_cors::Cors;
use actix_web::{http, middleware, web, App, HttpServer, Responder, HttpResponse};
use alarm_ws::{AlarmEvaluator, WsBroadcastServer};
use clickhouse::ClickHouseClient;
use dtu_receiver::DtuReceiver;
use handlers::AppState;
use metrics::encode_metrics;
use models::{AlarmConfig, GearParamsConfig, PipelineChannels};
use pointing_analyzer::PointingAnalyzer;
use std::sync::Arc;
use transmission_simulator::TransmissionSimulator;
use tracing::{info, warn, error, debug};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const CHANNEL_CAPACITY: usize = 1024;

async fn metrics_endpoint() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(encode_metrics())
}

async fn init_tracing() {
    let is_json = std::env::var("LOG_FORMAT").map(|v| v == "json").unwrap_or(false);

    let registry = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,hunyi_backend=debug,tower_http=info")));

    if is_json {
        registry
            .with(fmt::layer().json().flatten_event(true))
            .init();
    } else {
        registry
            .with(fmt::layer()
                .with_target(true)
                .with_level(true)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false))
            .init();
    }

    tracing_log::LogTracer::init().ok();
    debug!("Tracing subsystem initialized");
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    init_tracing().await;

    let config_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");

    let gear_cfg = Arc::new(GearParamsConfig::load_from_file(
        &config_dir.join("gear_params.json"))?);
    let alarm_cfg = Arc::new(AlarmConfig::load_from_file(
        &config_dir.join("alarm_thresholds.json"))?);

    info!(gear_axes = gear_cfg.axes.len(), "配置加载完毕");

    let ch_client = Arc::new(ClickHouseClient::new(
        std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://127.0.0.1:8123".to_string()),
        std::env::var("CLICKHOUSE_USER").unwrap_or_default(),
        std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default(),
        std::env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "hunyi_analysis".to_string()),
    ));
    info!("ClickHouse 客户端已初始化");

    let (transmission_channels, rx_transmission, rx_pointing, rx_alarm) =
        PipelineChannels::new(CHANNEL_CAPACITY);

    let ws_server = WsBroadcastServer::new().start();

    let tx_alarm_ws = transmission_channels.to_alarm_ws.clone();
    let tx_ws_only = transmission_channels.to_alarm_ws.clone();

    let dtu_receiver_arc = Arc::new(DtuReceiver::new(
        ch_client.clone(), gear_cfg.clone(), transmission_channels,
    ));

    // MQTT subscriber
    let mqtt_broker = std::env::var("MQTT_BROKER").ok();
    let mqtt_topic = std::env::var("MQTT_TOPIC").unwrap_or_else(|_| "hunyi/sensor".to_string());
    if let Some(broker) = mqtt_broker {
        let broker_clone = broker.clone();
        let dtu = dtu_receiver_arc.clone();
        let topic = mqtt_topic.clone();
        tokio::spawn(async move {
            mqtt_ingest::run_mqtt_subscriber(&broker_clone, &topic, dtu).await;
        });
        info!(%broker, %mqtt_topic, "MQTT subscriber started");
    } else {
        warn!("MQTT_BROKER not set, MQTT ingest disabled");
    }

    // Transmission Actor
    let transmission = TransmissionSimulator::new(
        ch_client.clone(), gear_cfg.clone(),
        rx_transmission, tx_alarm_ws.clone(), tx_ws_only.clone()
    );
    tokio::spawn(async move { transmission.run().await });

    // Alarm + WS Actor
    let alarm_evaluator = AlarmEvaluator::new(
        ch_client.clone(), alarm_cfg.clone(),
        rx_alarm, ws_server.clone()
    );
    tokio::spawn(async move { alarm_evaluator.run().await });

    // Pointing Actor
    let pointing = PointingAnalyzer::new(
        ch_client.clone(), gear_cfg.clone(),
        rx_pointing, tx_alarm_ws, tx_ws_only
    );
    tokio::spawn(async move { pointing.run().await });

    let app_state = web::Data::new(AppState {
        dtu_receiver: dtu_receiver_arc.clone(),
        ch_client: ch_client.clone(),
        ws_server: ws_server.clone(),
        config_dir: config_dir.clone(),
    });

    let port: u16 = std::env::var("SERVER_PORT")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(8080);
    let metrics_port: u16 = std::env::var("PROMETHEUS_PORT")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(8081);

    info!(port, "浑仪分析引擎启动");
    info!(metrics_port, "Prometheus metrics endpoint");
    info!(%mqtt_topic, "WebSocket端点: ws://localhost:{port}/ws");

    // Metrics server (separate port)
    let metrics_bind = ("0.0.0.0", metrics_port);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("metrics tokio runtime");
        rt.block_on(async {
            let metrics_app = || {
                App::new()
                    .route("/metrics", web::get().to(metrics_endpoint))
            };
            if let Err(e) = HttpServer::new(metrics_app)
                .bind(metrics_bind)
                .expect("metrics bind")
                .run()
                .await
            {
                error!(error = %e, "Metrics server error");
            }
        });
    });

    // Main HTTP server
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST", "OPTIONS"])
            .allowed_headers(vec![
                http::header::AUTHORIZATION,
                http::header::ACCEPT,
                http::header::CONTENT_TYPE,
            ])
            .max_age(3600);

        App::new()
            .app_data(app_state.clone())
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .route("/health", web::get().to(handlers::health_check))
            .route("/metrics", web::get().to(metrics_endpoint))
            .route("/api/v1/sensor/ingest", web::post().to(handlers::ingest_sensor_reading))
            .route("/api/v1/transmission/errors", web::get().to(handlers::query_transmission_errors))
            .route("/api/v1/pointing/accuracy", web::get().to(handlers::query_pointing_accuracy))
            .route("/api/v1/alarms", web::get().to(handlers::query_alarms))
            .route("/api/v1/gear/status", web::get().to(handlers::query_gear_status))
            .route("/api/v1/comparison/transmission", web::post().to(handlers::compare_instruments))
            .route("/api/v1/degradation/simulate", web::post().to(handlers::simulate_degradation))
            .route("/api/v1/virtual/rotate", web::post().to(handlers::virtual_rotate))
            .route("/ws", web::get().to(handlers::ws_handshake))
    })
    .workers(4)
    .bind(("0.0.0.0", port))?
    .run()
    .await?;

    Ok(())
}
