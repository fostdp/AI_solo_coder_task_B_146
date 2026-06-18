const API_BASE = location.protocol + '//' + location.hostname + ':8080/api/v1';

const BRIGHT_STARS = [
  { name: '北极星', ra: 37.95, dec: 89.26, mag: 1.98, con: '小熊座' },
  { name: '织女星', ra: 279.23, dec: 38.78, mag: 0.03, con: '天琴座' },
  { name: '牛郎星', ra: 297.70, dec: 8.87, mag: 0.77, con: '天鹰座' },
  { name: '天津四', ra: 310.36, dec: 45.28, mag: 1.25, con: '天鹅座' },
  { name: '参宿四', ra: 88.79, dec: 7.41, mag: 0.50, con: '猎户座' },
  { name: '参宿七', ra: 78.63, dec: -8.20, mag: 0.13, con: '猎户座' },
  { name: '天狼星', ra: 101.29, dec: -16.72, mag: -1.46, con: '大犬座' },
  { name: '大角星', ra: 213.92, dec: 19.18, mag: -0.05, con: '牧夫座' },
  { name: '五车二', ra: 79.17, dec: 45.99, mag: 0.08, con: '御夫座' },
  { name: '毕宿五', ra: 68.98, dec: 16.51, mag: 0.85, con: '金牛座' },
  { name: '心宿二', ra: 247.35, dec: -26.43, mag: 1.09, con: '天蝎座' },
  { name: '角宿一', ra: 201.30, dec: -11.16, mag: 0.97, con: '室女座' },
  { name: '轩辕十四', ra: 152.09, dec: 11.97, mag: 1.35, con: '狮子座' },
  { name: '南河三', ra: 114.83, dec: 5.22, mag: 0.34, con: '小犬座' },
  { name: '北落师门', ra: 344.41, dec: -29.62, mag: 1.16, con: '南鱼座' },
];

const INSTRUMENTS_VO = [
  { id: 'hunyi', name: '浑仪' },
  { id: 'jianyi', name: '简仪' },
  { id: 'xiangyiyi', name: '象限仪' },
  { id: 'modern_eq', name: '现代赤道仪' },
];

let voState = {
  azimuth: 45,
  elevation: 60,
  equatorial: 30,
  instrument: 'hunyi',
  wear: 0.1,
  dragging: null,
  lastMouse: null,
  sceneAPI: null,
  result: null,
  debounceTimer: null,
};

function initVirtualOperationPanel(sceneAPI) {
  voState.sceneAPI = sceneAPI;
  const container = document.getElementById('vop-content');
  if (!container) return;

  container.innerHTML = `
    <div style="display:flex;gap:6px;margin-bottom:8px;flex-wrap:wrap;">
      <div style="flex:1;min-width:80px;">
        <div style="font-size:10px;color:#8a95b8;">仪器</div>
        <select id="vo-inst" style="width:100%;background:rgba(30,40,80,0.6);border:1px solid rgba(100,150,255,0.2);color:#b4c7ff;border-radius:4px;padding:4px;font-size:12px;">
          ${INSTRUMENTS_VO.map(i => `<option value="${i.id}">${i.name}</option>`).join('')}
        </select>
      </div>
      <div style="flex:1;min-width:60px;">
        <div style="font-size:10px;color:#8a95b8;">磨损</div>
        <input id="vo-wear" type="range" min="0" max="0.95" step="0.05" value="0.1" style="width:100%;accent-color:#4c8bf5;" />
      </div>
    </div>
    <div style="font-size:11px;color:#7eb4ff;margin-bottom:6px;">
      🖱️ 拖拽3D浑仪环或使用下方滑块控制轴系旋转
    </div>
    <div style="display:flex;gap:6px;margin-bottom:10px;flex-wrap:wrap;">
      <div style="flex:1;min-width:80px;">
        <div style="font-size:10px;color:#8a95b8;">方位角</div>
        <input id="vo-az" type="range" min="0" max="360" step="1" value="45" style="width:100%;accent-color:#4c8bf5;" />
        <div style="text-align:center;font-size:11px;color:#b4c7ff;font-family:Consolas,monospace;" id="vo-az-val">45°</div>
      </div>
      <div style="flex:1;min-width:80px;">
        <div style="font-size:10px;color:#8a95b8;">高度角</div>
        <input id="vo-el" type="range" min="5" max="85" step="1" value="60" style="width:100%;accent-color:#4c8bf5;" />
        <div style="text-align:center;font-size:11px;color:#b4c7ff;font-family:Consolas,monospace;" id="vo-el-val">60°</div>
      </div>
      <div style="flex:1;min-width:80px;">
        <div style="font-size:10px;color:#8a95b8;">赤道角</div>
        <input id="vo-eq" type="range" min="0" max="360" step="1" value="30" style="width:100%;accent-color:#4c8bf5;" />
        <div style="text-align:center;font-size:11px;color:#b4c7ff;font-family:Consolas,monospace;" id="vo-eq-val">30°</div>
      </div>
    </div>
    <div id="vo-pointing-info" style="background:rgba(10,14,39,0.8);border-radius:6px;padding:10px;margin-bottom:10px;">
      <div style="font-size:11px;color:#7eb4ff;margin-bottom:6px;">指向信息</div>
      <div style="display:flex;justify-content:space-between;font-size:11px;">
        <span style="color:#8a95b8;">指向赤经</span>
        <span style="color:#b4c7ff;font-family:Consolas,monospace;" id="vo-ra">--</span>
      </div>
      <div style="display:flex;justify-content:space-between;font-size:11px;">
        <span style="color:#8a95b8;">指向赤纬</span>
        <span style="color:#b4c7ff;font-family:Consolas,monospace;" id="vo-dec">--</span>
      </div>
      <div style="display:flex;justify-content:space-between;font-size:11px;">
        <span style="color:#8a95b8;">传动误差</span>
        <span style="color:#b4c7ff;font-family:Consolas,monospace;" id="vo-terr">--</span>
      </div>
      <div style="display:flex;justify-content:space-between;font-size:11px;">
        <span style="color:#8a95b8;">天区</span>
        <span style="color:#fbbf24;" id="vo-zone">--</span>
      </div>
    </div>
    <div style="font-size:11px;color:#7eb4ff;margin-bottom:6px;">附近星体</div>
    <div id="vo-star-list" style="max-height:150px;overflow-y:auto;"></div>
  `;

  ['vo-az', 'vo-el', 'vo-eq'].forEach(id => {
    document.getElementById(id).addEventListener('input', onSliderChange);
  });
  document.getElementById('vo-wear').addEventListener('input', (e) => {
    voState.wear = parseFloat(e.target.value);
    debouncedQuery();
  });
  document.getElementById('vo-inst').addEventListener('change', (e) => {
    voState.instrument = e.target.value;
    debouncedQuery();
  });

  queryVirtualRotation();
}

