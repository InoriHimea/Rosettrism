import React, { useEffect, useRef } from 'react';
import * as THREE from 'three';

/**
 * LyricStage3D — Three.js 驱动的 3D 粒子星云舞台背景。
 *
 * 设计目标：
 * - 用自定义 shader 渲染柔和发光的粒子星云，作为 karaoke 歌词舞台的沉浸式背景。
 * - 粒子情绪跟随播放状态变化：phase（metadata/countdown/singing/interlude/ended）
 *   决定整体强度，currentMs/durationMs 进度驱动分布与流速，isPlaying 控制呼吸节奏。
 * - 颜色跟随用户选择的 colorPreset / solidColor，与歌词渐变保持视觉一致。
 * - 鼠标视差：相机随光标轻微偏移，营造空间纵深感。
 * - 性能与可达性：IntersectionObserver 不可见时暂停渲染；prefers-reduced-motion
 *   时完全不启动 WebGL，回退到 CSS 静态背景；DPR 上限 2；按视口宽度自适应粒子数。
 *
 * 该组件是对现有 CSS LyricAmbientEffects 的升级替代，由 settings.stage3D 开关控制。
 */
const PRESET_COLORS = {
  'qq-prism': [
    [0.063, 0.784, 0.627], // #10c8a0 青绿
    [0.133, 0.773, 0.369], // #22c55e 翠绿
    [0.945, 0.784, 0.294], // #f1c84b 金黄
  ],
  aurora: [
    [0.525, 0.945, 0.675], // #86efac 浅绿
    [0.133, 0.827, 0.933], // #22d3ee 青
    [0.376, 0.639, 0.980], // #60a5fa 蓝
  ],
  sunset: [
    [0.984, 0.749, 0.141], // #fbbf24 琥珀
    [0.984, 0.443, 0.522], // #fb7185 珊瑚粉
    [0.655, 0.545, 0.980], // #a78bfa 薰衣草
  ],
  classic: [
    [1.0, 1.0, 1.0], // #ffffff 白
    [0.812, 0.980, 1.0], // #cffafe 冰蓝
    [0.878, 0.906, 1.0], // #e0e7ff 雾蓝
  ],
};

const PHASE_INTENSITY = {
  metadata: 0.38,
  countdown: 0.55,
  singing: 1.0,
  interlude: 0.62,
  ended: 0.2,
};

const DEFAULT_INTENSITY = 0.4;

const VERTEX_SHADER = /* glsl */ `
  uniform float uTime;
  uniform float uFlow;
  uniform float uIntensity;
  uniform float uPixelRatio;
  uniform float uMouseX;
  uniform float uMouseY;
  attribute float aSeed;
  attribute float aSize;
  varying float vSeed;
  varying float vGlow;

  void main() {
    vSeed = aSeed;
    vec3 pos = position;
    float t = uTime * uFlow;
    float phase = aSeed * 6.28318;
    // 粒子按各自相位缓慢漂浮，形成有机的星云流动
    pos.x += sin(t * 0.32 + phase) * 0.42;
    pos.y += cos(t * 0.26 + phase * 0.83) * 0.36;
    pos.z += sin(t * 0.21 + phase * 1.21) * 0.30;
    // 鼠标视差：整体随光标方向产生轻微纵深位移
    pos.x += uMouseX * 0.5 * (0.5 + aSeed);
    pos.y += uMouseY * 0.5 * (0.5 + aSeed);
    // 大小脉动，活跃时更明显
    float pulse = 0.62 + 0.38 * sin(uTime * 1.15 + phase);
    vGlow = pulse;
    vec4 mvPosition = modelViewMatrix * vec4(pos, 1.0);
    float depth = max(0.001, -mvPosition.z);
    gl_PointSize = aSize * uPixelRatio * pulse * (0.65 + uIntensity * 0.9) * (260.0 / depth);
    gl_Position = projectionMatrix * mvPosition;
  }
`;

const FRAGMENT_SHADER = /* glsl */ `
  uniform vec3 uColorA;
  uniform vec3 uColorB;
  uniform vec3 uColorC;
  uniform float uIntensity;
  uniform float uAlpha;
  varying float vSeed;
  varying float vGlow;

  void main() {
    vec2 uv = gl_PointCoord - 0.5;
    float d = length(uv);
    if (d > 0.5) discard;
    // 软圆核心 + 外层光晕
    float core = smoothstep(0.5, 0.0, d);
    float halo = smoothstep(0.5, 0.18, d) * 0.5;
    float shape = core + halo;
    // 三色按 seed 在粒子间插值，形成多彩星云
    float s = fract(vSeed * 3.17);
    vec3 col;
    if (s < 0.5) {
      col = mix(uColorA, uColorB, s * 2.0);
    } else {
      col = mix(uColorB, uColorC, (s - 0.5) * 2.0);
    }
    float a = shape * (0.42 + 0.58 * vGlow) * uAlpha * (0.45 + uIntensity * 0.55);
    gl_FragColor = vec4(col, a);
  }
`;

