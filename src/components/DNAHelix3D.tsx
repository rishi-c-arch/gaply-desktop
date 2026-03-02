import React, { useRef, useMemo } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import * as THREE from 'three';

export interface DNAHelix3DProps {
  position: 'left' | 'right' | 'background';
  colors: {
    strand1: string;
    strand2: string;
    particles: string;
    connectors?: string;
  };
  geometry: {
    radius: number;
    height: number;
    turns: number;
    tubeRadius: number;
  };
  particles: {
    count: number;
    size: number;
    glowIntensity: number;
  };
  animation: {
    autoRotate: boolean;
    rotationSpeed: number;
    particleOrbit?: boolean;
  };
  opacity?: number;
  /** When false, pause animation (e.g. when not in view). */
  inView?: boolean;
  /** Reduce particle count (e.g. from useMediaQuery). */
  particleCountOverride?: number;
  className?: string;
  style?: React.CSSProperties;
}

function HelixGroup({
  colors,
  geometry: geom,
  particles: particleConfig,
  animation,
  inView,
  particleCountOverride,
}: {
  colors: DNAHelix3DProps['colors'];
  geometry: DNAHelix3DProps['geometry'];
  particles: DNAHelix3DProps['particles'];
  animation: DNAHelix3DProps['animation'];
  inView: boolean;
  particleCountOverride?: number;
}) {
  const groupRef = useRef<THREE.Group>(null);
  const count = particleCountOverride ?? particleConfig.count;

  const { tubeGeo1, tubeGeo2, particlePositions } = useMemo(() => {
    const points1: THREE.Vector3[] = [];
    const points2: THREE.Vector3[] = [];
    const res = 64;
    for (let i = 0; i <= res; i++) {
      const t = i / res;
      const angle = t * geom.turns * Math.PI * 2;
      points1.push(new THREE.Vector3(
        geom.radius * Math.cos(angle),
        (t - 0.5) * geom.height,
        geom.radius * Math.sin(angle)
      ));
      points2.push(new THREE.Vector3(
        geom.radius * Math.cos(angle + Math.PI),
        (t - 0.5) * geom.height,
        geom.radius * Math.sin(angle + Math.PI)
      ));
    }
    const curve1 = new THREE.CatmullRomCurve3(points1);
    const curve2 = new THREE.CatmullRomCurve3(points2);
    const tubeGeo1 = new THREE.TubeGeometry(curve1, 128, geom.tubeRadius, 8, false);
    const tubeGeo2 = new THREE.TubeGeometry(curve2, 128, geom.tubeRadius, 8, false);

    const positions: [number, number, number][] = [];
    for (let i = 0; i < count; i++) {
      const t = i / (count - 1 || 1);
      const angle = t * geom.turns * Math.PI * 2;
      if (i % 2 === 0) {
        positions.push([
          geom.radius * Math.cos(angle),
          (t - 0.5) * geom.height,
          geom.radius * Math.sin(angle),
        ]);
      } else {
        positions.push([
          geom.radius * Math.cos(angle + Math.PI),
          (t - 0.5) * geom.height,
          geom.radius * Math.sin(angle + Math.PI),
        ]);
      }
    }

    return { tubeGeo1, tubeGeo2, particlePositions: positions };
  }, [geom.radius, geom.height, geom.turns, geom.tubeRadius, count]);

  useFrame(() => {
    if (!groupRef.current || !inView || !animation.autoRotate) return;
    groupRef.current.rotation.y += animation.rotationSpeed;
  });

  const col1 = useMemo(() => new THREE.Color(colors.strand1), [colors.strand1]);
  const col2 = useMemo(() => new THREE.Color(colors.strand2), [colors.strand2]);
  const colP = useMemo(() => new THREE.Color(colors.particles), [colors.particles]);

  return (
    <group ref={groupRef}>
      <mesh geometry={tubeGeo1}>
        <meshBasicMaterial color={col1} transparent opacity={1} />
      </mesh>
      <mesh geometry={tubeGeo2}>
        <meshBasicMaterial color={col2} transparent opacity={1} />
      </mesh>
      {particlePositions.map((pos, i) => (
        <mesh key={i} position={pos}>
          <sphereGeometry args={[particleConfig.size, 8, 8]} />
          <meshBasicMaterial color={colP} transparent opacity={particleConfig.glowIntensity} />
        </mesh>
      ))}
    </group>
  );
}

export default function DNAHelix3D({
  position,
  colors,
  geometry,
  particles,
  animation,
  opacity = 1,
  inView = true,
  particleCountOverride,
  className = '',
  style = {},
}: DNAHelix3DProps) {
  const isBackground = position === 'background';
  const height = isBackground ? 800 : position === 'left' ? 500 : 600;
  const width = isBackground ? '100%' : '40%';

  return (
    <div
      className={`dna-helix-wrap dna-helix-wrap--${position} ${className}`}
      style={{
        width,
        height: `${height}px`,
        position: isBackground ? 'absolute' : 'relative',
        left: isBackground ? 0 : undefined,
        top: isBackground ? '50%' : undefined,
        transform: isBackground ? 'translateY(-50%)' : undefined,
        opacity: isBackground ? opacity : 1,
        pointerEvents: 'none',
        willChange: 'transform',
        ...style,
      }}
      aria-hidden="true"
      role="presentation"
    >
      <Canvas
        camera={{ position: [0, 0, 12], fov: 45 }}
        dpr={typeof window !== 'undefined' ? Math.min(window.devicePixelRatio, 2) : 1}
        gl={{ alpha: true, antialias: true, powerPreference: 'high-performance' }}
        style={{ width: '100%', height: '100%', background: 'transparent' }}
      >
        <color attach="background" args={['transparent']} />
        <HelixGroup
          colors={colors}
          geometry={geometry}
          particles={particles}
          animation={animation}
          inView={inView}
          particleCountOverride={particleCountOverride}
        />
      </Canvas>
    </div>
  );
}
