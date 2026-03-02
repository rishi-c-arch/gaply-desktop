import React, { useRef, useMemo, useEffect } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import * as THREE from 'three';

const PARTICLES_PER_STRAND = 200;
const RUNG_INTERVAL = 10;
const ROTATION_SPEED = 0.003;
const RADIUS = 2;
const HEIGHT = 8;
const TURNS = 3;

function HelixContent({
  mouseRef,
  isLight,
}: {
  mouseRef: React.MutableRefObject<{ x: number; y: number }>;
  isLight: boolean;
}) {
  const groupRef = useRef<THREE.Group>(null);
  const strand1Ref = useRef<THREE.InstancedMesh>(null);
  const strand2Ref = useRef<THREE.InstancedMesh>(null);
  const linesRef = useRef<THREE.LineSegments>(null);

  const { positions1, positions2, rungPairs } = useMemo(() => {
    const p1: [number, number, number][] = [];
    const p2: [number, number, number][] = [];
    const rungs: [THREE.Vector3, THREE.Vector3][] = [];
    for (let i = 0; i < PARTICLES_PER_STRAND; i++) {
      const t = i / (PARTICLES_PER_STRAND - 1);
      const y = (t - 0.5) * HEIGHT;
      const angle1 = t * TURNS * Math.PI * 2;
      const angle2 = angle1 + Math.PI;
      p1.push([
        RADIUS * Math.cos(angle1),
        y,
        RADIUS * Math.sin(angle1),
      ]);
      p2.push([
        RADIUS * Math.cos(angle2),
        y,
        RADIUS * Math.sin(angle2),
      ]);
      if (i % RUNG_INTERVAL === 0) {
        rungs.push([
          new THREE.Vector3(RADIUS * Math.cos(angle1), y, RADIUS * Math.sin(angle1)),
          new THREE.Vector3(RADIUS * Math.cos(angle2), y, RADIUS * Math.sin(angle2)),
        ]);
      }
    }
    return {
      positions1: p1,
      positions2: p2,
      rungPairs: rungs,
    };
  }, []);

  const lineGeometry = useMemo(() => {
    const geom = new THREE.BufferGeometry();
    const positions = new Float32Array(rungPairs.length * 2 * 3);
    rungPairs.forEach((pair, i) => {
      const [v0, v1] = pair;
      positions[i * 6] = v0.x;
      positions[i * 6 + 1] = v0.y;
      positions[i * 6 + 2] = v0.z;
      positions[i * 6 + 3] = v1.x;
      positions[i * 6 + 4] = v1.y;
      positions[i * 6 + 5] = v1.z;
    });
    geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geom.computeBoundingSphere();
    return geom;
  }, [rungPairs]);

  const dummy = useMemo(() => new THREE.Object3D(), []);

  useEffect(() => {
    if (!strand1Ref.current) return;
    positions1.forEach((pos, i) => {
      dummy.position.set(pos[0], pos[1], pos[2]);
      dummy.updateMatrix();
      strand1Ref.current!.setMatrixAt(i, dummy.matrix);
    });
    strand1Ref.current.instanceMatrix.needsUpdate = true;
  }, [positions1, dummy]);

  useEffect(() => {
    if (!strand2Ref.current) return;
    positions2.forEach((pos, i) => {
      dummy.position.set(pos[0], pos[1], pos[2]);
      dummy.updateMatrix();
      strand2Ref.current!.setMatrixAt(i, dummy.matrix);
    });
    strand2Ref.current.instanceMatrix.needsUpdate = true;
  }, [positions2, dummy]);

  useFrame(() => {
    if (!groupRef.current) return;
    groupRef.current.rotation.y += ROTATION_SPEED;
    const mx = mouseRef.current.x * 0.15;
    const my = mouseRef.current.y * 0.15;
    groupRef.current.rotation.x = THREE.MathUtils.lerp(groupRef.current.rotation.x, my, 0.05);
    groupRef.current.rotation.z = THREE.MathUtils.lerp(groupRef.current.rotation.z, -mx, 0.05);
  });

  const purple = new THREE.Color('#8b5cf6');
  const violet = new THREE.Color('#c084fc');
  const opacity = isLight ? 0.4 : 1;

  return (
    <group ref={groupRef}>
      <instancedMesh ref={strand1Ref} args={[undefined, undefined, PARTICLES_PER_STRAND]}>
        <sphereGeometry args={[0.04, 8, 8]} />
        <meshBasicMaterial color={purple} transparent opacity={opacity} />
      </instancedMesh>
      <instancedMesh ref={strand2Ref} args={[undefined, undefined, PARTICLES_PER_STRAND]}>
        <sphereGeometry args={[0.04, 8, 8]} />
        <meshBasicMaterial color={violet} transparent opacity={opacity} />
      </instancedMesh>
      <lineSegments ref={linesRef} geometry={lineGeometry}>
        <lineBasicMaterial color="#8b5cf6" transparent opacity={0.25} />
      </lineSegments>
    </group>
  );
}

export default function DNAHelix({
  className = '',
  style = {},
  isLight = false,
}: {
  className?: string;
  style?: React.CSSProperties;
  isLight?: boolean;
}) {
  const mouseRef = useRef({ x: 0, y: 0 });
  const containerRef = useRef<HTMLDivElement>(null);

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
      ref={containerRef}
      className={className}
      style={{
        position: 'absolute',
        width: 400,
        height: 600,
        left: 0,
        top: '50%',
        transform: 'translateY(-50%)',
        ...style,
      }}
      aria-hidden="true"
    >
      <Canvas
        camera={{ position: [0, 0, 14], fov: 45 }}
        gl={{ alpha: true, antialias: true, powerPreference: 'high-performance' }}
        dpr={Math.min(typeof window !== 'undefined' ? window.devicePixelRatio : 1, 2)}
        style={{ width: '100%', height: '100%', background: 'transparent' }}
      >
        <color attach="background" args={['transparent']} />
        <HelixContent mouseRef={mouseRef} isLight={isLight} />
      </Canvas>
    </div>
  );
}
