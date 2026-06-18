const state = {
  currentReading: null,
  currentPointing: null,
  currentTransmission: {},
  alarms: [],
  callbacks: {}
};

function updateClock() {
  document.getElementById('clock').textContent = new Date().toLocaleTimeString('zh-CN');
}

function setConnectedStatus(connected) {
  const dot = document.getElementById('conn-dot');
  const text = document.getElementById('conn-text');
  if (connected) {
    dot.className = 'status-dot connected';
    text.textContent = '已连接';
  } else {
    dot.className = 'status-dot disconnected';
    text.textContent = '未连接';
  }
}

function fmt(num, digits = 3) {
  return (num || 0).toFixed(digits);
}

function updateSensorUI(r) {
  state.currentReading = r;
  document.getElementById('az-val').textContent = fmt(r.axis_azimuth_angle, 2) + '°';
  document.getElementById('el-val').textContent = fmt(r.axis_elevation_angle, 2) + '°';
  document.getElementById('eq-val').textContent = fmt(r.axis_equatorial_angle, 2) + '°';

  const cumEl = document.getElementById('cum-err');
  cumEl.textContent = fmt(r.cumulative_transmission_error) + "'";
  cumEl.className = 'data-value ' + (r.cumulative_transmission_error >= 1 ? 'danger'
    : (r.cumulative_transmission_error >= 0.8 ? 'highlight' : 'ok'));
  document.getElementById('cum-err-bar').style.width =
    Math.min(100, r.cumulative_transmission_error / 1.5 * 100) + '%';

  document.getElementById('gear1-err').textContent = fmt(r.gear_meshing_error_1) + "'";
  document.getElementById('gear2-err').textContent = fmt(r.gear_meshing_error_2) + "'";
  document.getElementById('gear3-err').textContent = fmt(r.gear_meshing_error_3) + "'";

  function updateWear(id, barId, val) {
    const pct = val * 100;
    const el = document.getElementById(id);
    el.textContent = pct.toFixed(1) + '%';
    el.className = 'data-value ' + (val >= 0.85 ? 'danger' : (val >= 0.6 ? 'highlight' : ''));
    document.getElementById(barId).style.width = pct + '%';
  }
  updateWear('wear1', 'wear1-bar', r.gear_wear_level_1);
  updateWear('wear2', 'wear2-bar', r.gear_wear_level_2);
  updateWear('wear3', 'wear3-bar', r.gear_wear_level_3);

  document.getElementById('temp').textContent = fmt(r.temperature, 1) + '°C';
  document.getElementById('humidity').textContent = fmt(r.humidity, 0) + '%';
  document.getElementById('brg1').textContent = fmt(r.bearing_clearance_1) + "'";
  document.getElementById('brg2').textContent = fmt(r.bearing_clearance_2) + "'";
  document.getElementById('brg3').textContent = fmt(r.bearing_clearance_3) + "'";

  if (state.callbacks.onAxesUpdated) state.callbacks.onAxesUpdated(r);
}

function updatePointingUI(p) {
  state.currentPointing = p;
  document.getElementById('target-ra').textContent = fmt(p.target_ra, 2) + '°';
  document.getElementById('target-dec').textContent = fmt(p.target_dec, 2) + '°';
  document.getElementById('ra-err').textContent = (p.ra_error >= 0 ? '+' : '') + fmt(p.ra_error) + "'";
  document.getElementById('dec-err').textContent = (p.dec_error >= 0 ? '+' : '') + fmt(p.dec_error) + "'";
  const totalEl = document.getElementById('total-err');
  totalEl.textContent = fmt(p.total_pointing_error) + "'";
  totalEl.className = 'data-value ' + (p.total_pointing_error >= 1 ? 'danger'
    : (p.total_pointing_error >= 0.5 ? 'highlight' : 'ok'));
  document.getElementById('sky-zone').textContent = p.sky_zone;
  document.getElementById('etc').textContent = fmt(p.error_transfer_coefficient);

  if (state.callbacks.onPointingUpdated) state.callbacks.onPointingUpdated(p);
}

function updateTransmissionUI(t) {
  state.currentTransmission[t.axis_id] = t;
  document.getElementById('backlash-err').textContent = fmt(t.backlash_error) + "'";
  document.getElementById('elastic-err').textContent = fmt(t.elastic_deformation_error) + "'";
}

function addAlarm(alarm) {
  state.alarms.unshift(alarm);
  if (state.alarms.length > 20) state.alarms.pop();
  renderAlarms();
}

function renderAlarms() {
  const container = document.getElementById('alarm-list');
  if (state.alarms.length === 0) {
    container.innerHTML = '<div style="color:#8a95b8;font-size:12px;text-align:center;padding:10px;">暂无告警</div>';
    return;
  }
  container.innerHTML = state.alarms.map(a => `
    <div class="alarm-item level-${a.alarm_level}">
      <div class="alarm-title">${a.alarm_type}</div>
      <div>${a.alarm_message}</div>
      <div class="alarm-time">${new Date(a.timestamp).toLocaleString('zh-CN')}</div>
    </div>
  `).join('');
}

let wsReconnectTimer = null;

function connectWebSocket(wsUrl) {
  console.log('Connecting to', wsUrl);
  const ws = new WebSocket(wsUrl);

  ws.onopen = () => {
    setConnectedStatus(true);
    console.log('WebSocket connected');
    if (wsReconnectTimer) { clearTimeout(wsReconnectTimer); wsReconnectTimer = null; }
  };

  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data);
      switch (msg.message_type) {
        case 'sensor_reading': updateSensorUI(msg.payload); break;
        case 'pointing_accuracy': updatePointingUI(msg.payload); break;
        case 'transmission_error': updateTransmissionUI(msg.payload); break;
        case 'alarm': addAlarm(msg.payload); break;
      }
    } catch (e) {
      console.error('Parse WS message error:', e);
    }
  };

  ws.onclose = () => {
    setConnectedStatus(false);
    console.log('WebSocket disconnected, retrying in 3s...');
    wsReconnectTimer = setTimeout(() => connectWebSocket(wsUrl), 3000);
  };

  ws.onerror = (e) => {
    console.error('WebSocket error:', e);
    try { ws.close(); } catch (_) {}
  };
}

export function initPanel(wsUrl, callbacks = {}) {
  state.callbacks = callbacks || {};

  setInterval(updateClock, 1000);
  updateClock();

  document.getElementById('btn-rotate').addEventListener('click', (e) => {
    if (state.callbacks.onToggleAutoRotate) {
      const v = !state.callbacks.onToggleAutoRotate();
      if (typeof v === 'boolean') e.target.classList.toggle('active', v);
      else e.target.classList.toggle('active');
    }
  });
  document.getElementById('btn-gears').addEventListener('click', (e) => {
    if (state.callbacks.onToggleGears) {
      const v = !state.callbacks.onToggleGears();
      if (typeof v === 'boolean') e.target.classList.toggle('active', v);
      else e.target.classList.toggle('active');
    }
  });
  document.getElementById('btn-error').addEventListener('click', (e) => {
    if (state.callbacks.onToggleError) {
      const v = !state.callbacks.onToggleError();
      if (typeof v === 'boolean') e.target.classList.toggle('active', v);
      else e.target.classList.toggle('active');
    }
  });
  document.getElementById('btn-reset').addEventListener('click', () => {
    if (state.callbacks.onResetView) state.callbacks.onResetView();
  });

  const proto = (location.protocol === 'https:') ? 'wss' : 'ws';
  const url = wsUrl || `${proto}://${location.hostname}:8080/ws`;
  connectWebSocket(url);
}
