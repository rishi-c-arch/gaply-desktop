// Gaply — desktop landing globe. A clean, self-contained three.js (via
// react-three-fiber) interactive globe. It renders in a SQUARE container so the
// sphere is a true circle; auto-spins when idle and can be dragged to rotate.
//
// ─────────────────────────────────────────────────────────────────────────
// DROP-IN POINT: restyle from your reference images via the two configs below.
// GLOBE_CONFIG = dark theme, GLOBE_CONFIG_LIGHT = light-theme overrides.
// To map an image onto the sphere, set `texture` and uncomment the textured
// material branch in <GlobeMesh/>.
// ─────────────────────────────────────────────────────────────────────────
import React, { Suspense, useMemo, useRef } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls, Environment } from '@react-three/drei';
import * as THREE from 'three';

// Small stats formulas sprinkled between the big notation on the light-mode
// paper sphere — Gaply's own vocabulary, in ink.
const SMALL_FORMULAS = [
  'p = 0.002', 't(47) = 3.2', 'd = 0.46', 'H = −Σ p·log p', 'ppl = 2^H',
  'z = (x−μ)/σ', 'R² = 1 − SSr/SSt', 'n = 48', 'CI = x̄ ± z·σ/√n',
];

/** Build the light-mode "graph-paper math sphere" texture: warm white paper,
 *  a fine teal graph grid, and large handwritten-style notation in dark ink
 *  (Σ, ∫f(x)dx, π ≈ 3.1415, a big α, arrowed axes…) — the reference look.
 *  Equirect 2048×1024, repeat 1×1; the big glyphs stay in the mid band so
 *  pole distortion never mangles them, and nothing crosses the x-seam. */
