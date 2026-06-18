const API_BASE = location.protocol + '//' + location.hostname + ':8080/api/v1';

const INSTRUMENTS = [
    { id: 'hunyi', name: '浑仪', era: '古代', desc: '宋代三层环结构' },
    { id: 'jianyi', name: '简仪', era: '古代', desc: '元代郭守敬创制' },
    { id: 'xiangyiyi', name: '象限仪', era: '古代', desc: '明清高度测量仪' },
    { id: 'modern_eq', name: '现代赤道仪', era: '现代', desc: '精密蜗轮蜗杆驱动' },
];

const INSTRUMENT_COLORS = {
    hunyi: '#4c8bf5',
    jianyi: '#34d399',
    xiangyiyi: '#fbbf24',
    modern_eq: '#f87171',
};

let compResult = null;
let degrResult = null;

function initComparisonPanel() {
    const container = document.getElementById('comparison-content');
    if (!container) return;

    container.innerHTML = `
    <div style="margin-bottom:12px;">
      <div style="font-size:12px;color:#8a95b8;margin-bottom:6px;">选择对比仪器</div>
      <div style="display:flex;flex-wrap:wrap;gap:6px;">
        ${INSTRUMENTS.map(inst => `
          <label style="display:flex;align-items:center;gap:4px;cursor:pointer;font-size:12px;padding:4px 8px;background:rgba(50,70,130,0.25);border-radius:4px;">
            <input type="checkbox" class="comp-inst-cb" value="${inst.id}" ${inst.id === 'hunyi' || inst.id === 'modern_eq' ? 'checked' : ''} />
            <span style="color:${INSTRUMENT_COLORS[inst.id]}">${inst.name}</span>
            <span style="color:#8a95b8;font-size:10px">(${inst.era})</span>
          </label>
        `).join('')}
      </div>
    </div>
    <div style="display:flex;gap:6px;margin-bottom:10px;flex-wrap:wrap;">
      <div style="flex:1;min-width:60px;">
        <div style="font-size:10px;color:#8a95b8;">方位角</div>
        <input id="comp-az" type="number" value="45" step="5" style="width:100%;background:rgba(30,40,80,0.6);border:1px solid rgba(100,150,255,0.2);color:#b4c7ff;border-radius:4px;padding:4px;font-size:12px;" />
      </div>
      <div style="flex:1;min-width:60px;">
        <div style="font-size:10px;color:#8a95b8;">高度角</div>
        <input id="comp-el" type="number" value="60" step="5" style="width:100%;background:rgba(30,40,80,0.6);border:1px solid rgba(100,150,255,0.2);color:#b4c7ff;border-radius:4px;padding:4px;font-size:12px;" />
      </div>
      <div style="flex:1;min-width:60px;">
        <div style="font-size:10px;color:#8a95b8;">磨损</div>
        <input id="comp-wear" type="number" value="0.1" step="0.05" min="0" max="0.99" style="width:100%;background:rgba(30,40,80,0.6);border:1px solid rgba(100,150,255,0.2);color:#b4c7ff;border-radius:4px;padding:4px;font-size:12px;" />
      </div>
    </div>
    <button id="btn-run-comparison" style="width:100%;padding:8px;background:linear-gradient(90deg,#4c8bf5,#7eb4ff);color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:13px;margin-bottom:10px;">
      运行对比分析
    </button>
    <div id="comp-bar-chart" style="min-height:120px;"></div>
    <div id="comp-detail-table" style="margin-top:10px;"></div>
  `;

  document.getElementById('btn-run-comparison').addEventListener('click', runComparison);
}

