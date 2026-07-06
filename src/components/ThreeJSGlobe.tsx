import React, { useRef, Suspense } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls, Environment, useTexture } from '@react-three/drei';
import { Mesh } from 'three';
import globeTexture from '../assets/dreamstime_xxl_126606510.jpg';

const GlobeMesh: React.FC = () => {
  const meshRef = useRef<Mesh>(null);
  
  // Use useTexture from drei for better error handling and quality
  const texture = useTexture(globeTexture);
  
  // Configure texture for maximum quality
  texture.wrapS = texture.wrapT = 1001; // Repeat wrapping
  texture.anisotropy = 16; // Maximum anisotropy for sharp textures
  texture.minFilter = 1006; // Linear mipmap linear filter
  texture.magFilter = 1003; // Linear filter
  texture.generateMipmaps = true;

  useFrame((state) => {
    if (meshRef.current) {
      // Gentle floating motion
      meshRef.current.position.y = Math.sin(state.clock.elapsedTime * 0.5) * 0.1;
      
      // Slow rotation on Y axis
      meshRef.current.rotation.y += 0.005;
      
      // Gentle wobble motion
      meshRef.current.rotation.x = Math.sin(state.clock.elapsedTime * 0.3) * 0.05;
    }
  });

  return (
    <mesh ref={meshRef} position={[0, 0, 0]}>
      <sphereGeometry args={[2, 128, 128]} />
      <meshPhysicalMaterial 
        map={texture}
        transparent={true}
        opacity={0.95}
        roughness={0.1}
        metalness={0.05}
        clearcoat={0.8}
        clearcoatRoughness={0.1}
        transmission={0.1}
        thickness={0.5}
        ior={1.5}
      />
    </mesh>
  );
};

const LoadingFallback: React.FC = () => {
  return (
    <mesh position={[0, 0, 0]}>
      <sphereGeometry args={[2, 64, 64]} />
      <meshPhysicalMaterial 
        color="#ff7a1a" 
        roughness={0.1}
        metalness={0.1}
        clearcoat={0.8}
        clearcoatRoughness={0.1}
      />
    </mesh>
  );
};

const ThreeJSGlobe: React.FC = () => {
  return (
    <div className="threejs-globe-container">
      <Canvas
        camera={{ position: [0, 0, 5], fov: 45 }}
        style={{ width: '100%', height: '100%' }}
        shadows
        gl={{ 
          antialias: true, 
          alpha: true,
          powerPreference: "high-performance",
          stencil: false,
          depth: true
        }}
        dpr={[1, 2]} // Device pixel ratio for crisp rendering
        performance={{ min: 0.5 }}
      >
        {/* Enhanced Lighting Setup */}
        <ambientLight intensity={0.3} />
        <directionalLight 
          position={[10, 10, 5]} 
          intensity={1.2}
          castShadow
          shadow-mapSize-width={4096}
          shadow-mapSize-height={4096}
          shadow-camera-near={0.5}
          shadow-camera-far={50}
          shadow-camera-left={-10}
          shadow-camera-right={10}
          shadow-camera-top={10}
          shadow-camera-bottom={-10}
        />
        <pointLight position={[-10, -10, -5]} intensity={1.0} color="#ff7a1a" />
        <hemisphereLight intensity={0.2} />
        
        {/* Globe with Suspense for loading */}
        <Suspense fallback={<LoadingFallback />}>
          <GlobeMesh />
        </Suspense>
        
        {/* Enhanced Controls */}
        <OrbitControls 
          enableZoom={false}
          enablePan={false}
          autoRotate={true}
          autoRotateSpeed={0.3}
          enableDamping={true}
          dampingFactor={0.05}
          maxPolarAngle={Math.PI / 1.8}
          minPolarAngle={Math.PI / 2.2}
        />
        
        {/* High-quality Environment — bundled HDR (same file as drei's "studio"
            preset) so the globe renders offline / inside Tauri without CDN access */}
        <Environment files={`${process.env.PUBLIC_URL}/hdr/studio_small_03_1k.hdr`} background={false} />
      </Canvas>
    </div>
  );
};

export default ThreeJSGlobe;