function makePaperMathTexture(): THREE.Texture {
  const w = 2048;
  const h = 1024;
  const c = document.createElement('canvas');
  c.width = w;
  c.height = h;
  const ctx = c.getContext('2d')!;

  // paper
  ctx.fillStyle = '#f8f9f7';
  ctx.fillRect(0, 0, w, h);

  // graph grid — minor 32px, major 128px (both divide 2048 → seamless wrap)
  ctx.strokeStyle = 'rgba(96, 160, 146, 0.34)';
  ctx.lineWidth = 1;
  for (let x = 0; x <= w; x += 32) {
    ctx.beginPath(); ctx.moveTo(x + 0.5, 0); ctx.lineTo(x + 0.5, h); ctx.stroke();
  }
  for (let y = 0; y <= h; y += 32) {
    ctx.beginPath(); ctx.moveTo(0, y + 0.5); ctx.lineTo(w, y + 0.5); ctx.stroke();
  }
  ctx.strokeStyle = 'rgba(70, 140, 125, 0.48)';
  ctx.lineWidth = 1.5;
  for (let x = 0; x <= w; x += 128) {
    ctx.beginPath(); ctx.moveTo(x + 0.5, 0); ctx.lineTo(x + 0.5, h); ctx.stroke();
  }
  for (let y = 0; y <= h; y += 128) {
    ctx.beginPath(); ctx.moveTo(0, y + 0.5); ctx.lineTo(w, y + 0.5); ctx.stroke();
  }

  // handwritten-ish dark ink (italic serif reads as blackboard notation)
  const ink = (text: string, x: number, y: number, px: number, rot = 0, alpha = 0.92) => {
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(rot);
    ctx.font = `italic ${px}px Georgia, "Times New Roman", serif`;
    ctx.fillStyle = `rgba(28, 36, 48, ${alpha})`;
    ctx.textBaseline = 'middle';
    ctx.fillText(text, 0, 0);
    ctx.restore();
  };
  const stroke = (draw: () => void, width = 5, alpha = 0.85) => {
    ctx.save();
    ctx.strokeStyle = `rgba(28, 36, 48, ${alpha})`;
    ctx.lineWidth = width;
    ctx.lineCap = 'round';
    draw();
    ctx.restore();
  };
  const arrow = (x1: number, y1: number, x2: number, y2: number) => {
    stroke(() => {
      ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke();
      const a = Math.atan2(y2 - y1, x2 - x1);
      ctx.beginPath();
      ctx.moveTo(x2, y2);
      ctx.lineTo(x2 - 16 * Math.cos(a - 0.42), y2 - 16 * Math.sin(a - 0.42));
      ctx.moveTo(x2, y2);
      ctx.lineTo(x2 - 16 * Math.cos(a + 0.42), y2 - 16 * Math.sin(a + 0.42));
      ctx.stroke();
    });
  };

  // — the big reference glyphs, mid-band, clear of the x-seam —
  // Σ with limits + 2π (front-left)
  ink('∑', 220, 430, 200, -0.04);
  ink('π', 258, 302, 54, 0, 0.8);
  ink('i=0', 236, 560, 46, 0, 0.8);
  ink('2π', 420, 360, 92, 0.05);
  // ∫ f(x) dx
  ink('∫ f(x) dx', 620, 520, 86, -0.06);
  // the big π ≈ 3.1415
  ink('π ≈ 3.1415', 980, 640, 140, -0.03);
  // the large α
  ink('α', 880, 430, 230, 0.06);
  // arrowed axes with labels (reference top-left motif)
  arrow(1420, 560, 1420, 330);
  arrow(1330, 470, 1640, 470);
  ink('x', 1660, 470, 52, 0, 0.8);
  ink('c', 1440, 330, 50, 0, 0.8);
  ink('b', 1330, 520, 50, 0, 0.8);
  ink('a', 1560, 540, 50, 0.08, 0.8);
  // a soft parabola through the axes
  stroke(() => {
    ctx.beginPath();
    ctx.moveTo(1340, 440);
    ctx.quadraticCurveTo(1480, 620, 1620, 400);
    ctx.stroke();
  }, 4, 0.6);
  // f′(x) ≈ 2x + 4
  ink('f′(x) ≈ 2x + 4', 1250, 700, 64, 0.04);
  // smaller companions
  ink('√2', 540, 250, 64, -0.08, 0.75);
  ink('β', 1740, 620, 96, -0.05, 0.8);
  ink('(f(x))', 1700, 260, 56, 0.06, 0.7);
  // fill the sparser longitudes so EVERY rotation face shows notation
  ink('lim', 90, 330, 72, 0.03, 0.85);
  ink('n→∞', 84, 410, 46, 0.03, 0.75);
  ink('∂y/∂x', 1850, 500, 78, -0.04, 0.85);
  ink('e^{iπ} + 1 = 0', 60, 620, 60, -0.04, 0.8);
  ink('∇·F', 1880, 720, 66, 0.05, 0.75);
  ink('Δx', 1180, 470, 70, -0.06, 0.7);

  // small Gaply stats lines, faint, scattered in the band
  const spots: Array<[number, number, number]> = [
    [180, 700, -0.05], [520, 760, 0.04], [820, 250, 0.05], [1130, 300, -0.04],
    [1520, 760, -0.06], [340, 200, 0.03], [1860, 420, 0.05], [700, 660, -0.03],
    [1000, 800, 0.05],
  ];
  spots.forEach(([x, y, r], i) => ink(SMALL_FORMULAS[i % SMALL_FORMULAS.length], x, y, 40, r, 0.55));

  const tex = new THREE.CanvasTexture(c);
  tex.wrapS = THREE.RepeatWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.anisotropy = 16;
  // keep the paper white faithful (guarded for older three versions)
  const anyTex = tex as any;
  const anyThree = THREE as any;
  if (anyThree.SRGBColorSpace) anyTex.colorSpace = anyThree.SRGBColorSpace;
  else if (anyThree.sRGBEncoding) anyTex.encoding = anyThree.sRGBEncoding;
  return tex;
}

export const GLOBE_CONFIG = {
  radius: 1.6,
  segments: 72,
  /** idle auto-spin, radians per second */
  autoRotateSpeed: 0.3,
  /** base sphere fill */
  sphereColor: '#0b0b0d',
  /** lat/long wireframe — the original red accent */
  wireColor: '#ff2a2a',
  wireOpacity: 0.45,
  /** rim/point light tint (the red glow) */
  glowColor: '#ff0000',
  /** depth fade: the far side of the wireframe fades toward this colour (the
   *  page background), giving the globe a soft, faded feel. near/far below. */
  fogColor: '#000000',
  fogNear: 3.6,
  fogFar: 6.8,
  /** OPTIONAL: imported image URL to map onto the sphere (null = wireframe look) */
  texture: null as string | null,
};