async function runComparison() {
  const checked = [...document.querySelectorAll('.comp-inst-cb:checked')].map(el => el.value);
  if (checked.length < 2) { alert('请至少选择2种仪器'); return; }

  const body = {
    instruments: checked,
    azimuth_angle: parseFloat(document.getElementById('comp-az').value) || 45,
    elevation_angle: parseFloat(document.getElementById('comp-el').value) || 60,
    equatorial_angle: 30,
    temperature: 20,
    wear_level: parseFloat(document.getElementById('comp-wear').value) || 0.1,
  };

  const btn = document.getElementById('btn-run-comparison');
  btn.textContent = '分析中...';
  btn.disabled = true;

  try {
    const resp = await fetch(API_BASE + '/comparison/transmission', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    const json = await resp.json();
    if (json.success) {
      compResult = json.data;
      renderComparisonBars(compResult.results);
      renderComparisonTable(compResult.results);
    }
  } catch (e) {
    console.error('Comparison error:', e);
  } finally {
    btn.textContent = '运行对比分析';
    btn.disabled = false;
  }
}

function renderComparisonBars(results) {
  const container = document.getElementById('comp-bar-chart');
  if (!results || !results.length) { container.innerHTML = ''; return; }

  const maxErr = Math.max(...results.map(r => r.cumulative_error), 0.01);
  let html = '<div style="font-size:11px;color:#7eb4ff;margin-bottom:6px;">累积传动误差对比 (角分)</div>';

  results.forEach(r => {
    const pct = (r.cumulative_error / maxErr * 100).toFixed(1);
    const color = INSTRUMENT_COLORS[r.instrument_type] || '#7eb4ff';
    html += `
    <div style="margin-bottom:6px;">
      <div style="display:flex;justify-content:space-between;font-size:11px;">
        <span style="color:${color}">${r.instrument_name} <span style="color:#8a95b8;font-size:10px">(${r.era})</span></span>
        <span style="color:#b4c7ff;font-family:Consolas,monospace;">${r.cumulative_error.toFixed(3)}'</span>
      </div>
      <div style="width:100%;height:8px;background:rgba(50,70,130,0.4);border-radius:4px;overflow:hidden;">
        <div style="width:${pct}%;height:100%;background:${color};border-radius:4px;transition:width 0.5s;"></div>
      </div>
    </div>`;
  });
  container.innerHTML = html;
}

function renderComparisonTable(results) {
  const container = document.getElementById('comp-detail-table');
  if (!results || !results.length) { container.innerHTML = ''; return; }

  const metrics = [
    { key: 'avg_backlash', label: '平均齿隙' },
    { key: 'avg_elastic', label: '平均弹性' },
    { key: 'avg_wear_error', label: '平均磨损' },
    { key: 'avg_temp_effect', label: '温度效应' },
    { key: 'max_single_axis_error', label: '最大单轴误差' },
  ];

  let html = '<div style="font-size:11px;color:#7eb4ff;margin-bottom:6px;">误差分量明细 (角分)</div>';
  html += '<table style="width:100%;font-size:11px;border-collapse:collapse;">';
  html += '<tr style="border-bottom:1px solid rgba(100,150,255,0.15);"><td style="color:#8a95b8;padding:3px;"></td>';
  results.forEach(r => {
    html += `<td style="color:${INSTRUMENT_COLORS[r.instrument_type] || '#b4c7ff'};padding:3px;text-align:right;">${r.instrument_name}</td>`;
  });
  html += '</tr>';

  metrics.forEach(m => {
    html += `<tr style="border-bottom:1px dashed rgba(100,150,255,0.08);"><td style="color:#8a95b8;padding:3px;">${m.label}</td>`;
    results.forEach(r => {
      const val = r[m.key];
      html += `<td style="color:#b4c7ff;padding:3px;text-align:right;font-family:Consolas,monospace;">${(val || 0).toFixed(3)}'</td>`;
    });
    html += '</tr>';
  });
  html += '</table>';
  container.innerHTML = html;
}

function initDegradationPanel() {
  const container = document.getElementById('degradation-content');
  if (!container) return;

  container.innerHTML = `
    <div style="display:flex;gap:6px;margin-bottom:8px;flex-wrap:wrap;">
      <div style="flex:1;min-width:70px;">
        <div style="font-size:10px;color:#8a95b8;">仪器</div>
        <select id="degr-inst" style="width:100%;background:rgba(30,40,80,0.6);border:1px solid rgba(100,150,255,0.2);color:#b4c7ff;border-radius:4px;padding:4px;font-size:12px;">
          ${INSTRUMENTS.map(i => `<option value="${i.id}">${i.name} (${i.era})</option>`).join('')}
        </select>
      </div>
      <div style="flex:1;min-width:60px;">
        <div style="font-size:10px;color:#8a95b8;">运行时长(h)</div>
        <input id="degr-hours" type="number" value="10000" step="1000" style="width:100%;background:rgba(30,40,80,0.6);border:1px solid rgba(100,150,255,0.2);color:#b4c7ff;border-radius:4px;padding:4px;font-size:12px;" />
      </div>
    </div>
    <div style="display:flex;gap:6px;margin-bottom:10px;flex-wrap:wrap;">
      <div style="flex:1;min-width:60px;">
        <div style="font-size:10px;color:#8a95b8;">初始磨损</div>
        <input id="degr-init-wear" type="number" value="0.05" step="0.05" min="0" max="0.9" style="width:100%;background:rgba(30,40,80,0.6);border:1px solid rgba(100,150,255,0.2);color:#b4c7ff;border-radius:4px;padding:4px;font-size:12px;" />
      </div>
      <div style="flex:1;min-width:60px;">
        <div style="font-size:10px;color:#8a95b8;">磨损速率</div>
        <input id="degr-rate" type="number" value="1.0" step="0.5" style="width:100%;background:rgba(30,40,80,0.6);border:1px solid rgba(100,150,255,0.2);color:#b4c7ff;border-radius:4px;padding:4px;font-size:12px;" />
      </div>
      <div style="flex:1;min-width:60px;">
        <div style="font-size:10px;color:#8a95b8;">采样步数</div>
        <input id="degr-steps" type="number" value="50" step="10" style="width:100%;background:rgba(30,40,80,0.6);border:1px solid rgba(100,150,255,0.2);color:#b4c7ff;border-radius:4px;padding:4px;font-size:12px;" />
      </div>
    </div>
    <button id="btn-run-degradation" style="width:100%;padding:8px;background:linear-gradient(90deg,#f59e0b,#fbbf24);color:#1a1f3a;border:none;border-radius:6px;cursor:pointer;font-size:13px;margin-bottom:10px;font-weight:bold;">
      运行退化仿真
    </button>
    <canvas id="degr-canvas" width="380" height="180" style="width:100%;border-radius:6px;background:rgba(10,14,39,0.8);"></canvas>
  `;

  document.getElementById('btn-run-degradation').addEventListener('click', runDegradation);
}

async function runDegradation() {
  const body = {
    instrument: document.getElementById('degr-inst').value,
    total_hours: parseInt(document.getElementById('degr-hours').value) || 10000,
    steps: parseInt(document.getElementById('degr-steps').value) || 50,
    initial_wear: parseFloat(document.getElementById('degr-init-wear').value) || 0.05,
    wear_rate: parseFloat(document.getElementById('degr-rate').value) || 1.0,
    temperature: 20,
    azimuth_angle: 45,
    elevation_angle: 60,
  };

  const btn = document.getElementById('btn-run-degradation');
  btn.textContent = '仿真中...';
  btn.disabled = true;

  try {
    const resp = await fetch(API_BASE + '/degradation/simulate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    const json = await resp.json();
    if (json.success) {
      degrResult = json.data;
      drawDegradationChart(degrResult);
    }
  } catch (e) {
    console.error('Degradation error:', e);
  } finally {
    btn.textContent = '运行退化仿真';
    btn.disabled = false;
  }
}

function drawDegradationChart(data) {
  const canvas = document.getElementById('degr-canvas');
  if (!canvas || !data || !data.data_points || !data.data_points.length) return;

  const ctx = canvas.getContext('2d');
  const W = canvas.width, H = canvas.height;
  const pad = { l: 50, r: 15, t: 15, b: 30 };
  const pw = W - pad.l - pad.r, ph = H - pad.t - pad.b;

  ctx.clearRect(0, 0, W, H);

  const pts = data.data_points;
  const maxH = pts[pts.length - 1].elapsed_hours;
  const maxErr = Math.max(...pts.map(p => p.cumulative_error), 0.01);
  const maxWear = Math.max(...pts.map(p => p.wear_level), 0.01);
  const maxPE = Math.max(...pts.map(p => p.total_pointing_error), 0.01);

  ctx.strokeStyle = 'rgba(100,150,255,0.15)';
  ctx.lineWidth = 0.5;
  for (let i = 0; i <= 4; i++) {
    const y = pad.t + ph * i / 4;
    ctx.beginPath(); ctx.moveTo(pad.l, y); ctx.lineTo(pad.l + pw, y); ctx.stroke();
  }

  ctx.fillStyle = '#8a95b8';
  ctx.font = '9px sans-serif';
  ctx.textAlign = 'center';
  for (let i = 0; i <= 4; i++) {
    const x = pad.l + pw * i / 4;
    const val = (maxH * i / 4).toFixed(0);
    ctx.fillText(val + 'h', x, H - 5);
  }

  function drawLine(points, valueKey, maxVal, color, yLabel) {
    ctx.beginPath();
    ctx.strokeStyle = color;
    ctx.lineWidth = 2;
    points.forEach((p, i) => {
      const x = pad.l + (p.elapsed_hours / maxH) * pw;
      const y = pad.t + ph - (p[valueKey] / maxVal) * ph;
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    });
    ctx.stroke();
  }

  drawLine(pts, 'cumulative_error', maxErr, '#4c8bf5', '累积误差');
  drawLine(pts, 'wear_level', maxWear, '#34d399', '磨损');
  drawLine(pts, 'total_pointing_error', maxPE, '#f87171', '指向误差');

  const legend = [
    { color: '#4c8bf5', label: '累积误差' },
    { color: '#34d399', label: '磨损等级' },
    { color: '#f87171', label: '指向误差' },
  ];
  ctx.font = '10px sans-serif';
  legend.forEach((l, i) => {
    const x = pad.l + i * 110;
    const y = pad.t + 10;
    ctx.fillStyle = l.color;
    ctx.fillRect(x, y - 6, 12, 3);
    ctx.fillStyle = '#b4c7ff';
    ctx.textAlign = 'left';
    ctx.fillText(l.label, x + 16, y);
  });
}

export function initComparisonAndDegradation() {
  initComparisonPanel();
  initDegradationPanel();
}
