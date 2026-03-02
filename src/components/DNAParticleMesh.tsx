import React, { useRef, useMemo, useEffect } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import * as THREE from 'three';

const NUM_POINTS = 1200;
const BOX_W = 30;
const BOX_H = 30;
const BOX_D = 10;
const CONNECT_DISTANCE = 2;
const ROTATION_SPEED = 0.0005;

function ParticleMeshContent({
  mouseRef,
  isLight,
}: {
  mouseRef: React.MutableRefObject<{ x: number; y: number }>;
  isLight: boolean;
}) {
  const groupRef = useRef<THREE.Group>(null);
  const meshRef = useRef<THREE.Points>(null);
  const linesRef = useRef<THREE.LineSegments>(null);

  const { positions, linePositions } = useMemo(() => {
    const pos = new Float32Array(NUM_POINTS * 3);
    const points: THREE.Vector3[] = [];
    for (let i = 0; i < NUM_POINTS; i++) {
      const x = (Math.random() - 0.5) * BOX_W;
      const y = (Math.random() - 0.5) * BOX_H;
      const z = (Math.random() - 0.5) * BOX_D;
      pos[i * 3] = x;
      pos[i * 3 + 1] = y;
      pos[i * 3 + 2] = z;
      points.push(new THREE.Vector3(x, y, z));
    }
    const linePos: number[] = [];
    const maxConnections = 8;
    for (let i = 0; i < points.length; i++) {
      let count = 0;
      for (let j = i + 1; j < points.length && count < maxConnections; j++) {
        if (points[i].distanceTo(points[j]) < CONNECT_DISTANCE) {
          linePos.push(
            points[i].x, points[i].y, points[i].z,
            points[j].x, points[j].y, points[j].z
          );
          count++;
        }
      }
    }
    const lineGeo = new THREE.BufferAttribute(new Float32Array(linePos), 3);
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(pos, 3));
    geom.computeBoundingSphere();
    const lineGeom = new THREE.BufferGeometry();
    lineGeom.setAttribute('position', lineGeo);
    lineGeom.computeBoundingSphere();
    return { positions: geom, linePositions: lineGeom };
  }, []);

  useFrame(() => {
    if (!groupRef.current) return;
    groupRef.current.rotation.y += ROTATION_SPEED;
    const mx = mouseRef.current.x * 0.02;
    const my = mouseRef.current.y * 0.02;
    groupRef.current.rotation.x = THREE.MathUtils.lerp(groupRef.current.rotation.x, my, 0.03);
    groupRef.current.rotation.z = THREE.MathUtils.lerp(groupRef.current.rotation.z, -mx, 0.03);
  });

  const opacity = isLight ? 0.3 : 1;

  return (
    <group ref={groupRef}>
      <points ref={meshRef} geometry={positions}>
        <pointsMaterial
          size={0.05}
          color="#8b5cf6"
          transparent
          opacity={0.6 * opacity}
          sizeAttenuation
        />
      </points>
      <lineSegments ref={linesRef} geometry={linePositions}>
        <lineBasicMaterial color="#8b5cf6" transparent opacity={0.08 * opacity} />
      </lineSegments>
    </group>
  );
}

export default function DNAParticleMesh({
  className = '',
  style = {},
  isLight = false,
}: {
  className?: string;
  style?: React.CSSProperties;
  isLight?: boolean;
}) {
  const mouseRef = useRef({ x: 0, y: 0 });

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      const w = window.innerWidth;
      const h = window.innerHeight;
      mouseRef.current.x = (e.clientX / w) * 2 - 1;
      mouseRef.current.y = -(e.clientY / h) * 2 + 1;
    };
    window.addEventListener('mousemove', onMove);
    return () => window.removeEventListener('mousemove', onMove);
  }, []);

  return (
    <div
      className={className}
      style={{
        position: 'absolute',
        inset: 0,
        zIndex: 0,
        pointerEvents: 'none',
        ...style,
      }}
      aria-hidden="true"
    >
      <Canvas
        camera={{ position: [0, 0, 25], fov: 60 }}
        gl={{ alpha: true, antialias: true, powerPreference: 'high-performance' }}
        dpr={Math.min(typeof window !== 'undefined' ? window.devicePixelRatio : 1, 2)}
        style={{ width: '100%', height: '100%', background: 'transparent' }}
      >
        <color attach="background" args={['transparent']} />
        <ParticleMeshContent mouseRef={mouseRef} isLight={isLight} />
      </Canvas>
    </div>
  );
}
