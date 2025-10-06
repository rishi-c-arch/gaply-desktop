import React, { useRef, useMemo } from 'react';
import { Canvas, useFrame, useLoader } from '@react-three/fiber';
import { Environment, useTexture } from '@react-three/drei';
import * as THREE from 'three';
import magnifyingGlassTexture from '../assets/magnifying-4340698.jpg';

// 3D Magnifying Glass Component for Login
const LoginMagnifyingGlass3D: React.FC = () => {
  const meshRef = useRef<THREE.Group>(null);
  const texture = useTexture(magnifyingGlassTexture);
  
  // Configure texture for premium quality
  texture.wrapS = texture.wrapT = THREE.RepeatWrapping;
  texture.anisotropy = 16;
  texture.minFilter = THREE.LinearMipmapLinearFilter;
  texture.magFilter = THREE.LinearFilter;
  texture.generateMipmaps = true;

  useFrame((state) => {
    if (meshRef.current) {
      // Gentle floating motion
      meshRef.current.position.y = Math.sin(state.clock.elapsedTime * 0.4) * 0.15;
      
      // Slow rotation
      meshRef.current.rotation.y += 0.0015;
      
      // Subtle scale pulsing
      const scale = 1 + Math.sin(state.clock.elapsedTime * 0.6) * 0.03;
      meshRef.current.scale.setScalar(scale);
    }
  });

  return (
    <group ref={meshRef} position={[0, 0, -2.5]}>
      {/* Main magnifying glass body */}
      <mesh position={[0, 0, 0]}>
        <cylinderGeometry args={[0.6, 0.5, 0.25, 32]} />
        <meshPhysicalMaterial 
          map={texture}
          metalness={0.8}
          roughness={0.2}
          clearcoat={1.0}
          clearcoatRoughness={0.1}
          transmission={0.1}
          thickness={0.2}
          ior={1.5}
          transparent
          opacity={0.85}
        />
      </mesh>
      
      {/* Glass lens */}
      <mesh position={[0, 0, 0.15]}>
        <cylinderGeometry args={[0.5, 0.5, 0.08, 64]} />
        <meshPhysicalMaterial 
          transmission={0.95}
          thickness={0.08}
          ior={1.5}
          roughness={0.0}
          metalness={0.0}
          clearcoat={1.0}
          clearcoatRoughness={0.0}
          transparent
          opacity={0.8}
        />
      </mesh>
      
      {/* Handle */}
      <mesh position={[-0.3, -0.6, 0]} rotation={[0, 0, Math.PI / 6]}>
        <cylinderGeometry args={[0.04, 0.04, 1.0, 16]} />
        <meshPhysicalMaterial 
          color="#8B4513"
          metalness={0.3}
          roughness={0.7}
        />
      </mesh>
    </group>
  );
};

// Floating Particles Component for Login
const LoginFloatingParticles: React.FC = () => {
  const particlesRef = useRef<THREE.Group>(null);
  
  const particles = useMemo(() => {
    return Array.from({ length: 30 }).map((_, i) => ({
      position: [
        Math.random() * 16 - 8,
        Math.random() * 16 - 8,
        Math.random() * 8 - 4
      ] as [number, number, number],
      scale: Math.random() * 0.3 + 0.05,
      speed: Math.random() * 0.015 + 0.008
    }));
  }, []);

  useFrame((state) => {
    if (particlesRef.current) {
      particlesRef.current.rotation.y += 0.0008;
      particlesRef.current.rotation.x += 0.0003;
    }
  });

  return (
    <group ref={particlesRef}>
      {particles.map((particle, i) => (
        <mesh key={i} position={particle.position}>
          <sphereGeometry args={[particle.scale, 8, 8]} />
          <meshStandardMaterial 
            color="#ff7a1a"
            transparent
            opacity={0.2}
            emissive="#ff7a1a"
            emissiveIntensity={0.1}
          />
        </mesh>
      ))}
    </group>
  );
};

// Premium Login Background Component
const PremiumLoginBackground: React.FC = () => {
  return (
    <div className="premium-login-background">
      <Canvas
        camera={{ position: [0, 0, 6], fov: 65 }}
        style={{ width: '100%', height: '100%' }}
        shadows
        gl={{ 
          antialias: true, 
          alpha: true,
          powerPreference: "high-performance",
          stencil: false,
          depth: true
        }}
        dpr={[1, 2]}
        performance={{ min: 0.5 }}
      >
        {/* Enhanced Lighting Setup */}
        <ambientLight intensity={0.25} />
        <directionalLight 
          position={[8, 8, 4]} 
          intensity={1.2}
          castShadow
          shadow-mapSize-width={4096}
          shadow-mapSize-height={4096}
        />
        <pointLight position={[-8, -8, -4]} intensity={0.8} color="#ff7a1a" />
        <pointLight position={[8, -8, 4]} intensity={0.6} color="#ffffff" />
        <hemisphereLight intensity={0.2} />
        
        {/* Main 3D Magnifying Glass */}
        <LoginMagnifyingGlass3D />
        
        {/* Floating Particles */}
        <LoginFloatingParticles />
        
        {/* High-quality Environment */}
        <Environment preset="studio" background={false} />
      </Canvas>
      
      {/* CSS Overlay Effects */}
      <div className="premium-overlay-effects">
        <div className="gradient-overlay"></div>
        <div className="light-rays"></div>
      </div>
    </div>
  );
};

export default PremiumLoginBackground;
