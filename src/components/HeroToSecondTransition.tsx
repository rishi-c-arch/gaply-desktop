import React, { useRef, useEffect, useState } from 'react';
import { Canvas, useFrame, useThree } from '@react-three/fiber';
import { Text, Environment, OrbitControls } from '@react-three/drei';
import * as THREE from 'three';

// Three.js Transition Component
const TransitionMesh: React.FC<{ scrollProgress: number }> = ({ scrollProgress }) => {
  const meshRef = useRef<THREE.Mesh>(null);
  const groupRef = useRef<THREE.Group>(null);

  useFrame((state) => {
    if (meshRef.current && groupRef.current) {
      // Rotate the mesh based on scroll progress
      meshRef.current.rotation.x = scrollProgress * Math.PI * 2;
      meshRef.current.rotation.y = scrollProgress * Math.PI;
      
      // Scale animation
      const scale = 1 + Math.sin(scrollProgress * Math.PI * 4) * 0.2;
      meshRef.current.scale.setScalar(scale);
      
      // Position animation
      groupRef.current.position.y = Math.sin(scrollProgress * Math.PI) * 2;
      groupRef.current.position.z = Math.cos(scrollProgress * Math.PI) * 3;
    }
  });

  return (
    <group ref={groupRef}>
      <mesh ref={meshRef}>
        <torusGeometry args={[1, 0.4, 16, 100]} />
        <meshStandardMaterial 
          color="#cecece" 
          metalness={0.8} 
          roughness={0.2}
          transparent
          opacity={0.8}
        />
      </mesh>
      
      {/* Floating particles */}
      {Array.from({ length: 20 }).map((_, i) => (
        <mesh key={i} position={[
          Math.sin(i * 0.3) * 3,
          Math.cos(i * 0.3) * 3,
          Math.sin(i * 0.5) * 2
        ]}>
          <sphereGeometry args={[0.05, 8, 8]} />
          <meshStandardMaterial 
            color="#b5b5b5" 
            transparent
            opacity={0.6}
          />
        </mesh>
      ))}
    </group>
  );
};

// CSS 3D Transform Component
const CSS3DTransition: React.FC<{ scrollProgress: number }> = ({ scrollProgress }) => {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (containerRef.current) {
      const transform = `
        perspective(1000px) 
        rotateX(${scrollProgress * 15}deg) 
        rotateY(${scrollProgress * 10}deg) 
        translateZ(${scrollProgress * 100}px)
        scale(${1 + scrollProgress * 0.2})
      `;
      containerRef.current.style.transform = transform;
      containerRef.current.style.opacity = `${1 - scrollProgress * 0.5}`;
    }
  }, [scrollProgress]);

  return (
    <div 
      ref={containerRef}
      style={{
        position: 'fixed',
        top: '50%',
        left: '50%',
        transform: 'translate(-50%, -50%)',
        width: '200px',
        height: '200px',
        background: 'linear-gradient(45deg, #535353, #6c6c6c, #848484, #9d9d9d, #b5b5b5, #cecece, #e6e6e6, #ffffff)',
        borderRadius: '50%',
        opacity: 0.8,
        zIndex: 1000,
        transition: 'all 0.1s ease-out',
        boxShadow: '0 20px 40px rgba(0,0,0,0.3)',
      }}
    />
  );
};

// Main Transition Component
const HeroToSecondTransition: React.FC = () => {
  const [scrollProgress, setScrollProgress] = useState(0);
  const transitionRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleScroll = () => {
      if (transitionRef.current) {
        const rect = transitionRef.current.getBoundingClientRect();
        const windowHeight = window.innerHeight;
        const progress = Math.max(0, Math.min(1, (windowHeight - rect.top) / windowHeight));
        setScrollProgress(progress);
      }
    };

    window.addEventListener('scroll', handleScroll);
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  return (
    <div 
      ref={transitionRef}
      style={{
        position: 'relative',
        height: '100vh',
        background: 'linear-gradient(180deg, #0A0A0A 0%, #1a1a1a 50%, #2a2a2a 100%)',
        overflow: 'hidden',
      }}
    >
      {/* Three.js Canvas */}
      <div style={{ 
        position: 'absolute', 
        top: 0, 
        left: 0, 
        width: '100%', 
        height: '100%',
        zIndex: 1
      }}>
        <Canvas camera={{ position: [0, 0, 5], fov: 75 }}>
          <ambientLight intensity={0.4} />
          <directionalLight position={[5, 5, 5]} intensity={1} />
          <pointLight position={[-5, -5, -5]} intensity={0.5} />
          
          <TransitionMesh scrollProgress={scrollProgress} />
          
          <Environment preset="night" />
        </Canvas>
      </div>

      {/* CSS 3D Transform Overlay */}
      <CSS3DTransition scrollProgress={scrollProgress} />

      {/* Transition Text */}
      <div style={{
        position: 'absolute',
        top: '50%',
        left: '50%',
        transform: 'translate(-50%, -50%)',
        zIndex: 2,
        textAlign: 'center',
        color: '#cecece',
        fontFamily: 'Inter, sans-serif',
        fontSize: '2rem',
        fontWeight: '600',
        opacity: scrollProgress > 0.3 ? 1 : 0,
        transition: 'opacity 0.5s ease-in-out',
      }}>
        <div style={{
          background: 'linear-gradient(45deg, #535353, #6c6c6c, #848484, #9d9d9d, #b5b5b5, #cecece, #e6e6e6, #ffffff)',
          backgroundSize: '400% 400%',
          WebkitBackgroundClip: 'text',
          WebkitTextFillColor: 'transparent',
          backgroundClip: 'text',
          animation: 'colorMotion 8s ease-in-out infinite',
        }}>
          TRANSFORMING IDEAS
        </div>
        <div style={{
          fontSize: '1rem',
          marginTop: '1rem',
          opacity: 0.8,
        }}>
          Into Academic Excellence
        </div>
      </div>

      {/* Scroll Indicator */}
      <div style={{
        position: 'absolute',
        bottom: '2rem',
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 2,
        color: '#cecece',
        fontSize: '0.9rem',
        opacity: scrollProgress < 0.1 ? 1 : 0,
        transition: 'opacity 0.5s ease-in-out',
      }}>
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          gap: '0.5rem',
        }}>
          <span>Scroll to Continue</span>
          <div style={{
            width: '2px',
            height: '30px',
            background: 'linear-gradient(to bottom, #cecece, transparent)',
            animation: 'scrollPulse 2s ease-in-out infinite',
          }} />
        </div>
      </div>
    </div>
  );
};

export default HeroToSecondTransition;