/** Light-theme overrides (merged over GLOBE_CONFIG when `light` is set).
 *  Light mode is the WHITE graph-paper math sphere (the site-reference look):
 *  the grid + notation live in the canvas texture, so there is NO red
 *  wireframe overlay in light mode; shading tints go neutral. Geometry,
 *  radius and camera are shared with dark mode — the size is identical. */
export const GLOBE_CONFIG_LIGHT: Partial<typeof GLOBE_CONFIG> = {
  sphereColor: '#f8f9f7',
  glowColor: '#cfd8dc',
  fogColor: '#f4f4f5',
};

type Cfg = typeof GLOBE_CONFIG;

const GlobeMesh: React.FC<{ cfg: Cfg; paperTexture?: THREE.Texture | null }> = ({ cfg, paperTexture }) => {
  const group = useRef<THREE.Group>(null);

  // Idle auto-rotation. Dragging (OrbitControls) orbits the camera on top of it.
  useFrame((_, delta) => {
    if (group.current) group.current.rotation.y += cfg.autoRotateSpeed * delta;
  });

  return (
    <group ref={group}>
      {/* base sphere — light mode: the WHITE graph-paper math sphere (grid +
          notation baked into the texture, soft clearcoat sheen); dark mode:
          the plain dark fill. Same geometry either way. */}
      <mesh>
        <sphereGeometry args={[cfg.radius, cfg.segments, cfg.segments]} />
        {paperTexture ? (
          <meshPhysicalMaterial
            color="#ffffff"
            map={paperTexture}
            roughness={0.4}
            metalness={0}
            clearcoat={0.85}
            clearcoatRoughness={0.18}
            envMapIntensity={0.5}
          />
        ) : (
          <meshStandardMaterial color={cfg.sphereColor} roughness={0.85} metalness={0.15} />
        )}
      </mesh>

      {/* lat/long wireframe overlay — DARK MODE ONLY. The light paper sphere
          carries its grid in the texture; a red cage over white paper is not
          the reference look. */}
      {!paperTexture && (
        <mesh scale={1.004}>
          <sphereGeometry args={[cfg.radius, cfg.segments, cfg.segments]} />
          <meshBasicMaterial color={cfg.wireColor} wireframe transparent opacity={cfg.wireOpacity} />
        </mesh>
      )}
    </group>
  );
};

export interface GlobeProps {
  light?: boolean;
}

/** The interactive globe. Transparent canvas; square container → true circle. */
const Globe: React.FC<GlobeProps> = ({ light = false }) => {
  const cfg: Cfg = light ? { ...GLOBE_CONFIG, ...GLOBE_CONFIG_LIGHT } : GLOBE_CONFIG;
  // Light mode: the WHITE graph-paper math sphere (site-reference look).
  const paperTexture = useMemo(() => (light ? makePaperMathTexture() : null), [light]);
  return (
    <Canvas
      camera={{ position: [0, 0, 4.6], fov: 42 }}
      dpr={[1, 2]}
      gl={{ alpha: true, antialias: true }}
      style={{ width: '100%', height: '100%', display: 'block' }}
      data-testid="landing-globe"
    >
      {/* depth fade toward the page background — softens the far side */}
      <fog attach="fog" args={[cfg.fogColor, cfg.fogNear, cfg.fogFar]} />
      {/* light mode: soft, even studio-ish light so the paper reads white with
          a gentle top-left key — no red tint anywhere on the paper. */}
      <ambientLight intensity={light ? 1.45 : 0.6} />
      <directionalLight position={[3, 2, 4]} intensity={light ? 1.1 : 1.1} />
      <pointLight position={[-3, -2, -2]} intensity={light ? 0.35 : 0.7} color={cfg.glowColor} />
      {/* light mode ONLY: bundled studio HDR gives the paper a premium glossy
          highlight (the reference's rendered-sphere sheen) — reflections only,
          never drawn as background. Suspense keeps a slow/absent HDR from
          blanking the sphere; dark mode is untouched (no env map). */}
      {light && (
        <Suspense fallback={null}>
          <Environment
            files={`${process.env.PUBLIC_URL}/hdr/studio_small_03_1k.hdr`}
            background={false}
          />
        </Suspense>
      )}
      <GlobeMesh cfg={cfg} paperTexture={paperTexture} />
      {/* Interactive: drag to rotate. No zoom/pan so it stays centred. */}
      <OrbitControls enablePan={false} enableZoom={false} rotateSpeed={0.6} />
    </Canvas>
  );
};

export default Globe;
