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
import React, { useRef } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import * as THREE from 'three';

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

const GlobeMesh: React.FC<{ cfg: Cfg }> = ({ cfg }) => {
  const group = useRef<THREE.Group>(null);

  // Idle auto-rotation. Dragging (OrbitControls) orbits the camera on top of it.
  useFrame((_, delta) => {
    if (group.current) group.current.rotation.y += cfg.autoRotateSpeed * delta;
  });

  return (
    <group ref={group}>
      {/* base sphere */}
      <mesh>
        <sphereGeometry args={[cfg.radius, cfg.segments, cfg.segments]} />
        {/* Reference drop-in: swap for a textured material when cfg.texture is set,
            e.g. <meshStandardMaterial map={useTexture(cfg.texture!)} /> */}
        <meshStandardMaterial color={cfg.sphereColor} roughness={0.85} metalness={0.15} />
      </mesh>

      {/* lat/long wireframe overlay */}
      <mesh scale={1.001}>
        <sphereGeometry args={[cfg.radius, cfg.segments, cfg.segments]} />
        <meshBasicMaterial color={cfg.wireColor} wireframe transparent opacity={cfg.wireOpacity} />
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
      <ambientLight intensity={light ? 0.9 : 0.6} />
      <directionalLight position={[3, 2, 4]} intensity={1.1} />
      <pointLight position={[-3, -2, -2]} intensity={0.7} color={cfg.glowColor} />
      <GlobeMesh cfg={cfg} />
      {/* Interactive: drag to rotate. No zoom/pan so it stays centred. */}
      <OrbitControls enablePan={false} enableZoom={false} rotateSpeed={0.6} />
    </Canvas>
  );
};

export default Globe;
