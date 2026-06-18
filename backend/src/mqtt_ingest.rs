use crate::dtu_receiver::DtuReceiver;
use crate::metrics::{MQTT_MESSAGES_RECEIVED, SENSOR_READINGS_RECEIVED};
use crate::models::SensorReading;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn, debug};

pub async fn run_mqtt_subscriber(broker_url: &str, topic: &str, dtu: Arc<DtuReceiver>) {
    let broker_url = broker_url.trim_start_matches("mqtt://");
    let (host, port) = match broker_url.split_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(1883)),
        None => (broker_url.to_string(), 1883),
    };

    let mut mqttoptions = MqttOptions::new(
        format!("hunyi-backend-{}", rand::random::<u64>()),
        host, port,
    );
    mqttoptions.set_keep_alive(Duration::from_secs(30));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 50);

    loop {
        match client.subscribe(topic, QoS::AtLeastOnce).await {
            Ok(_) => {
                info!(topic, "MQTT subscribed");
                break;
            }
            Err(e) => {
                warn!(error = %e, "MQTT subscribe failed, retrying in 3s...");
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }

    loop {
        match eventloop.poll().await {
            Ok(notification) => {
                match notification {
                    Event::Incoming(Packet::Publish(p)) => {
                        let payload = String::from_utf8_lossy(&p.payload);
                        debug!(topic = p.topic, payload_len = payload.len(), "MQTT message received");
                        handle_mqtt_message(&dtu, &payload).await;
                    }
                    Event::Incoming(Packet::ConnAck(_)) => {
                        info!("MQTT connected");
                        if let Err(e) = client.subscribe(topic, QoS::AtLeastOnce).await {
                            warn!(error = %e, "Re-subscribe failed");
                        }
                    }
                    Event::Incoming(Packet::PubAck(_)) => {}
                    Event::Incoming(Packet::PingResp) => {}
                    Event::Incoming(Packet::SubAck(_)) => {
                        info!(topic, "MQTT subscription confirmed");
                    }
                    Event::Incoming(_) => {}
                    Event::Outgoing(_) => {}
                }
            }
            Err(e) => {
                error!(error = %e, "MQTT connection error, reconnecting...");
                MQTT_MESSAGES_RECEIVED
                    .with_label_values(&[topic, "error"])
                    .inc();
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

async fn handle_mqtt_message(dtu: &DtuReceiver, payload: &str) {
    SENSOR_READINGS_RECEIVED.with_label_values(&["mqtt"]).inc();

    match serde_json::from_str::<SensorReading>(payload) {
        Ok(reading) => {
            match dtu.ingest(reading).await {
                Ok(_) => {
                    MQTT_MESSAGES_RECEIVED
                        .with_label_values(&["hunyi/sensor", "ok"])
                        .inc();
                }
                Err(e) => {
                    warn!(error = %e, "MQTT message validation failed");
                    MQTT_MESSAGES_RECEIVED
                        .with_label_values(&["hunyi/sensor", "validation_error"])
                        .inc();
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "MQTT message parsing failed");
            MQTT_MESSAGES_RECEIVED
                .with_label_values(&["hunyi/sensor", "parse_error"])
                .inc();
        }
    }
}
