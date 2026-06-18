use crate::clickhouse::ClickHouseClient;
use crate::metrics::ALARMS_TRIGGERED_TOTAL;
use crate::models::{
    AlarmConfig, AlarmEvent, PipelineMessage, SensorReading, WebSocketMessage
};
use actix::prelude::*;
use chrono::Utc;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn, debug};
use uuid::Uuid;

// ==================== 告警评估器 ====================
pub struct AlarmEvaluator {
    ch_client: Arc<ClickHouseClient>,
    cfg: Arc<AlarmConfig>,
    rx: mpsc::Receiver<PipelineMessage>,
    ws_broadcaster: Addr<WsBroadcastServer>,
    last_alarm_times: Arc<Mutex<HashMap<String, chrono::DateTime<Utc>>>>,
}

impl AlarmEvaluator {
    pub fn new(
        ch_client: Arc<ClickHouseClient>,
        cfg: Arc<AlarmConfig>,
        rx: mpsc::Receiver<PipelineMessage>,
        ws_broadcaster: Addr<WsBroadcastServer>,
    ) -> Self {
        AlarmEvaluator {
            ch_client, cfg, rx, ws_broadcaster,
            last_alarm_times: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn should_trigger(&self, key: &str, interval: i64) -> bool {
        let now = Utc::now();
        let mut last = self.last_alarm_times.lock();
        if let Some(t) = last.get(key) {
            if (now - *t).num_seconds() < interval {
                return false;
            }
        }
        true
    }

    fn mark_triggered(&self, key: &str) {
        self.last_alarm_times.lock().insert(key.to_string(), Utc::now());
    }

    fn check_cumulative(&self, reading: &SensorReading) -> Option<AlarmEvent> {
        let err = reading.cumulative_transmission_error;
        let t = &self.cfg.cumulative_error;
        let key = format!("cumulative_error_{}", reading.device_id);
        let (lv, thr, msg) = if err >= t.alarm_threshold_arcmin {
            if !self.should_trigger(&key, t.debounce_seconds) { return None; }
            (2u8, t.alarm_threshold_arcmin,
             format!("浑仪{}累积传动误差达{:.3}角分，超过告警阈值1角分，请立即检查齿轮系统！",
                     reading.device_id, err))
        } else if err >= t.warning_threshold_arcmin {
            if !self.should_trigger(&key, t.debounce_seconds) { return None; }
            (1u8, t.warning_threshold_arcmin,
             format!("浑仪{}累积传动误差达{:.3}角分，接近告警阈值，请关注齿轮磨损情况。",
                     reading.device_id, err))
        } else { return None; };

        self.mark_triggered(&key);
        Some(AlarmEvent {
            timestamp: Utc::now(),
            device_id: reading.device_id.clone(),
            alarm_id: Uuid::new_v4(),
            alarm_type: "累积误差超限".to_string(),
            alarm_level: lv,
            alarm_message: msg,
            affected_axis: None,
            error_value: err,
            threshold_value: thr,
            is_acknowledged: 0,
            acknowledged_at: None,
        })
    }

    fn check_gear_wear(&self, reading: &SensorReading) -> Vec<AlarmEvent> {
        let t = &self.cfg.gear_wear;
        let mut out = Vec::new();
        let wears = [(reading.gear_wear_level_1, 1u8),
            (reading.gear_wear_level_2, 2u8),
            (reading.gear_wear_level_3, 3u8)];
        for (w, gid) in wears {
            let key = format!("gear_wear_{}_{}", reading.device_id, gid);
            let (lv, thr, msg) = if w >= t.alarm_threshold {
                if !self.should_trigger(&key, t.debounce_seconds) { continue; }
                (3u8, t.alarm_threshold,
                 format!("浑仪{}齿轮组{}磨损程度达{:.1}%，已严重磨损，建议立即停机更换！",
                         reading.device_id, gid, w * 100.0))
            } else if w >= t.warning_threshold {
                if !self.should_trigger(&key, t.debounce_seconds) { continue; }
                (1u8, t.warning_threshold,
                 format!("浑仪{}齿轮组{}磨损程度达{:.1}%，建议安排维护检修。",
                         reading.device_id, gid, w * 100.0))
            } else { continue; };
            self.mark_triggered(&key);
            out.push(AlarmEvent {
                timestamp: Utc::now(),
                device_id: reading.device_id.clone(),
                alarm_id: Uuid::new_v4(),
                alarm_type: "齿轮磨损异常".to_string(),
                alarm_level: lv,
                alarm_message: msg,
                affected_axis: Some(gid),
                error_value: w,
                threshold_value: thr,
                is_acknowledged: 0,
                acknowledged_at: None,
            });
        }
        out
    }

    pub async fn run(mut self) {
        info!("AlarmEvaluator actor started");
        while let Some(msg) = self.rx.recv().await {
            match msg {
                PipelineMessage::ValidatedReading(reading) => {
                    let mut alarms = Vec::new();
                    if let Some(a) = self.check_cumulative(&reading) { alarms.push(a); }
                    alarms.extend(self.check_gear_wear(&reading));

                    for a in alarms {
                        let a_arc = Arc::new(a);
                        if let Err(e) = self.ch_client.insert_alarm(&a_arc).await {
                            error!(error = %e, "Alarm CH insert error");
                        }
                        ALARMS_TRIGGERED_TOTAL
                            .with_label_values(&[&a_arc.alarm_type, &a_arc.alarm_level.to_string().as_str()])
                            .inc();
                        self.ws_broadcaster.do_send(BroadcastAlarm { alarm: (*a_arc).clone() });
                    }
                    self.ws_broadcaster.do_send(BroadcastSensorReading {
                        reading: (*reading).clone()
                    });
                }
                PipelineMessage::TransmissionResult(tr) => {
                    self.ws_broadcaster.do_send(BroadcastTransmissionError {
                        result: (*tr).clone()
                    });
                }
                PipelineMessage::PointingResult(pr) => {
                    self.ws_broadcaster.do_send(BroadcastPointingAccuracy {
                        result: (*pr).clone()
                    });
                }
                _ => {}
            }
        }
        warn!("AlarmEvaluator actor stopped");
    }
}

// ==================== WebSocket 广播服务 ====================
pub struct WsBroadcastServer {
    sessions: HashMap<usize, Recipient<WsMessage>>,
    counter: usize,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct WsMessage(pub String);

#[derive(Message)]
#[rtype(usize)]
pub struct Connect { pub addr: Recipient<WsMessage> }

#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect { pub id: usize }

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastSensorReading { pub reading: SensorReading }

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastTransmissionError { pub result: crate::models::TransmissionErrorResult }

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastPointingAccuracy { pub result: crate::models::PointingAccuracyResult }

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastAlarm { pub alarm: AlarmEvent }

impl WsBroadcastServer {
    pub fn new() -> Self { WsBroadcastServer { sessions: HashMap::new(), counter: 0 } }
    fn broadcast(&self, msg: &str) {
        for (_, addr) in &self.sessions {
            let _ = addr.do_send(WsMessage(msg.to_string()));
        }
    }
}

impl Actor for WsBroadcastServer { type Context = Context<Self>; }

impl Handler<Connect> for WsBroadcastServer {
    type Result = usize;
    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        self.counter += 1;
        self.sessions.insert(self.counter, msg.addr);
        self.counter
    }
}

impl Handler<Disconnect> for WsBroadcastServer {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        self.sessions.remove(&msg.id);
    }
}

fn wrap_ws<M: serde::Serialize>(mt: &str, payload: &M) -> Option<String> {
    let m = WebSocketMessage {
        message_type: mt.to_string(),
        payload: serde_json::to_value(payload).ok()?,
        timestamp: Utc::now(),
    };
    serde_json::to_string(&m).ok()
}

impl Handler<BroadcastSensorReading> for WsBroadcastServer {
    type Result = ();
    fn handle(&mut self, msg: BroadcastSensorReading, _: &mut Context<Self>) {
        if let Some(json) = wrap_ws("sensor_reading", &msg.reading) { self.broadcast(&json); }
    }
}

impl Handler<BroadcastTransmissionError> for WsBroadcastServer {
    type Result = ();
    fn handle(&mut self, msg: BroadcastTransmissionError, _: &mut Context<Self>) {
        if let Some(json) = wrap_ws("transmission_error", &msg.result) { self.broadcast(&json); }
    }
}

impl Handler<BroadcastPointingAccuracy> for WsBroadcastServer {
    type Result = ();
    fn handle(&mut self, msg: BroadcastPointingAccuracy, _: &mut Context<Self>) {
        if let Some(json) = wrap_ws("pointing_accuracy", &msg.result) { self.broadcast(&json); }
    }
}

impl Handler<BroadcastAlarm> for WsBroadcastServer {
    type Result = ();
    fn handle(&mut self, msg: BroadcastAlarm, _: &mut Context<Self>) {
        if let Some(json) = wrap_ws("alarm", &msg.alarm) { self.broadcast(&json); }
    }
}

impl Default for WsBroadcastServer {
    fn default() -> Self { Self::new() }
}

// ==================== WebSocket 会话 ====================
pub struct WsSession {
    pub id: usize,
    pub addr: Addr<WsBroadcastServer>,
}

impl Actor for WsSession {
    type Context = actix_web_actors::ws::WebsocketContext<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        self.addr.send(Connect { addr: addr.recipient() })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res { Ok(id) => act.id = id, _ => ctx.stop() }
                fut::ready(())
            }).wait(ctx);
    }
    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        self.addr.do_send(Disconnect { id: self.id });
        Running::Stop
    }
}

impl Handler<WsMessage> for WsSession {
    type Result = ();
    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) { ctx.text(msg.0); }
}

impl actix::StreamHandler<Result<actix_web_actors::ws::Message, actix_web_actors::ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<actix_web_actors::ws::Message, actix_web_actors::ws::ProtocolError>, ctx: &mut Self::Context) {
        use actix_web_actors::ws::Message as WsM;
        match msg {
            Ok(WsM::Ping(m)) => ctx.pong(&m),
            Ok(WsM::Pong(_)) => (),
            Ok(WsM::Close(r)) => { ctx.close(r); ctx.stop(); }
            Err(_) => ctx.stop(),
            _ => {}
        }
    }
}
