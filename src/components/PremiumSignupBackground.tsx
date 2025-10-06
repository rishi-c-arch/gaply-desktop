import React, { useRef, useMemo } from 'react';
import { Canvas, useFrame, useLoader } from '@react-three/fiber';
import { Environment, useTexture } from '@react-three/drei';
import * as THREE from 'three';
import magnifyingGlassTexture from '../assets/magnifying-4340698.jpg';

// Premium 3D Magnifying Glass Component
const MagnifyingGlass3D: React.FC = () => {
  const meshRef = useRef<THREE.Group>(null);
  const texture = useTexture(magnifyingGlassTexture);
  
  // Configure texture for ultra-premium quality
  texture.wrapS = texture.wrapT = THREE.RepeatWrapping;
  texture.anisotropy = 16;
  texture.minFilter = THREE.LinearMipmapLinearFilter;
  texture.magFilter = THREE.LinearFilter;
  texture.generateMipmaps = true;

  useFrame((state) => {
    if (meshRef.current) {
      // Enhanced floating motion
      meshRef.current.position.y = Math.sin(state.clock.elapsedTime * 0.4) * 0.3;
      meshRef.current.position.x = Math.cos(state.clock.elapsedTime * 0.2) * 0.1;
      
      // Smooth rotation
      meshRef.current.rotation.y += 0.003;
      meshRef.current.rotation.z = Math.sin(state.clock.elapsedTime * 0.3) * 0.05;
      
      // Dynamic scale pulsing
      const scale = 1 + Math.sin(state.clock.elapsedTime * 0.6) * 0.08;
      meshRef.current.scale.setScalar(scale);
    }
  });

  return (
    <group ref={meshRef} position={[0, 0, -1.5]}>
      {/* Main magnifying glass body - much larger and more prominent */}
      <mesh position={[0, 0, 0]}>
        <cylinderGeometry args={[1.8, 1.4, 0.6, 64]} />
        <meshPhysicalMaterial 
          map={texture}
          metalness={0.9}
          roughness={0.1}
          clearcoat={1.0}
          clearcoatRoughness={0.05}
          transmission={0.2}
          thickness={0.4}
          ior={1.6}
          transparent
          opacity={0.98}
          emissive="#ff7a1a"
          emissiveIntensity={0.2}
        />
      </mesh>
      
      {/* Glass lens - much larger and more prominent */}
      <mesh position={[0, 0, 0.35]}>
        <cylinderGeometry args={[1.5, 1.5, 0.2, 128]} />
        <meshPhysicalMaterial 
          transmission={0.98}
          thickness={0.2}
          ior={1.6}
          roughness={0.0}
          metalness={0.0}
          clearcoat={1.0}
          clearcoatRoughness={0.0}
          transparent
          opacity={0.95}
          emissive="#ffffff"
          emissiveIntensity={0.1}
        />
      </mesh>
      
      {/* Enhanced handle - larger */}
      <mesh position={[-0.9, -1.8, 0]} rotation={[0, 0, Math.PI / 6]}>
        <cylinderGeometry args={[0.12, 0.12, 2.5, 32]} />
        <meshPhysicalMaterial 
          color="#8B4513"
          metalness={0.4}
          roughness={0.6}
          emissive="#8B4513"
          emissiveIntensity={0.1}
        />
      </mesh>
      
      {/* Additional lens reflection - larger */}
      <mesh position={[0, 0, 0.45]}>
        <cylinderGeometry args={[0.5, 0.5, 0.03, 32]} />
        <meshStandardMaterial 
          color="#ffffff"
          transparent
          opacity={0.8}
          emissive="#ffffff"
          emissiveIntensity={0.5}
        />
      </mesh>
    </group>
  );
};

// Enhanced Floating Particles Component
const FloatingParticles: React.FC = () => {
  const particlesRef = useRef<THREE.Group>(null);
  
  const particles = useMemo(() => {
    return Array.from({ length: 80 }).map((_, i) => ({
      position: [
        Math.random() * 25 - 12.5,
        Math.random() * 25 - 12.5,
        Math.random() * 15 - 7.5
      ] as [number, number, number],
      scale: Math.random() * 0.8 + 0.1,
      speed: Math.random() * 0.03 + 0.01,
      color: Math.random() > 0.7 ? "#ff7a1a" : "#ffffff"
    }));
  }, []);

  useFrame((state) => {
    if (particlesRef.current) {
      particlesRef.current.rotation.y += 0.002;
      particlesRef.current.rotation.x += 0.001;
      particlesRef.current.rotation.z += 0.0005;
    }
  });

  return (
    <group ref={particlesRef}>
      {particles.map((particle, i) => (
        <mesh key={i} position={particle.position}>
          <sphereGeometry args={[particle.scale, 16, 16]} />
          <meshStandardMaterial 
            color={particle.color}
            transparent
            opacity={0.4}
            emissive={particle.color}
            emissiveIntensity={0.3}
            roughness={0.1}
            metalness={0.8}
          />
        </mesh>
      ))}
    </group>
  );
};

// Premium Light Orbs Component
const PremiumLightOrbs: React.FC = () => {
  const orbsRef = useRef<THREE.Group>(null);
  
  const orbs = useMemo(() => {
    return Array.from({ length: 6 }).map((_, i) => ({
      position: [
        Math.cos(i * Math.PI / 3) * 8,
        Math.sin(i * Math.PI / 3) * 8,
        Math.random() * 4 - 2
      ] as [number, number, number],
      scale: Math.random() * 0.5 + 0.3
    }));
  }, []);

  useFrame((state) => {
    if (orbsRef.current) {
      orbsRef.current.rotation.y += 0.001;
    }
  });

  return (
    <group ref={orbsRef}>
      {orbs.map((orb, i) => (
        <mesh key={i} position={orb.position}>
          <sphereGeometry args={[orb.scale, 32, 32]} />
          <meshStandardMaterial 
            color="#ff7a1a"
            transparent
            opacity={0.2}
            emissive="#ff7a1a"
            emissiveIntensity={0.5}
            roughness={0.0}
            metalness={0.0}
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
        camera={{ position: [0, 0, 6], fov: 75 }}
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
        
        {/* Enhanced Floating Particles */}
        <FloatingParticles />
        
        {/* Premium Light Orbs */}
        <PremiumLightOrbs />
        
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
