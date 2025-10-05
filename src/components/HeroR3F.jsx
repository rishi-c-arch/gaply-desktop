import React, { Suspense, useRef, useMemo, useState, useCallback } from 'react';
import { Canvas, useFrame, useThree } from '@react-three/fiber';
import { Html, useGLTF, ContactShadows, Text } from '@react-three/drei';
import * as THREE from 'three';

// Rotating Logo Component
function RotatingLogo({ src, spinSpeed = 0.18, onHover }) {
  const ref = useRef();
  const [isHovered, setIsHovered] = useState(false);
  
  useFrame((state, delta) => {
    if (ref.current && !isHovered) {
      ref.current.rotation.y += delta * spinSpeed;
    }
  });

  const handlePointerEnter = useCallback(() => {
    setIsHovered(true);
    onHover?.(true);
  }, [onHover]);

  const handlePointerLeave = useCallback(() => {
    setIsHovered(false);
    onHover?.(false);
  }, [onHover]);

  // If GLB logo exists, load it; otherwise create a simple 3D logo
  if (src) {
    try {
      const { scene } = useGLTF(src);
      return (
        <group 
          ref={ref} 
          onPointerEnter={handlePointerEnter}
          onPointerLeave={handlePointerLeave}
          scale={[0.8, 0.8, 0.8]}
        >
          <primitive object={scene} />
        </group>
      );
    } catch (error) {
      console.warn('GLB logo not found, using fallback');
    }
  }

  // Fallback: Create a simple 3D logo using geometries
  return (
    <group 
      ref={ref} 
      onPointerEnter={handlePointerEnter}
      onPointerLeave={handlePointerLeave}
      scale={[0.6, 0.6, 0.6]}
    >
      {/* Main logo body */}
      <mesh position={[0, 0, 0]}>
        <boxGeometry args={[1.5, 0.3, 0.3]} />
        <meshStandardMaterial color="#4F46E5" />
      </mesh>
      
      {/* Logo accent */}
      <mesh position={[0, 0, 0.16]}>
        <boxGeometry args={[1.8, 0.1, 0.1]} />
        <meshStandardMaterial color="#6366F1" />
      </mesh>
      
      {/* Letter G representation */}
      <mesh position={[-0.6, 0, 0.2]}>
        <cylinderGeometry args={[0.2, 0.2, 0.1, 8]} />
        <meshStandardMaterial color="#FFFFFF" />
      </mesh>
    </group>
  );
}

