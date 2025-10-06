import React, { useRef, useMemo } from 'react';
import { Canvas, useFrame, useLoader } from '@react-three/fiber';
import { Environment, useTexture } from '@react-three/drei';
import * as THREE from 'three';
import magnifyingGlassTexture from '../assets/magnifying-4340698.jpg';

// 3D Magnifying Glass Component
const MagnifyingGlass3D: React.FC = () => {
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
      meshRef.current.position.y = Math.sin(state.clock.elapsedTime * 0.3) * 0.2;
      
      // Slow rotation
      meshRef.current.rotation.y += 0.002;
      
      // Subtle scale pulsing
      const scale = 1 + Math.sin(state.clock.elapsedTime * 0.5) * 0.05;
      meshRef.current.scale.setScalar(scale);
    }
  });

  return (
    <group ref={meshRef} position={[0, 0, -3]}>
      {/* Main magnifying glass body */}
      <mesh position={[0, 0, 0]}>
        <cylinderGeometry args={[0.8, 0.6, 0.3, 32]} />
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
          opacity={0.9}
        />
      </mesh>
      
      {/* Glass lens */}
      <mesh position={[0, 0, 0.2]}>
        <cylinderGeometry args={[0.7, 0.7, 0.1, 64]} />
        <meshPhysicalMaterial 
          transmission={0.95}
          thickness={0.1}
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
      <mesh position={[-0.4, -0.8, 0]} rotation={[0, 0, Math.PI / 6]}>
        <cylinderGeometry args={[0.05, 0.05, 1.2, 16]} />
        <meshPhysicalMaterial 
          color="#8B4513"
          metalness={0.3}
          roughness={0.7}
        />
      </mesh>
    </group>
  );
};

// Floating Particles Component
const FloatingParticles: React.FC = () => {
  const particlesRef = useRef<THREE.Group>(null);
  
  const particles = useMemo(() => {
    return Array.from({ length: 50 }).map((_, i) => ({
      position: [
        Math.random() * 20 - 10,
        Math.random() * 20 - 10,
        Math.random() * 10 - 5
      ] as [number, number, number],
      scale: Math.random() * 0.5 + 0.1,
      speed: Math.random() * 0.02 + 0.01
    }));
  }, []);

  useFrame((state) => {
    if (particlesRef.current) {
      particlesRef.current.rotation.y += 0.001;
      particlesRef.current.rotation.x += 0.0005;
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
            opacity={0.3}
            emissive="#ff7a1a"
            emissiveIntensity={0.2}
          />
        </mesh>
      ))}
    </group>
  );
};

// Premium Background Component
const PremiumSignupBackground: React.FC = () => {
  return (
    <div className="premium-signup-background">
      <Canvas
        camera={{ position: [0, 0, 8], fov: 60 }}
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
        <ambientLight intensity={0.2} />
        <directionalLight 
          position={[10, 10, 5]} 
          intensity={1.5}
          castShadow
          shadow-mapSize-width={4096}
          shadow-mapSize-height={4096}
        />
        <pointLight position={[-10, -10, -5]} intensity={1.0} color="#ff7a1a" />
        <pointLight position={[10, -10, 5]} intensity={0.8} color="#ffffff" />
        <hemisphereLight intensity={0.3} />
        
        {/* Main 3D Magnifying Glass */}
        <MagnifyingGlass3D />
        
        {/* Floating Particles */}
        <FloatingParticles />
        
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

export default PremiumSignupBackground;
