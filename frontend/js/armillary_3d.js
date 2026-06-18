import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

export function createArmillaryScene(container, configUrl = './config/visualization.json') {
  const scene = new THREE.Scene();
  scene.background = null;
  scene.fog = new THREE.FogExp2(0x0a0e27, 0.015);

  const camera = new THREE.PerspectiveCamera(
    45, container.clientWidth / container.clientHeight, 0.1, 1000);
  camera.position.set(5, 3.5, 7);

  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setSize(container.clientWidth, container.clientHeight);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  container.appendChild(renderer.domElement);

  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.dampingFactor = 0.05;
  controls.minDistance = 3;
  controls.maxDistance = 25;

  const ambient = new THREE.AmbientLight(0x4a6fff, 0.5);
  scene.add(ambient);
  const dirLight = new THREE.DirectionalLight(0xffffff, 0.8);
  dirLight.position.set(5, 10, 5);
  dirLight.castShadow = true;
  scene.add(dirLight);
  const pointLight1 = new THREE.PointLight(0x4c8bf5, 1, 20);
  pointLight1.position.set(-5, 3, 5);
  scene.add(pointLight1);
  const pointLight2 = new THREE.PointLight(0xa78bfa, 0.6, 15);
  pointLight2.position.set(5, -2, -5);
  scene.add(pointLight2);

  function createStarField() {
    const stars = new THREE.BufferGeometry();
    const starCount = 3000;
    const positions = new Float32Array(starCount * 3);
    const colors = new Float32Array(starCount * 3);
    for (let i = 0; i < starCount; i++) {
      const r = 80 + Math.random() * 40;
      const theta = Math.random() * Math.PI * 2;
      const phi = Math.acos(2 * Math.random() - 1);
      positions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
      positions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
      positions[i * 3 + 2] = r * Math.cos(phi);
      const temp = Math.random();
      if (temp < 0.33) { colors[i * 3] = 0.8; colors[i * 3 + 1] = 0.9; colors[i * 3 + 2] = 1.0; }
      else if (temp < 0.66) { colors[i * 3] = 1.0; colors[i * 3 + 1] = 1.0; colors[i * 3 + 2] = 0.95; }
      else { colors[i * 3] = 1.0; colors[i * 3 + 1] = 0.9; colors[i * 3 + 2] = 0.8; }
    }
    stars.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    stars.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    const starMat = new THREE.PointsMaterial({
      size: 0.15, vertexColors: true, transparent: true, opacity: 0.85
    });
    return new THREE.Points(stars, starMat);
  }
  scene.add(createStarField());

  const hunyiGroup = new THREE.Group();
  scene.add(hunyiGroup);

  function createRing(radius, thickness, color, opacity = 0.7) {
    const geometry = new THREE.TorusGeometry(radius, thickness, 32, 128);
    const material = new THREE.MeshPhysicalMaterial({
      color, metalness: 0.7, roughness: 0.3,
      transparent: true, opacity, emissive: color,
      emissiveIntensity: 0.15, clearcoat: 0.3
    });
    const mesh = new THREE.Mesh(geometry, material);
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    return mesh;
  }

  const liuheGroup = new THREE.Group();
  {
    const liuhe_color = 0xa78bfa;
    const liuhe_horizon = createRing(2.8, 0.06, liuhe_color, 0.65);
    liuheGroup.add(liuhe_horizon);
    const liuhe_meridian = createRing(2.8, 0.06, liuhe_color, 0.65);
    liuhe_meridian.rotation.x = Math.PI / 2;
    liuheGroup.add(liuhe_meridian);
    const liuhe_prime = createRing(2.8, 0.06, liuhe_color, 0.5);
    liuhe_prime.rotation.x = Math.PI / 2;
    liuhe_prime.rotation.y = Math.PI / 2;
    liuheGroup.add(liuhe_prime);
  }
  hunyiGroup.add(liuheGroup);

  const sanchenGroup = new THREE.Group();
  {
    const sanchen_color = 0x7eb4ff;
    const sanchen_equator = createRing(2.2, 0.05, sanchen_color, 0.75);
    sanchen_equator.rotation.x = (23.5 * Math.PI / 180);
    sanchenGroup.add(sanchen_equator);
    const sanchen_ecliptic = createRing(2.2, 0.05, 0xfbbf24, 0.7);
    sanchenGroup.add(sanchen_ecliptic);
    const sanchen_meridian = createRing(2.2, 0.05, sanchen_color, 0.7);
    sanchen_meridian.rotation.x = Math.PI / 2;
    sanchenGroup.add(sanchen_meridian);
  }
  hunyiGroup.add(sanchenGroup);

  const siyoutGroup = new THREE.Group();
  {
    const siyout_color = 0x4c8bf5;
    const siyou_ring = createRing(1.6, 0.055, siyout_color, 0.85);
    siyou_ring.rotation.x = Math.PI / 2;
    siyoutGroup.add(siyou_ring);

    const sightTubeGeo = new THREE.CylinderGeometry(0.035, 0.035, 3.0, 16);
    const sightTubeMat = new THREE.MeshPhysicalMaterial({
      color: 0x60a5fa, metalness: 0.8, roughness: 0.25,
      transparent: true, opacity: 0.9, emissive: 0x1e40af, emissiveIntensity: 0.2
    });
    const sightTube = new THREE.Mesh(sightTubeGeo, sightTubeMat);
    sightTube.rotation.z = Math.PI / 2;
    sightTube.castShadow = true;
    siyoutGroup.add(sightTube);
  }
  {
    const axisGeo = new THREE.CylinderGeometry(0.04, 0.04, 3.2, 16);
    const axisMat = new THREE.MeshPhysicalMaterial({ color: 0xc084fc, metalness: 0.9, roughness: 0.2 });
    const axis = new THREE.Mesh(axisGeo, axisMat);
    hunyiGroup.add(axis);
  }
  hunyiGroup.add(siyoutGroup);

  {
    const baseGeo = new THREE.CylinderGeometry(1.8, 2.2, 0.3, 48);
    const baseMat = new THREE.MeshPhysicalMaterial({ color: 0x374151, metalness: 0.6, roughness: 0.5 });
    const base = new THREE.Mesh(baseGeo, baseMat);
    base.position.y = -1.9;
    base.receiveShadow = true;
    hunyiGroup.add(base);

    function createPillar() {
      const pillarGroup = new THREE.Group();
      const geo = new THREE.CylinderGeometry(0.05, 0.07, 1.6, 12);
      const mat = new THREE.MeshPhysicalMaterial({ color: 0x6b7280, metalness: 0.7, roughness: 0.4 });
      const pillar = new THREE.Mesh(geo, mat);
      pillar.castShadow = true;
      pillarGroup.add(pillar);
      return pillarGroup;
    }
    for (let i = 0; i < 4; i++) {
      const angle = (i * Math.PI / 2) + Math.PI / 4;
      const pillar = createPillar();
      pillar.position.set(Math.cos(angle) * 1.3, -1.0, Math.sin(angle) * 1.3);
      pillar.rotation.z = Math.cos(angle) * 0.15;
      pillar.rotation.x = Math.sin(angle) * 0.15;
      hunyiGroup.add(pillar);
    }
  }

  function involutePoint(rBase, angle) {
    const x = rBase * (Math.cos(angle) + angle * Math.sin(angle));
    const y = rBase * (Math.sin(angle) - angle * Math.cos(angle));
    return new THREE.Vector2(x, y);
  }

  function generateToothProfile(baseR, outerR, teeth, idx) {
    const points = [];
    const toothAngle = (Math.PI * 2) / teeth;
    const halfTooth = toothAngle / 2;
    const pressureAngle = 20 * Math.PI / 180;
    const involuteEnd = Math.tan(pressureAngle);
    const center = idx * toothAngle;
    for (let t = 0; t <= 1; t += 0.05) {
      const ang = involuteEnd * t;
      const inv = involutePoint(baseR, ang);
      const rotated = inv.rotateAround(new THREE.Vector2(0, 0), center - halfTooth);
      points.push(new THREE.Vector3(rotated.x, rotated.y, 0));
    }
    for (let t = 1; t >= 0; t -= 0.05) {
      const ang = involuteEnd * t;
      const inv = involutePoint(baseR, ang);
      const rotated = inv.rotateAround(new THREE.Vector2(0, 0), center + halfTooth);
      const flipped = new THREE.Vector2(rotated.x, -rotated.y).rotateAround(new THREE.Vector2(0, 0), center + halfTooth);
      const final = new THREE.Vector2(
        Math.cos(center + halfTooth) * baseR + (flipped.x - Math.cos(center + halfTooth) * baseR),
        Math.sin(center + halfTooth) * baseR + (flipped.y - Math.sin(center + halfTooth) * baseR)
      );
      points.push(new THREE.Vector3(final.x, final.y, 0));
    }
    return points;
  }

  function createGearWireframe(innerRadius, outerRadius, teeth, thickness, color, opacity) {
    const group = new THREE.Group();
    const baseR = outerRadius * Math.cos(20 * Math.PI / 180);
    const toothDepth = (outerRadius - innerRadius) * 0.6;
    const rootR = outerRadius - toothDepth;
    const outlinePoints = [];
    const toothProfiles = [];
    for (let i = 0; i < teeth; i++) {
      const toothPts = generateToothProfile(baseR, outerRadius, teeth, i);
      outlinePoints.push(...toothPts);
      toothProfiles.push(toothPts.map(p => p.clone()));
    }
    outlinePoints.push(outlinePoints[0].clone());
    const lineMat = new THREE.LineBasicMaterial({ color, transparent: true, opacity });
    const outlineGeo = new THREE.BufferGeometry().setFromPoints(outlinePoints);
    group.add(new THREE.Line(outlineGeo, lineMat));
    const rootPts = [];
    for (let i = 0; i <= 128; i++) {
      const a = (i / 128) * Math.PI * 2;
      rootPts.push(new THREE.Vector3(Math.cos(a) * rootR, Math.sin(a) * rootR, 0));
    }
    group.add(new THREE.LineLoop(new THREE.BufferGeometry().setFromPoints(rootPts), lineMat));
    for (const sign of [-1, 1]) {
      const z = sign * thickness / 2;
      const innerPts = [];
      for (let i = 0; i <= 64; i++) {
        const a = (i / 64) * Math.PI * 2;
        innerPts.push(new THREE.Vector3(Math.cos(a) * innerRadius, Math.sin(a) * innerRadius, z));
      }
      group.add(new THREE.LineLoop(new THREE.BufferGeometry().setFromPoints(innerPts), lineMat));
    }
    const edgePts = [];
    for (let i = 0; i <= 12; i++) {
      const a = (i / 12) * Math.PI * 2;
      edgePts.push(new THREE.Vector3(Math.cos(a) * innerRadius, Math.sin(a) * innerRadius, -thickness / 2));
      edgePts.push(new THREE.Vector3(Math.cos(a) * innerRadius, Math.sin(a) * innerRadius, thickness / 2));
    }
    group.add(new THREE.LineSegments(new THREE.BufferGeometry().setFromPoints(edgePts), lineMat));

    const contactPulseGeo = new THREE.RingGeometry(outerRadius * 0.92, outerRadius * 1.08, 32, 1, 0, Math.PI * 0.35);
    const contactPulseMat = new THREE.MeshBasicMaterial({
      color: 0xff4444, transparent: true, opacity: 0, side: THREE.DoubleSide
    });
    const contactPulse = new THREE.Mesh(contactPulseGeo, contactPulseMat);
    contactPulse.visible = false;
    group.add(contactPulse);

    group.userData = {
      ...group.userData, teeth, baseR, outerR: outerRadius, innerR: innerRadius,
      toothProfiles, contactPulse,
      currentOmega: 0, lastOmega: 0, contactFlash: 0, engagedTooth: -1
    };
    return group;
  }

  const gearsGroup = new THREE.Group();
  const gearConfigs = [
    { id: 0, r: 0.55, pos: [2.0, -1.2, 0.3], teeth: 80, color: 0xfbbf24, baseSpeed: 1.8, pairWith: 1 },
    { id: 1, r: 0.45, pos: [2.0, -0.7, -0.5], teeth: 60, color: 0xf59e0b, baseSpeed: -2.4, pairWith: 0 },
    { id: 2, r: 0.38, pos: [-2.0, -1.2, 0.5], teeth: 48, color: 0xfbbf24, baseSpeed: 2.2, pairWith: 3 },
    { id: 3, r: 0.48, pos: [-2.0, -0.6, -0.3], teeth: 72, color: 0xf59e0b, baseSpeed: -1.47, pairWith: 2 },
    { id: 4, r: 0.32, pos: [0, -2.0, 1.8], teeth: 40, color: 0xfbbf24, baseSpeed: 2.6, pairWith: 5 },
    { id: 5, r: 0.42, pos: [0.5, -2.0, -1.8], teeth: 56, color: 0xf59e0b, baseSpeed: -1.86, pairWith: 4 },
  ];
  gearConfigs.forEach(cfg => {
    const gear = createGearWireframe(cfg.r * 0.45, cfg.r, cfg.teeth, 0.08, cfg.color, 0.6);
    gear.position.set(...cfg.pos);
    gear.rotation.x = 0.35 + cfg.id * 0.15;
    gear.rotation.y = -0.25 + cfg.id * 0.1;
    gear.userData.baseSpeed = cfg.baseSpeed;
    gear.userData.id = cfg.id;
    gear.userData.pairWith = cfg.pairWith;
    gear.userData.cfg = cfg;
    gearsGroup.add(gear);
  });
  hunyiGroup.add(gearsGroup);

  function detectToothContact(gearA, gearB) {
    const worldToLocalB = gearB.matrixWorld.clone().invert();
    let minDist = Infinity, closestIdxA = -1, closestIdxB = -1, closestPoint = null;
    for (let i = 0; i < gearA.userData.toothProfiles.length; i++) {
      const profileA = gearA.userData.toothProfiles[i];
      for (const pA of profileA) {
        const worldA = pA.clone().applyMatrix4(gearA.matrixWorld);
        const localB = worldA.clone().applyMatrix4(worldToLocalB);
        for (let j = 0; j < gearB.userData.toothProfiles.length; j++) {
          const profileB = gearB.userData.toothProfiles[j];
          for (const pB of profileB) {
            const dx = localB.x - pB.x, dy = localB.y - pB.y;
            const dist = Math.sqrt(dx * dx + dy * dy);
            if (dist < minDist) {
              minDist = dist; closestIdxA = i; closestIdxB = j;
              closestPoint = worldA.clone();
            }
          }
        }
      }
    }
    return { minDist, closestIdxA, closestIdxB, closestPoint };
  }

  function enforceMeshingConstraint(gearA, gearB) {
    const ratio = -gearA.userData.cfg.teeth / gearB.userData.cfg.teeth;
    const desiredOmegaB = gearA.userData.currentOmega * ratio;
    const omegaError = gearB.userData.currentOmega - desiredOmegaB;
    if (Math.abs(omegaError) > 0.05) {
      gearB.userData.currentOmega -= omegaError * 0.25;
      gearA.userData.currentOmega += omegaError * 0.1 * Math.abs(ratio);
    }
  }

  function updateContactVisualization(gear, contactPoint) {
    const pulse = gear.userData.contactPulse;
    pulse.visible = true;
    pulse.material.opacity = 0.85;
    gear.userData.contactFlash = 1.0;
    if (contactPoint) {
      const local = contactPoint.clone().applyMatrix4(gear.matrixWorld.clone().invert());
      const ang = Math.atan2(local.y, local.x);
      pulse.rotation.z = ang - Math.PI * 0.18;
    }
    gear.children.forEach(child => {
      if (child.material && child.material.color) {
        child.material.color.setHex(0xff5555);
        child.material.opacity = 0.9;
      }
    });
  }

  const errorArrowGroup = new THREE.Group();
  function createArrow(color, from, to, headLength = 0.3, headWidth = 0.12) {
    const dir = new THREE.Vector3().subVectors(to, from).normalize();
    const length = from.distanceTo(to);
    return new THREE.ArrowHelper(dir, from, length, color, headLength, headWidth);
  }
  const theoreticalArrow = createArrow(0x4ade80, new THREE.Vector3(0, 0, 0), new THREE.Vector3(0, 2, 0), 0.25, 0.1);
  theoreticalArrow.material.transparent = true;
  theoreticalArrow.material.opacity = 0.75;
  errorArrowGroup.add(theoreticalArrow);
  const errorArrow = createArrow(0xf87171, new THREE.Vector3(0, 0, 0), new THREE.Vector3(0.1, 1.9, 0), 0.25, 0.1);
  errorArrow.material.transparent = true;
  errorArrow.material.opacity = 0.9;
  errorArrowGroup.add(errorArrow);
  const deviationLineMat = new THREE.LineDashedMaterial({
    color: 0xf87171, dashSize: 0.08, gapSize: 0.05, transparent: true, opacity: 0.7
  });
  const deviationGeo = new THREE.BufferGeometry().setFromPoints([
    new THREE.Vector3(0, 2, 0), new THREE.Vector3(0.1, 1.9, 0)
  ]);
  const deviationLine = new THREE.Line(deviationGeo, deviationLineMat);
  deviationLine.computeLineDistances();
  errorArrowGroup.add(deviationLine);
  const errorLabel = new THREE.Mesh(
    new THREE.SphereGeometry(0.04, 16, 16),
    new THREE.MeshBasicMaterial({ color: 0xef4444 })
  );
  errorLabel.position.set(0.05, 1.95, 0);
  errorArrowGroup.add(errorLabel);
  hunyiGroup.add(errorArrowGroup);

  function createTickMarks(ringRadius, count, color, length = 0.08) {
    const group = new THREE.Group();
    for (let i = 0; i < count; i++) {
      const angle = (i / count) * Math.PI * 2;
      const inner = ringRadius - length;
      const outer = ringRadius + 0.02;
      const isMajor = i % (count / 12) === 0;
      const pts = [
        new THREE.Vector3(Math.cos(angle) * (isMajor ? inner - 0.05 : inner),
          Math.sin(angle) * (isMajor ? inner - 0.05 : inner), 0),
        new THREE.Vector3(Math.cos(angle) * outer, Math.sin(angle) * outer, 0)
      ];
      const geo = new THREE.BufferGeometry().setFromPoints(pts);
      const mat = new THREE.LineBasicMaterial({
        color, transparent: true, opacity: isMajor ? 0.8 : 0.4
      });
      group.add(new THREE.Line(geo, mat));
    }
    return group;
  }
  {
    const ticks_eq = createTickMarks(2.2, 72, 0x7eb4ff);
    ticks_eq.rotation.x = 23.5 * Math.PI / 180;
    sanchenGroup.add(ticks_eq);
    sanchenGroup.add(createTickMarks(2.2, 72, 0xfbbf24));
    const ticks_siyou = createTickMarks(1.6, 48, 0x4c8bf5);
    ticks_siyou.rotation.x = Math.PI / 2;
    siyoutGroup.add(ticks_siyou);
  }

  const state = {
    autoRotate: true, showGears: true, showError: true,
    currentReading: null, currentPointing: null
  };

  function setAutoRotate(v) { state.autoRotate = v; }
  function setShowGears(v) { state.showGears = v; gearsGroup.visible = v; }
  function setShowError(v) { state.showError = v; errorArrowGroup.visible = v; }
  function resetView() { camera.position.set(5, 3.5, 7); controls.target.set(0, 0, 0); controls.update(); }
  function updateAxes(r) { state.currentReading = r; }
  function updatePointing(p) { state.currentPointing = p; }

  const clock = new THREE.Clock();
  let timeAccum = 0;
  let rafId = 0;

  function animate() {
    rafId = requestAnimationFrame(animate);
    const delta = clock.getDelta();
    timeAccum += delta;

    if (state.autoRotate) hunyiGroup.rotation.y += delta * 0.15;

    const processedPairs = new Set();
    gearsGroup.children.forEach(gear => {
      if (gear.userData.currentOmega === undefined) gear.userData.currentOmega = gear.userData.baseSpeed;
      gear.userData.currentOmega += (gear.userData.baseSpeed - gear.userData.currentOmega) * Math.min(1, delta * 0.8);
    });
    gearsGroup.children.forEach(gearA => {
      const pairIdx = gearA.userData.pairWith;
      const pairKey = [Math.min(gearA.userData.id, pairIdx), Math.max(gearA.userData.id, pairIdx)].join('-');
      if (processedPairs.has(pairKey)) return;
      processedPairs.add(pairKey);
      const gearB = gearsGroup.children[pairIdx];
      if (!gearB) return;
      enforceMeshingConstraint(gearA, gearB);
      const contact = detectToothContact(gearA, gearB);
      const threshold = (gearA.userData.outerR + gearB.userData.outerR) * 0.02;
      if (contact.minDist < threshold) {
        const overlap = threshold - contact.minDist;
        const hertzForce = 1.2e5 * Math.pow(overlap, 1.5);
        const impulse = hertzForce * delta * 1e-6;
        const ratioA = gearA.userData.cfg.teeth / (gearA.userData.cfg.teeth + gearB.userData.cfg.teeth);
        gearA.userData.currentOmega -= impulse * ratioA * Math.sign(gearA.userData.currentOmega);
        gearB.userData.currentOmega += impulse * (1 - ratioA) * Math.sign(gearB.userData.currentOmega);
        if (gearA.userData.contactFlash < 0.1) {
          updateContactVisualization(gearA, contact.closestPoint);
          updateContactVisualization(gearB, contact.closestPoint);
        }
        gearA.userData.engagedTooth = contact.closestIdxA;
        gearB.userData.engagedTooth = contact.closestIdxB;
      }
    });
    gearsGroup.children.forEach(gear => {
      gear.rotation.z += gear.userData.currentOmega * delta;
      gear.userData.contactFlash = Math.max(0, gear.userData.contactFlash - delta * 3.5);
      const pulse = gear.userData.contactPulse;
      if (pulse) {
        pulse.material.opacity = Math.max(0, pulse.material.opacity - delta * 3.0);
        pulse.scale.setScalar(1 + (1 - pulse.material.opacity) * 0.3);
        if (pulse.material.opacity < 0.01) pulse.visible = false;
      }
      if (gear.userData.contactFlash < 0.05 && gear.children) {
        const cfg = gear.userData.cfg;
        if (cfg) {
          gear.children.forEach((child, idx) => {
            if (idx < 4 && child.material && child.material.color) {
              child.material.color.lerp(new THREE.Color(cfg.color), Math.min(1, delta * 4));
              child.material.opacity = THREE.MathUtils.lerp(child.material.opacity, 0.55, Math.min(1, delta * 3));
            }
          });
        }
      }
    });

    if (state.currentReading) {
      siyoutGroup.rotation.y = state.currentReading.axis_azimuth_angle * Math.PI / 180;
      siyoutGroup.rotation.x = (90 - state.currentReading.axis_elevation_angle) * Math.PI / 180;
      sanchenGroup.rotation.z = state.currentReading.axis_equatorial_angle * Math.PI / 180;
    }
    if (state.showError && state.currentPointing) {
      const raRad = state.currentPointing.target_ra * Math.PI / 180;
      const decRad = state.currentPointing.target_dec * Math.PI / 180;
      const r = 2.0;
      const tx = r * Math.cos(decRad) * Math.cos(raRad);
      const ty = r * Math.sin(decRad);
      const tz = r * Math.cos(decRad) * Math.sin(raRad);
      const target = new THREE.Vector3(tx, ty, tz);
      theoreticalArrow.setDirection(target.clone().normalize());
      theoreticalArrow.setLength(target.length(), 0.2, 0.1);

      const errRaRad = (state.currentPointing.ra_error / 60) * Math.PI / 180;
      const errDecRad = (state.currentPointing.dec_error / 60) * Math.PI / 180;
      const mx = r * Math.cos(decRad + errDecRad) * Math.cos(raRad + errRaRad);
      const my = r * Math.sin(decRad + errDecRad);
      const mz = r * Math.cos(decRad + errDecRad) * Math.sin(raRad + errRaRad);
      const measured = new THREE.Vector3(mx, my, mz);
      errorArrow.setDirection(measured.clone().normalize());
      errorArrow.setLength(measured.length(), 0.2, 0.1);
      deviationGeo.setFromPoints([target, measured]);
      deviationGeo.computeLineDistances();
      errorLabel.position.copy(target.clone().add(measured).multiplyScalar(0.5));
    }

    pointLight1.intensity = 1.0 + Math.sin(timeAccum * 2) * 0.2;
    pointLight2.intensity = 0.6 + Math.cos(timeAccum * 1.5) * 0.15;
    controls.update();
    renderer.render(scene, camera);
  }
  animate();

  function onResize() {
    camera.aspect = container.clientWidth / container.clientHeight;
    camera.updateProjectionMatrix();
    renderer.setSize(container.clientWidth, container.clientHeight);
  }
  window.addEventListener('resize', onResize);

  function dispose() {
    cancelAnimationFrame(rafId);
    window.removeEventListener('resize', onResize);
    controls.dispose();
    renderer.dispose();
  }

  return {
    setAutoRotate, setShowGears, setShowError, resetView,
    updateAxes, updatePointing, dispose
  };
}
