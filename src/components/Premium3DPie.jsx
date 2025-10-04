import React, { useRef, useMemo, useEffect, useState, Suspense } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { Html, Environment, OrbitControls } from '@react-three/drei';
import gsap from 'gsap';
import './Premium3DPie.css';

/**
 * Premium3DPie - Professional 3D Pie Chart Component
 * 
 * Features:
 * - 3D extruded pie slices with premium materials
 * - Smooth hover animations and interactions
 * - Professional legend with bullet points
 * - WebGL fallback to SVG
 * - Responsive design
 * - Click to download reports
 */

function createSliceShape(innerR, outerR, startAngle, endAngle, segments = 64) {
  const shape = new THREE.Shape();

  // Outer arc clockwise
  for (let i = 0; i <= segments; i++) {
    const t = i / segments;
    const angle = startAngle + (endAngle - startAngle) * t;
    const x = Math.cos(angle) * outerR;
    const y = Math.sin(angle) * outerR;
    if (i === 0) shape.moveTo(x, y);
    else shape.lineTo(x, y);
  }

  // Inner arc counter-clockwise (hole)
  const holePath = new THREE.Path();
  for (let i = 0; i <= segments; i++) {
    const t = i / segments;
    const angle = endAngle - (endAngle - startAngle) * t;
    const x = Math.cos(angle) * innerR;
    const y = Math.sin(angle) * innerR;
    if (i === 0) holePath.moveTo(x, y);
    else holePath.lineTo(x, y);
  }
  shape.holes.push(holePath);
  return shape;
}

function Slice({
  index, startAngle, endAngle, outerR, innerR, height, color, label, value,
  explodeOffset = 0.25, animateDelay = 0.08, onHoverChange = () => {}, onSliceClick = () => {}
}) {
  const meshRef = useRef();
  const groupRef = useRef();
  const [hovered, setHovered] = useState(false);
  const midAngle = (startAngle + endAngle) / 2;
  const outward = new THREE.Vector3(Math.cos(midAngle), Math.sin(midAngle), 0).multiplyScalar(explodeOffset);

  // Create geometry once
  const geom = useMemo(() => {
    const shape = createSliceShape(innerR, outerR, startAngle, endAngle, 64);
    const extrudeSettings = { depth: height, bevelEnabled: true, bevelThickness: 0.02, bevelSize: 0.02, steps: 2 };
    return new THREE.ExtrudeGeometry(shape, extrudeSettings);
  }, [innerR, outerR, startAngle, endAngle, height]);

  // Center geometry
  useEffect(() => {
    geom.computeBoundingBox?.();
    const bbox = geom.boundingBox;
    if (bbox) {
      const centerX = (bbox.min.x + bbox.max.x) / 2;
      const centerY = (bbox.min.y + bbox.max.y) / 2;
      geom.translate(-centerX, -centerY, -height / 2);
    }
  }, [geom, height]);

  // Mount animation (staggered pop)
  useEffect(() => {
    const mesh = meshRef.current;
    if (!mesh) return;
    
    mesh.scale.set(0.001, 0.001, 0.001);
    gsap.to(mesh.scale, { 
      x: 1, y: 1, z: 1, 
      duration: 0.8, 
      ease: 'back.out(1.4)', 
      delay: index * animateDelay 
    });
  }, [index, animateDelay]);

  // Hover effect
  useEffect(() => {
    const mesh = meshRef.current;
    const group = groupRef.current;
    if (!mesh || !group) return;
    
    if (hovered) {
      gsap.to(group.position, { 
        x: outward.x, y: outward.y, z: 0.15, 
        duration: 0.3, 
        ease: 'power2.out' 
      });
      gsap.to(mesh.material, { 
        metalness: 0.8, 
        roughness: 0.1, 
        emissive: new THREE.Color(color).multiplyScalar(0.1),
        duration: 0.25 
      });
      onHoverChange({ label, value, color });
    } else {
      gsap.to(group.position, { 
        x: 0, y: 0, z: 0, 
        duration: 0.4, 
        ease: 'power3.out' 
      });
      gsap.to(mesh.material, { 
        metalness: 0.3, 
        roughness: 0.4, 
        emissive: new THREE.Color(0x000000),
        duration: 0.4 
      });
      onHoverChange(null);
    }
  }, [hovered, outward, onHoverChange, label, value, color]);

  const handlePointerOver = (e) => { 
    e.stopPropagation(); 
    setHovered(true); 
  };
  
  const handlePointerOut = (e) => { 
    e.stopPropagation(); 
    setHovered(false); 
  };

  const handleClick = () => {
    onSliceClick({ label, value, color });
  };

  return (
    <group ref={groupRef}>
      <mesh
        ref={meshRef}
        geometry={geom}
        onPointerOver={handlePointerOver}
        onPointerOut={handlePointerOut}
        onClick={handleClick}
        castShadow
        receiveShadow
      >
        <meshPhysicalMaterial
          attach="material"
          color={color}
          metalness={0.3}
          roughness={0.4}
          clearcoat={0.1}
          clearcoatRoughness={0.3}
          reflectivity={0.5}
          emissive={new THREE.Color(0x000000)}
        />
      </mesh>
    </group>
  );
}