// Falling Words Component with Instanced Meshes
function FallingWords({ words = ['IDEA', 'PAPER', 'MENTOR', 'FUND', 'REVIEW', 'PUBLISH', 'GAPLY'], maxWords = 24, onWordClick }) {
  const meshRef = useRef();
  const instancesRef = useRef([]);
  const [hoveredWord, setHoveredWord] = useState(null);

  // Initialize word instances
  const wordInstances = useMemo(() => {
    const instances = [];
    for (let i = 0; i < Math.min(maxWords, words.length); i++) {
      const word = words[i % words.length];
      instances.push({
        id: i,
        word,
        position: [Math.random() * 8 - 4, Math.random() * 4 + 2, Math.random() * 0.9 - 0.3],
        rotation: [Math.random() * 0.2 - 0.1, Math.random() * 0.2 - 0.1, Math.random() * 0.2 - 0.1],
        scale: [0.8 + Math.random() * 0.4, 0.8 + Math.random() * 0.4, 0.8 + Math.random() * 0.4],
        speed: 0.15 + Math.random() * 0.45,
        opacity: 0.6 + Math.random() * 0.4,
        rotationSpeed: [Math.random() * 0.02 - 0.01, Math.random() * 0.02 - 0.01, Math.random() * 0.02 - 0.01],
        fadeIn: 0.2,
        fadeOut: 1.8,
        age: Math.random() * 10 // Random starting age
      });
    }
    return instances;
  }, [words, maxWords]);

  useFrame((state, delta) => {
    if (!meshRef.current) return;

    wordInstances.forEach((instance, index) => {
      // Update age
      instance.age += delta;
      
      // Update position (falling)
      instance.position[1] -= instance.speed * delta;
      
      // Update rotation
      instance.rotation[0] += instance.rotationSpeed[0];
      instance.rotation[1] += instance.rotationSpeed[1];
      instance.rotation[2] += instance.rotationSpeed[2];
      
      // Fade in/out logic
      if (instance.age < instance.fadeIn) {
        instance.opacity = instance.age / instance.fadeIn * (0.6 + Math.random() * 0.4);
      } else if (instance.position[1] < -instance.fadeOut) {
        instance.opacity = Math.max(0, (instance.position[1] + instance.fadeOut + 2) / 2);
      }
      
      // Respawn when fallen too far
      if (instance.position[1] < -2.5) {
        instance.position = [Math.random() * 8 - 4, 3, Math.random() * 0.9 - 0.3];
        instance.age = 0;
        instance.opacity = 0.6 + Math.random() * 0.4;
        instance.speed = 0.15 + Math.random() * 0.45;
        instance.rotationSpeed = [Math.random() * 0.02 - 0.01, Math.random() * 0.02 - 0.01, Math.random() * 0.02 - 0.01];
      }
    });

    // Update instanced mesh
    const matrix = new THREE.Matrix4();
    wordInstances.forEach((instance, index) => {
      matrix.compose(
        new THREE.Vector3(...instance.position),
        new THREE.Quaternion().setFromEuler(new THREE.Euler(...instance.rotation)),
        new THREE.Vector3(...instance.scale)
      );
      meshRef.current.setMatrixAt(index, matrix);
    });
    meshRef.current.instanceMatrix.needsUpdate = true;
  });

  const handleWordClick = useCallback((word) => {
    onWordClick?.(word);
  }, [onWordClick]);

  return (
    <>
      {/* Instanced mesh for performance */}
      <instancedMesh ref={meshRef} args={[null, null, wordInstances.length]}>
        <planeGeometry args={[0.8, 0.2]} />
        <meshStandardMaterial 
          color="#4F46E5" 
          transparent 
          opacity={0.8}
        />
      </instancedMesh>
      
      {/* Individual Text meshes for interaction */}
      {wordInstances.map((instance, index) => (
        <Text
          key={instance.id}
          position={instance.position}
          rotation={instance.rotation}
          scale={instance.scale}
          fontSize={0.15}
          color="#4F46E5"
          anchorX="center"
          anchorY="middle"
          font="/fonts/Inter-Regular.woff"
          onPointerEnter={() => setHoveredWord(instance.word)}
          onPointerLeave={() => setHoveredWord(null)}
          onClick={() => handleWordClick(instance.word)}
          style={{
            opacity: instance.opacity,
            cursor: 'pointer',
            transform: hoveredWord === instance.word ? 'scale(1.1)' : 'scale(1)',
            transition: 'transform 0.2s ease'
          }}
        >
          {instance.word}
        </Text>
      ))}
    </>
  );
}

