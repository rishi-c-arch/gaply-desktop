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
import React, { useMemo, useRef } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import * as THREE from 'three';

// Formulas / algorithm snippets drawn onto the light-mode sphere. Standard
// stats / info-theory notation — edit freely.
const FORMULAS = [
  'p = 0.002', 't(47) = 3.2', 'd = 0.46', 'χ² = Σ (O−E)²/E', 'r = cov(X,Y)/σx σy',
  'H = −Σ p·log p', 'ppl = 2^H', 'cos = A·B / |A||B|', 'tfidf = tf·log(N/df)',
  'CI = x̄ ± z·σ/√n', 'F = MSB / MSW', 'z = (x−μ)/σ', 'R² = 1 − SSr/SSt', 'n = 48',
  'sha256(doc)', 'argmax Σ wᵢ·cᵢ', 'if p < 0.05: flag()', 'verify(ref) → {found}',
  'sim > 0.85 → dup', 'β = 1 − power', 'σ² = Σ(x−x̄)²/n', 'OR = (a/b)/(c/d)',
];

/** Build a black texture with formulas written across it, for the light-mode
 *  sphere ("black sphere with algorithms/formulas"). */
function makeFormulaTexture(): THREE.Texture {
  const w = 2048;
  const h = 1024;
  const c = document.createElement('canvas');
  c.width = w;
  c.height = h;
  const ctx = c.getContext('2d')!;
  ctx.fillStyle = '#050505';
  ctx.fillRect(0, 0, w, h);
  ctx.font = '34px "Source Code Pro", ui-monospace, monospace';
  ctx.textBaseline = 'top';
  const rowH = 50;
  for (let row = 0, i = 0; row * rowH < h; row++) {
    let x = 18 - ((row % 3) * 70); // stagger rows
    const y = row * rowH + 8;
    while (x < w) {
      const text = FORMULAS[i % FORMULAS.length];
      i++;
      // bright legible text, occasional bold red accent
      ctx.fillStyle = i % 5 === 0 ? 'rgba(255,74,74,0.98)' : 'rgba(236,238,243,0.94)';
      ctx.fillText(text, x, y);
      x += ctx.measureText(text).width + 52;
    }
  }
  const tex = new THREE.CanvasTexture(c);
  tex.wrapS = THREE.RepeatWrapping;
  tex.wrapT = THREE.RepeatWrapping;
  tex.repeat.set(2, 1);
  tex.anisotropy = 8;
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

/** Light-theme overrides (merged over GLOBE_CONFIG when `light` is set). The
 *  wireframe keeps the SAME red as the dark version; only the sphere fill (so it
 *  reads as hollow on white) and the fade colour change with the background. */
export const GLOBE_CONFIG_LIGHT: Partial<typeof GLOBE_CONFIG> = {
  sphereColor: '#eef0f4',
  wireColor: '#ff2a2a',
  wireOpacity: 0.45,
  glowColor: '#ff0000',
  fogColor: '#f4f4f5',
};

type Cfg = typeof GLOBE_CONFIG;

const GlobeMesh: React.FC<{ cfg: Cfg; formulaTexture?: THREE.Texture | null }> = ({ cfg, formulaTexture }) => {
  const group = useRef<THREE.Group>(null);

  // Idle auto-rotation. Dragging (OrbitControls) orbits the camera on top of it.
  useFrame((_, delta) => {
    if (group.current) group.current.rotation.y += cfg.autoRotateSpeed * delta;
  });

  return (
    <group ref={group}>
      {/* base sphere — light mode: BLACK sphere with formulas/algorithms mapped
          onto it; dark mode: the plain dark fill. */}
      <mesh>
        <sphereGeometry args={[cfg.radius, cfg.segments, cfg.segments]} />
        {formulaTexture ? (
          // color MUST be white: meshStandardMaterial multiplies color × map, so
          // a black base would erase the formula text. The sphere reads black
          // from the texture's own near-black (#050505) background.
          <meshStandardMaterial color="#ffffff" map={formulaTexture} roughness={0.9} metalness={0.05} />
        ) : (
          <meshStandardMaterial color={cfg.sphereColor} roughness={0.85} metalness={0.15} />
        )}
      </mesh>

      {/* lat/long wireframe overlay. Dark mode: the dense mesh. Formula (light)
          mode: a cleaner, coarser red lat/long grid so the red lines stay
          clearly visible AND the formulas read through the gaps. */}
      <mesh scale={1.004}>
        <sphereGeometry
          args={[cfg.radius, formulaTexture ? 40 : cfg.segments, formulaTexture ? 26 : cfg.segments]}
        />
        <meshBasicMaterial
          color={formulaTexture ? '#ff1a1a' : cfg.wireColor}
          wireframe
          transparent
          opacity={formulaTexture ? 0.55 : cfg.wireOpacity}
        />
      </mesh>
    </group>
  );
};

export interface GlobeProps {
  light?: boolean;
}

/** The interactive globe. Transparent canvas; square container → true circle. */
const Globe: React.FC<GlobeProps> = ({ light = false }) => {
  const cfg: Cfg = light ? { ...GLOBE_CONFIG, ...GLOBE_CONFIG_LIGHT } : GLOBE_CONFIG;
  // Light mode: a BLACK sphere with formulas/algorithms written on it.
  const formulaTexture = useMemo(() => (light ? makeFormulaTexture() : null), [light]);
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
      {/* brighter ambient in light mode so the formula text is legible */}
      <ambientLight intensity={light ? 1.6 : 0.6} />
      <directionalLight position={[3, 2, 4]} intensity={light ? 1.4 : 1.1} />
      <pointLight position={[-3, -2, -2]} intensity={0.7} color={cfg.glowColor} />
      <GlobeMesh cfg={cfg} formulaTexture={formulaTexture} />
      {/* Interactive: drag to rotate. No zoom/pan so it stays centred. */}
      <OrbitControls enablePan={false} enableZoom={false} rotateSpeed={0.6} />
    </Canvas>
  );
};

export default Globe;
