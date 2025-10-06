import React, { useRef, useState } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls, Environment } from '@react-three/drei';
import { Mesh } from 'three';

const MagnifyingGlass: React.FC = () => {
  const meshRef = useRef<Mesh>(null);
  const [hovered, setHovered] = useState(false);

  useFrame((state) => {
    if (meshRef.current) {
      // Gentle floating animation
      meshRef.current.position.y = Math.sin(state.clock.elapsedTime * 0.5) * 0.1;
      // Slow rotation
      meshRef.current.rotation.y += 0.005;
    }
  });

  return (
    <group ref={meshRef}>
      {/* Magnifying Glass Body */}
      <mesh 
        position={[0, 0, 0]}
        onPointerOver={() => setHovered(true)}
        onPointerOut={() => setHovered(false)}
      >
        <cylinderGeometry args={[0.8, 0.6, 0.2, 32]} />
        <meshStandardMaterial 
          color={hovered ? "#0A84FF" : "#ffffff"} 
          metalness={0.8}
          roughness={0.2}
          transparent={true}
          opacity={0.9}
        />
      </mesh>
      
      {/* Glass Lens */}
      <mesh position={[0, 0, 0.11]}>
        <cylinderGeometry args={[0.75, 0.75, 0.05, 32]} />
        <meshStandardMaterial 
          color="#87CEEB" 
          transparent={true}
          opacity={0.3}
          roughness={0.1}
        />
      </mesh>
      
      {/* Handle */}
      <mesh position={[-0.7, 0, 0]} rotation={[0, 0, Math.PI / 6]}>
        <cylinderGeometry args={[0.08, 0.12, 2, 8]} />
        <meshStandardMaterial 
          color="#8B4513"
          roughness={0.8}
        />
      </mesh>
      
      {/* Light reflection */}
      <mesh position={[0.2, 0.2, 0.12]}>
        <sphereGeometry args={[0.1, 16, 16]} />
        <meshStandardMaterial 
          color="#ffffff"
          transparent={true}
          opacity={0.6}
        />
      </mesh>
    </group>
  );
};

const MagnifyingGlass3D: React.FC = () => {
  return (
    <div className="magnifying-glass-3d">
      <Canvas
        camera={{ position: [0, 0, 5], fov: 50 }}
        style={{ width: '100%', height: '100%' }}
      >
        <ambientLight intensity={0.6} />
        <directionalLight position={[10, 10, 5]} intensity={1} />
        <pointLight position={[-10, -10, -5]} intensity={0.5} />
        
        <MagnifyingGlass />
        
        <OrbitControls 
          enableZoom={false}
          enablePan={false}
          autoRotate={true}
          autoRotateSpeed={0.5}
        />
        
        <Environment preset="studio" />
      </Canvas>
    </div>
  );
};

export default MagnifyingGlass3D;