// Main Hero Component
export default function HeroR3F({ 
  spinSpeed = 0.18, 
  maxWords = 24, 
  words = ['IDEA', 'PAPER', 'MENTOR', 'FUND', 'REVIEW', 'PUBLISH', 'GAPLY'],
  onWordClick,
  enablePoster = true 
}) {
  const [isWebGLSupported, setIsWebGLSupported] = useState(true);
  const [isHovered, setIsHovered] = useState(false);

  // Check WebGL support
  React.useEffect(() => {
    try {
      const canvas = document.createElement('canvas');
      const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
      if (!gl) {
        setIsWebGLSupported(false);
      }
    } catch (e) {
      setIsWebGLSupported(false);
    }
  }, []);

  // Fallback poster component
  const PosterFallback = () => (
    <div className="w-full h-full bg-gradient-to-br from-indigo-100 via-purple-50 to-blue-100 flex items-center justify-center relative overflow-hidden">
      {/* Gradient overlay */}
      <div className="absolute inset-0 bg-gradient-to-b from-[rgba(120,120,128,0.5)] via-transparent to-transparent"></div>
      
      {/* Logo representation */}
      <div className="relative z-10 text-center">
        <div className="w-32 h-32 mx-auto mb-6 bg-gradient-to-br from-indigo-500 to-purple-600 rounded-2xl flex items-center justify-center shadow-2xl">
          <span className="text-white text-4xl font-bold">G</span>
        </div>
        <h3 className="text-2xl font-bold text-gray-800 mb-2">Interactive Research Platform</h3>
        <p className="text-gray-600">3D visualization loading...</p>
      </div>
      
      {/* Floating words */}
      <div className="absolute inset-0 pointer-events-none">
        {words.slice(0, 8).map((word, index) => (
          <div
            key={word}
            className="absolute text-indigo-400 font-semibold opacity-30"
            style={{
              left: `${10 + (index * 12)}%`,
              top: `${20 + (index % 3) * 25}%`,
              fontSize: '14px',
              animation: `float 3s ease-in-out infinite ${index * 0.5}s`
            }}
          >
            {word}
          </div>
        ))}
      </div>
      
      <style jsx>{`
        @keyframes float {
          0%, 100% { transform: translateY(0px); }
          50% { transform: translateY(-10px); }
        }
      `}</style>
    </div>
  );

  if (!isWebGLSupported && enablePoster) {
    return (
      <section className="max-w-6xl mx-auto px-6 py-24 lg:grid lg:grid-cols-2 gap-12 items-center">
        {/* Left Column - Content */}
        <div className="lg:w-[52%]">
          <h1 className="text-3xl md:text-5xl lg:text-6xl font-extrabold leading-[1.02] text-gray-900" style={{ fontFamily: 'Inter, SF Pro, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif' }}>
            THIS IS THE PLATFORM THAT MAKES <span className="text-indigo-600">GENIUS</span> LOOK EFFORTLESS
          </h1>

          <p className="mt-6 text-lg md:text-xl text-gray-700 max-w-xl font-medium">
            Publish a 300-word idea. Get fast peer feedback. Find mentors & funding.
          </p>

          {/* CTA Buttons */}
          <div className="mt-10 flex gap-4">
            <a 
              href="/start" 
              className="px-6 py-3 rounded-xl bg-indigo-600 text-white font-medium shadow-md hover:scale-[1.03] transition-transform duration-160"
              aria-label="Start free trial"
            >
              Start Free
            </a>
            <a 
              href="/mentors" 
              className="px-5 py-3 rounded-xl border border-gray-200 text-gray-700 hover:bg-gray-50 transition-colors"
              aria-label="Explore mentors"
            >
              Explore Mentors
            </a>
          </div>

          {/* Stats Block */}
          <div className="mt-10 grid grid-cols-3 gap-6 max-w-md">
            <div>
              <div className="text-3xl md:text-4xl font-bold text-gray-900" style={{ letterSpacing: '0.5px' }}>10K+</div>
              <div className="text-xs uppercase tracking-wider text-gray-500 mt-1">Research Papers</div>
            </div>
            <div>
              <div className="text-3xl md:text-4xl font-bold text-gray-900" style={{ letterSpacing: '0.5px' }}>500+</div>
              <div className="text-xs uppercase tracking-wider text-gray-500 mt-1">Journals</div>
            </div>
            <div>
              <div className="text-3xl md:text-4xl font-bold text-gray-900" style={{ letterSpacing: '0.5px' }}>95%</div>
              <div className="text-xs uppercase tracking-wider text-gray-500 mt-1">Accuracy</div>
            </div>
          </div>
        </div>

        {/* Right Column - Fallback */}
        <div className="lg:w-[48%] flex justify-center lg:justify-end mt-12 lg:mt-0">
          <div className="w-[420px] md:w-[520px] lg:w-[620px] h-[420px] md:h-[520px] lg:h-[620px] rounded-2xl shadow-2xl bg-white/60 backdrop-blur-sm overflow-hidden relative">
            <PosterFallback />
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="max-w-6xl mx-auto px-6 py-24 lg:grid lg:grid-cols-2 gap-12 items-center">
      {/* Left Column - Content */}
      <div className="lg:w-[52%]">
        <h1 className="text-3xl md:text-5xl lg:text-6xl font-extrabold leading-[1.02] text-gray-900" style={{ fontFamily: 'Inter, SF Pro, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif' }}>
          THIS IS THE PLATFORM THAT MAKES <span className="text-indigo-600">GENIUS</span> LOOK EFFORTLESS
        </h1>

        <p className="mt-6 text-lg md:text-xl text-gray-700 max-w-xl font-medium">
          Publish a 300-word idea. Get fast peer feedback. Find mentors & funding.
        </p>

        {/* CTA Buttons */}
        <div className="mt-10 flex gap-4">
          <a 
            href="/start" 
            className="px-6 py-3 rounded-xl bg-indigo-600 text-white font-medium shadow-md hover:scale-[1.03] transition-transform duration-160"
            aria-label="Start free trial"
          >
            Start Free
          </a>
          <a 
            href="/mentors" 
            className="px-5 py-3 rounded-xl border border-gray-200 text-gray-700 hover:bg-gray-50 transition-colors"
            aria-label="Explore mentors"
          >
            Explore Mentors
          </a>
        </div>

        {/* Stats Block */}
        <div className="mt-10 grid grid-cols-3 gap-6 max-w-md">
          <div>
            <div className="text-3xl md:text-4xl font-bold text-gray-900" style={{ letterSpacing: '0.5px' }}>10K+</div>
            <div className="text-xs uppercase tracking-wider text-gray-500 mt-1">Research Papers</div>
          </div>
          <div>
            <div className="text-3xl md:text-4xl font-bold text-gray-900" style={{ letterSpacing: '0.5px' }}>500+</div>
            <div className="text-xs uppercase tracking-wider text-gray-500 mt-1">Journals</div>
          </div>
          <div>
            <div className="text-3xl md:text-4xl font-bold text-gray-900" style={{ letterSpacing: '0.5px' }}>95%</div>
            <div className="text-xs uppercase tracking-wider text-gray-500 mt-1">Accuracy</div>
          </div>
        </div>
      </div>

      {/* Right Column - 3D Canvas */}
      <div className="lg:w-[48%] flex justify-center lg:justify-end mt-12 lg:mt-0">
        <div 
          className="w-[420px] md:w-[520px] lg:w-[620px] h-[420px] md:h-[520px] lg:h-[620px] rounded-2xl shadow-2xl bg-white/60 backdrop-blur-sm overflow-hidden relative"
          aria-hidden="true"
        >
          {/* Gradient overlay */}
          <div className="absolute inset-0 bg-gradient-to-b from-[rgba(120,120,128,0.5)] via-transparent to-transparent pointer-events-none z-10"></div>
          
          <Canvas 
            pixelRatio={Math.min(window.devicePixelRatio, 1.5)} 
            camera={{ position: [0, 0, 4.2], fov: 45 }}
            onCreated={({ gl }) => {
              gl.shadowMap.enabled = true;
              gl.shadowMap.type = THREE.PCFSoftShadowMap;
            }}
          >
            {/* Lighting Setup */}
            <ambientLight intensity={0.4} />
            <directionalLight 
              position={[5, 5, 5]} 
              intensity={0.8} 
              castShadow
              shadow-mapSize-width={1024}
              shadow-mapSize-height={1024}
              shadow-camera-far={50}
              shadow-camera-left={-10}
              shadow-camera-right={10}
              shadow-camera-top={10}
              shadow-camera-bottom={-10}
            />
            <directionalLight position={[-5, 5, -5]} intensity={0.3} color="#6366F1" />
            <pointLight position={[0, 5, 0]} intensity={0.5} color="#8B5CF6" />

            <Suspense fallback={
              <Html center>
                <div className="text-center">
                  <div className="w-8 h-8 border-2 border-indigo-600 border-t-transparent rounded-full animate-spin mx-auto mb-2"></div>
                  <p className="text-gray-600 text-sm">Loading 3D scene...</p>
                </div>
              </Html>
            }>
              <RotatingLogo 
                src="/models/r3f-hero/gaply-logo-2025.glb" 
                spinSpeed={spinSpeed}
                onHover={setIsHovered}
              />
              <FallingWords 
                words={words} 
                maxWords={maxWords}
                onWordClick={onWordClick}
              />
              <ContactShadows 
                position={[0, -1.2, 0]} 
                opacity={0.6} 
                blur={2} 
                far={4.5}
                resolution={256}
                color="#4F46E5"
              />
            </Suspense>
          </Canvas>
          
          {/* Screen reader transcript */}
          <div className="sr-only">
            <ul>
              <li>Interactive 3D research platform visualization</li>
              <li>Rotating GAPLY logo with hover pause functionality</li>
              <li>Falling research keywords: {words.join(', ')}</li>
              <li>Click on any word to explore related features</li>
            </ul>
          </div>
        </div>
      </div>
    </section>
  );
}