function resolveColorTriplet(colorMode, colorPreset, solidColor) {
  if (colorMode === 'solid' && solidColor) {
    const c = new THREE.Color(solidColor);
    return [c, c, c];
  }
  const triplet = PRESET_COLORS[colorPreset] || PRESET_COLORS['qq-prism'];
  return triplet.map((rgb) => new THREE.Color(rgb[0], rgb[1], rgb[2]));
}

function backgroundLuminance(hex) {
  const value = String(hex || '#fff0a6').trim();
  const match = /^#?([\da-f]{6})$/i.exec(value);
  if (!match) {
    return 0.85;
  }
  const int = parseInt(match[1], 16);
  const r = (int >> 16) & 255;
  const g = (int >> 8) & 255;
  const b = int & 255;
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}

function particleCountFor(width, lowDistraction) {
  if (!Number.isFinite(width) || width <= 0) {
    return 0;
  }
  const base = width < 720 ? 320 : 640;
  const count = Math.round(base * Math.min(1.2, Math.max(0.5, width / 900)));
  return lowDistraction ? Math.round(count * 0.5) : count;
}

export function LyricStage3D({
  currentMs = 0,
  durationMs = 1000,
  isPlaying = false,
  phase = 'singing',
  colorPreset = 'qq-prism',
  colorMode = 'gradient',
  solidColor = '#14c9a2',
  lowDistraction = false,
  stageBackgroundColor = '#fff0a6',
}) {
  const containerRef = useRef(null);

  // 用 ref 承载高频变化的播放状态，避免每帧触发 React 重渲染
  const stateRef = useRef({ currentMs, durationMs, isPlaying, phase });
  stateRef.current = { currentMs, durationMs, isPlaying, phase };

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return undefined;
    }

    // 可达性：用户偏好减少动效时，不启动 WebGL，回退 CSS 静态背景
    const reduceMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
    if (reduceMotion) {
      return undefined;
    }

    let width = container.clientWidth || 1;
    let height = container.clientHeight || 1;
    const count = particleCountFor(width, lowDistraction);
    if (count === 0) {
      return undefined;
    }

    const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true, powerPreference: 'high-performance' });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.setSize(width, height);
    renderer.setClearColor(0x000000, 0);
    container.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(58, width / height, 0.1, 100);
    camera.position.set(0, 0, 8);

    // 粒子几何：在椭球空间随机分布，附 seed 与尺寸属性
    const positions = new Float32Array(count * 3);
    const seeds = new Float32Array(count);
    const sizes = new Float32Array(count);
    for (let i = 0; i < count; i += 1) {
      const i3 = i * 3;
      // 球面均匀采样 + 轻微椭球压缩，让星云有立体厚度
      const u = Math.random();
      const v = Math.random();
      const theta = 2 * Math.PI * u;
      const phi = Math.acos(2 * v - 1);
      const r = 2.6 + Math.pow(Math.random(), 0.7) * 3.4;
      positions[i3] = r * Math.sin(phi) * Math.cos(theta) * 1.15;
      positions[i3 + 1] = r * Math.sin(phi) * Math.sin(theta) * 0.82;
      positions[i3 + 2] = r * Math.cos(phi) * 0.7;
      seeds[i] = Math.random();
      sizes[i] = 6 + Math.random() * 14;
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geometry.setAttribute('aSeed', new THREE.BufferAttribute(seeds, 1));
    geometry.setAttribute('aSize', new THREE.BufferAttribute(sizes, 1));

    const [cA, cB, cC] = resolveColorTriplet(colorMode, colorPreset, solidColor);
    const uniforms = {
      uTime: { value: 0 },
      uFlow: { value: 0.2 },
      uIntensity: { value: DEFAULT_INTENSITY },
      uPixelRatio: { value: renderer.getPixelRatio() },
      uMouseX: { value: 0 },
      uMouseY: { value: 0 },
      uColorA: { value: cA },
      uColorB: { value: cB },
      uColorC: { value: cC },
      uAlpha: { value: 0.8 },
    };
    const material = new THREE.ShaderMaterial({
      uniforms,
      vertexShader: VERTEX_SHADER,
      fragmentShader: FRAGMENT_SHADER,
      transparent: true,
      depthWrite: false,
      depthTest: false,
      blending: THREE.NormalBlending,
    });
    // 暴露 material 到容器，供颜色副作用在 props 变化时更新 uniform
    container.__lyricStage3DMaterial = material;
    const points = new THREE.Points(geometry, material);
    scene.add(points);

    // 鼠标视差：监听 window 以容器 rect 归一化，保证容器设 pointer-events:none 时仍能响应
    const target = { x: 0, y: 0 };
    const onMouseMove = (event) => {
      const rect = container.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0) {
        return;
      }
      target.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
      target.y = -(((event.clientY - rect.top) / rect.height) * 2 - 1);
    };
    window.addEventListener('mousemove', onMouseMove);

    // 视口适配
    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const w = entry.contentRect.width;
        const h = entry.contentRect.height;
        if (w > 0 && h > 0) {
          width = w;
          height = h;
          camera.aspect = width / height;
          camera.updateProjectionMatrix();
          renderer.setSize(width, height);
        }
      }
    });
    resizeObserver.observe(container);

    // 可见性暂停：舞台滚出视口或被遮挡时停止渲染，节省 GPU
    let visible = true;
    const intersectionObserver = new IntersectionObserver((entries) => {
      visible = entries[0]?.isIntersecting !== false;
    }, { threshold: 0 });
    intersectionObserver.observe(container);

    const clock = new THREE.Clock();
    let frameId = 0;

    const render = () => {
      frameId = requestAnimationFrame(render);
      if (!visible) {
        return;
      }
      const elapsed = clock.getElapsedTime();
      const state = stateRef.current;
      const intensity = PHASE_INTENSITY[state.phase] ?? DEFAULT_INTENSITY;
      const progress = state.durationMs > 0
        ? Math.min(1, Math.max(0, state.currentMs / state.durationMs))
        : 0;
      // 进度曲线：中段最浓，首尾收敛
      const progressBoost = 1 - Math.abs(progress - 0.5) * 0.8;
      // 播放时流动加快并伴随呼吸；暂停时收敛为近乎静止的微光
      const flow = state.isPlaying ? (0.16 + 0.18 * intensity) : 0.04;
      const effectiveIntensity = intensity * (0.55 + 0.45 * progressBoost);

      uniforms.uTime.value = elapsed;
      uniforms.uFlow.value = flow;
      uniforms.uIntensity.value = effectiveIntensity;

      // 相机视差 lerp
      camera.position.x += (target.x * 1.4 - camera.position.x) * 0.05;
      camera.position.y += (target.y * 1.0 - camera.position.y) * 0.05;
      camera.lookAt(0, 0, 0);

      // 星云整体缓慢自转，增强空间感
      points.rotation.y = elapsed * 0.03;
      points.rotation.x = Math.sin(elapsed * 0.02) * 0.12;

      renderer.render(scene, camera);
    };
    render();

    return () => {
      cancelAnimationFrame(frameId);
      window.removeEventListener('mousemove', onMouseMove);
      resizeObserver.disconnect();
      intersectionObserver.disconnect();
      geometry.dispose();
      material.dispose();
      renderer.dispose();
      if (renderer.domElement.parentNode === container) {
        container.removeChild(renderer.domElement);
      }
      delete container.__lyricStage3DMaterial;
    };
    // 仅在容器挂载与低干扰开关变化时重建场景
  }, [lowDistraction]);

  // 颜色 / 背景明暗变化时更新 uniform，无需重建场景
  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }
    const material = container.__lyricStage3DMaterial;
    if (!material || !material.uniforms) {
      return;
    }
    const [cA, cB, cC] = resolveColorTriplet(colorMode, colorPreset, solidColor);
    material.uniforms.uColorA.value = cA;
    material.uniforms.uColorB.value = cB;
    material.uniforms.uColorC.value = cC;
    const lum = backgroundLuminance(stageBackgroundColor);
    // 浅色舞台背景：粒子透明度调低避免过亮；深色背景：提高发光感
    const baseAlpha = lum > 0.6 ? 0.5 : 0.9;
    material.uniforms.uAlpha.value = lowDistraction ? baseAlpha * 0.6 : baseAlpha;
  }, [colorPreset, colorMode, solidColor, stageBackgroundColor, lowDistraction]);

  return <div className="lyric-stage-3d" ref={containerRef} aria-hidden="true" />;
}