function onSliderChange() {
  voState.azimuth = parseFloat(document.getElementById('vo-az').value);
  voState.elevation = parseFloat(document.getElementById('vo-el').value);
  voState.equatorial = parseFloat(document.getElementById('vo-eq').value);

  document.getElementById('vo-az-val').textContent = voState.azimuth.toFixed(0) + '°';
  document.getElementById('vo-el-val').textContent = voState.elevation.toFixed(0) + '°';
  document.getElementById('vo-eq-val').textContent = voState.equatorial.toFixed(0) + '°';

  if (voState.sceneAPI && voState.sceneAPI.updateAxes) {
    voState.sceneAPI.updateAxes({
      axis_azimuth_angle: voState.azimuth,
      axis_elevation_angle: voState.elevation,
      axis_equatorial_angle: voState.equatorial,
    });
  }

  debouncedQuery();
}

function debouncedQuery() {
  if (voState.debounceTimer) clearTimeout(voState.debounceTimer);
  voState.debounceTimer = setTimeout(queryVirtualRotation, 300);
}

async function queryVirtualRotation() {
  const body = {
    azimuth_angle: voState.azimuth,
    elevation_angle: voState.elevation,
    equatorial_angle: voState.equatorial,
    instrument: voState.instrument,
    wear_level: voState.wear,
    temperature: 20,
  };

  try {
    const resp = await fetch(API_BASE + '/virtual/rotate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    const json = await resp.json();
    if (json.success) {
      voState.result = json.data;
      updateVODisplay(voState.result);
    }
  } catch (e) {
    console.error('Virtual rotation error:', e);
  }
}

function updateVODisplay(data) {
  document.getElementById('vo-ra').textContent = data.pointing_ra.toFixed(2) + '°';
  document.getElementById('vo-dec').textContent = data.pointing_dec.toFixed(2) + '°';
  document.getElementById('vo-terr').textContent = data.transmission_error.toFixed(3) + "'";
  document.getElementById('vo-zone').textContent = data.sky_zone;

  const starList = document.getElementById('vo-star-list');
  if (data.visible_stars && data.visible_stars.length > 0) {
    starList.innerHTML = data.visible_stars.map(s => {
      const magClass = s.magnitude < 0 ? 'ok' : (s.magnitude < 1 ? '' : 'highlight');
      return `
      <div style="display:flex;justify-content:space-between;padding:3px 0;font-size:11px;border-bottom:1px dashed rgba(100,150,255,0.08);">
        <span style="color:#b4c7ff;">${s.name} <span style="color:#8a95b8;font-size:10px;">${s.constellation}</span></span>
        <span style="color:#8a95b8;font-family:Consolas,monospace;font-size:10px;">${s.angular_distance_arcmin.toFixed(0)}' mag${s.magnitude.toFixed(1)}</span>
      </div>`;
    }).join('');
  } else {
    starList.innerHTML = '<div style="color:#8a95b8;font-size:11px;text-align:center;padding:8px;">该方向无亮星</div>';
  }

  if (voState.sceneAPI && voState.sceneAPI.updatePointing) {
    voState.sceneAPI.updatePointing({
      target_ra: data.pointing_ra,
      target_dec: data.pointing_dec,
      ra_error: data.transmission_error * 0.5,
      dec_error: data.transmission_error * 0.3,
    });
  }
}

export function initVirtualOperation(sceneAPI) {
  initVirtualOperationPanel(sceneAPI);
}