function PieScene({ data = [], radius = 1.8, innerRadius = 1.0, height = 0.4, rotateSpeed = 0.015, onHoverChange, onSliceClick }) {
  const groupRef = useRef();

  // Idle rotation
  useFrame((state, delta) => {
    if (groupRef.current) groupRef.current.rotation.y += rotateSpeed * delta;
  });

  // Compute angles
  const total = data.reduce((s, d) => s + d.value, 0) || 1;
  let start = -Math.PI / 2;
  const slices = data.map((d, i) => {
    const angle = (d.value / total) * Math.PI * 2;
    const slice = { 
      startAngle: start, 
      endAngle: start + angle, 
      color: d.color, 
      label: d.label, 
      value: d.value 
    };
    start += angle;
    return slice;
  });

  return (
    <group ref={groupRef} rotation={[Math.PI * 0.05, 0, 0]}>
      {slices.map((s, i) => (
        <Slice
          key={i}
          index={i}
          startAngle={s.startAngle}
          endAngle={s.endAngle}
          outerR={radius}
          innerR={innerRadius}
          height={height}
          color={s.color}
          label={s.label}
          value={s.value}
          onHoverChange={onHoverChange}
          onSliceClick={onSliceClick}
        />
      ))}
    </group>
  );
}

export default function Premium3DPie({
  data = [
    { label: 'MEDICINE & CLINICAL SCIENCES', value: 16.67, color: '#1a1a1a' },
    { label: 'LIFE SCIENCES & MOLECULAR BIOLOGY', value: 16.67, color: '#404040' },
    { label: 'CHEMISTRY', value: 16.67, color: '#606060' },
    { label: 'PHYSICS & ASTRONOMY', value: 16.67, color: '#808080' },
    { label: 'COMPUTER SCIENCE & AI', value: 16.67, color: '#a0a0a0' },
    { label: 'ENGINEERING', value: 16.65, color: '#c0c0c0' }
  ],
  width = 680,
  height = 480,
  radius = 1.8,
  innerRadius = 1.0,
  depth = 0.4,
  rotateSpeed = 0.015,
  showLegend = true,
  onSliceClick = () => {}
}) {
  const [hoverInfo, setHoverInfo] = useState(null);
  const [glAvailable, setGlAvailable] = useState(true);

  useEffect(() => {
    try {
      const canvas = document.createElement('canvas');
      const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
      if (!gl) setGlAvailable(false);
    } catch (e) {
      setGlAvailable(false);
    }
  }, []);

  // 2D fallback as SVG donut
  const SVGFallback = (
    <div className="ppt-fallback">
      <svg viewBox="-50 -50 100 100" width="100%" height="100%" preserveAspectRatio="xMidYMid meet">
        <g transform="rotate(-90)">
          {(() => {
            const total = data.reduce((s, d) => s + d.value, 0) || 1;
            let startAngle = 0;
            return data.map((d, i) => {
              const angle = (d.value / total) * Math.PI * 2;
              const large = angle > Math.PI ? 1 : 0;
              const r = 40;
              const innerR = 22;
              const x1 = Math.cos(startAngle) * r;
              const y1 = Math.sin(startAngle) * r;
              const x2 = Math.cos(startAngle + angle) * r;
              const y2 = Math.sin(startAngle + angle) * r;
              const xi1 = Math.cos(startAngle) * innerR;
              const yi1 = Math.sin(startAngle) * innerR;
              const xi2 = Math.cos(startAngle + angle) * innerR;
              const yi2 = Math.sin(startAngle + angle) * innerR;
              const path = `M ${xi1} ${yi1} L ${x1} ${y1} A ${r} ${r} 0 ${large} 1 ${x2} ${y2} L ${xi2} ${yi2} A ${innerR} ${innerR} 0 ${large} 0 ${xi1} ${yi1} Z`;
              startAngle += angle;
              return (
                <path 
                  key={i} 
                  d={path} 
                  fill={d.color} 
                  stroke="#ffffff" 
                  strokeWidth="0.8"
                  onClick={() => onSliceClick(d)}
                  style={{ cursor: 'pointer' }}
                />
              );
            });
          })()}
        </g>
      </svg>
      {showLegend && (
        <div className="ppt-legend">
          {data.map((d, i) => (
            <div className="ppt-legend-item" key={i} onClick={() => onSliceClick(d)}>
              <span className="ppt-legend-swatch" style={{ background: d.color }} />
              <span className="ppt-legend-text">{d.label} ({d.value.toFixed(0)}%)</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );

  return (
    <div className="ppt-wrapper" style={{ width: '100%', maxWidth: width, margin: '0 auto' }}>
      {!glAvailable ? (
        SVGFallback
      ) : (
        <div className="ppt-canvas-wrap" style={{ width: '100%', height }}>
          <Canvas
            shadows
            camera={{ position: [0, 0, 7], fov: 45 }}
            style={{ width: '100%', height: '100%', display: 'block' }}
          >
            <ambientLight intensity={0.6} />
            <directionalLight position={[5, 5, 5]} intensity={1.2} castShadow />
            <pointLight position={[-5, -5, 5]} intensity={0.5} />
            <Suspense fallback={null}>
              <Environment preset="studio" />
              <PieScene 
                data={data} 
                radius={radius} 
                innerRadius={innerRadius} 
                height={depth} 
                rotateSpeed={rotateSpeed} 
                onHoverChange={(h) => setHoverInfo(h)} 
                onSliceClick={onSliceClick}
              />
            </Suspense>
            <OrbitControls 
              enableZoom={false} 
              enablePan={false} 
              rotateSpeed={0.3} 
              minPolarAngle={Math.PI / 2 - 0.7} 
              maxPolarAngle={Math.PI / 2 + 0.7} 
            />
          </Canvas>

          {/* Overlay info/legend */}
          <div className="ppt-overlay">
            {showLegend && (
              <div className="ppt-legend">
                {data.map((d, i) => (
                  <div className="ppt-legend-item" key={i} onClick={() => onSliceClick(d)}>
                    <span className="ppt-legend-swatch" style={{ background: d.color }} />
                    <span className="ppt-legend-text">{d.label} ({d.value.toFixed(0)}%)</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
