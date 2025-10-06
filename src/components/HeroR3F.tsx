import React, { Suspense, useRef, useMemo, useState, useCallback } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Html, ContactShadows, Text, OrbitControls } from '@react-three/drei';
import * as THREE from 'three';

// TypeScript declarations
declare global {
  namespace JSX {
    interface IntrinsicElements {
      mesh: any;
      planeGeometry: any;
      meshBasicMaterial: any;
      group: any;
      ambientLight: any;
      directionalLight: any;
      pointLight: any;
    }
  }
}

// Rotating Logo Component
function RotatingLogo({ src, spinSpeed = 0.18, onHover }: { src?: string; spinSpeed?: number; onHover?: (hovering: boolean) => void }) {
  const ref = useRef<THREE.Group>(null);
  const [isHovered, setIsHovered] = useState(false);
  const [isPaused, setIsPaused] = useState(false);

  useFrame((state, delta) => {
    if (ref.current && !isPaused) {
      ref.current.rotation.y += delta * spinSpeed;
    }
  });

  const handlePointerEnter = useCallback(() => {
    setIsHovered(true);
    setIsPaused(true);
    onHover?.(true);
  }, [onHover]);

  const handlePointerLeave = useCallback(() => {
    setIsHovered(false);
    setIsPaused(false);
    onHover?.(false);
  }, [onHover]);

  return (
    <group 
      ref={ref} 
      onPointerEnter={handlePointerEnter}
      onPointerLeave={handlePointerLeave}
      scale={isHovered ? 1.05 : 1}
    >
      {/* Simple 3D Logo - Using geometry since we don't have a GLB */}
      <mesh position={[0, 0, 0]}>
        <cylinderGeometry args={[0.8, 0.8, 0.2, 32]} />
        <meshStandardMaterial 
          color="#4F46E5" 
          metalness={0.3} 
          roughness={0.4}
          emissive="#1e1b4b"
          emissiveIntensity={0.1}
        />
      </mesh>
      
      {/* Inner ring */}
      <mesh position={[0, 0, 0.11]}>
        <cylinderGeometry args={[0.6, 0.6, 0.05, 32]} />
        <meshStandardMaterial 
          color="#ffffff" 
          metalness={0.8} 
          roughness={0.2}
        />
      </mesh>

      {/* Center dot */}
      <mesh position={[0, 0, 0.16]}>
        <sphereGeometry args={[0.15, 16, 16]} />
        <meshStandardMaterial 
          color="#4F46E5" 
          metalness={0.5} 
          roughness={0.3}
          emissive="#4F46E5"
          emissiveIntensity={0.2}
        />
      </mesh>
    </group>
  );
}

// Word Instance for Falling Words
interface WordInstance {
  id: number;
  word: string;
  position: [number, number, number];
  rotation: [number, number, number];
  scale: number;
  speed: number;
  opacity: number;
  fadeIn: number;
  fadeOut: number;
}

// Falling Words Component
function FallingWords({ 
  words = ['IDEA', 'PAPER', 'MENTOR', 'FUND', 'REVIEW', 'PUBLISH', 'GAPLY'], 
  maxWords = 24,
  isPaused = false,
  onWordClick
}: { 
  words?: string[]; 
  maxWords?: number; 
  isPaused?: boolean;
  onWordClick?: (word: string) => void;
}) {
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const [wordInstances, setWordInstances] = useState<WordInstance[]>([]);
  const [hoveredWord, setHoveredWord] = useState<number | null>(null);

  // Initialize word instances
  useMemo(() => {
    const instances: WordInstance[] = [];
    for (let i = 0; i < maxWords; i++) {
      instances.push({
        id: i,
        word: words[i % words.length],
        position: [
          (Math.random() - 0.5) * 6, // X: -3 to 3
          Math.random() * 4 + 2,      // Y: 2 to 6 (start above)
          (Math.random() - 0.5) * 1   // Z: -0.5 to 0.5 (depth)
        ] as [number, number, number],
        rotation: [
          Math.random() * Math.PI * 2,
          Math.random() * Math.PI * 2,
          Math.random() * Math.PI * 2
        ] as [number, number, number],
        scale: 0.3 + Math.random() * 0.4,
        speed: 0.15 + Math.random() * 0.45,
        opacity: 0,
        fadeIn: Math.random() * 0.5,
        fadeOut: 0.8 + Math.random() * 0.2
      });
    }
    setWordInstances(instances);
  }, [maxWords, words]);

  useFrame((state, delta) => {
    if (isPaused || !meshRef.current) return;

    const tempObject = new THREE.Object3D();

    setWordInstances(prev => prev.map((instance, index) => {
      let newInstance = { ...instance };

      // Update position (falling)
      newInstance.position[1] -= newInstance.speed * delta;
      
      // Update rotation
      newInstance.rotation[0] += delta * 0.5;
      newInstance.rotation[1] += delta * 0.3;
      newInstance.rotation[2] += delta * 0.4;

      // Update opacity (fade in/out)
      if (newInstance.position[1] > 3) {
        newInstance.opacity = Math.min(1, (newInstance.position[1] - 3) / newInstance.fadeIn);
      } else if (newInstance.position[1] < -2) {
        newInstance.opacity = Math.max(0, (newInstance.position[1] + 2) / newInstance.fadeOut);
      } else {
        newInstance.opacity = 1;
      }

      // Respawn if below bottom
      if (newInstance.position[1] < -3) {
        newInstance.position = [
          (Math.random() - 0.5) * 6,
          4 + Math.random() * 2,
          (Math.random() - 0.5) * 1
        ] as [number, number, number];
        newInstance.speed = 0.15 + Math.random() * 0.45;
        newInstance.word = words[Math.floor(Math.random() * words.length)];
        newInstance.scale = 0.3 + Math.random() * 0.4;
        newInstance.opacity = 0;
      }

      // Update instance matrix
      tempObject.position.set(...newInstance.position);
      tempObject.rotation.set(...newInstance.rotation);
      tempObject.scale.setScalar(newInstance.scale);
      tempObject.updateMatrix();
      meshRef.current!.setMatrixAt(index, tempObject.matrix);

      return newInstance;
    }));

    meshRef.current.instanceMatrix.needsUpdate = true;
  });

  return (
    <group>
      {wordInstances.map((instance, index) => (
        <Text
          key={instance.id}
          position={instance.position}
          rotation={instance.rotation}
          scale={instance.scale}
          color="#4F46E5"
          fontSize={0.4}
          font="/fonts/inter.woff"
          anchorX="center"
          anchorY="middle"
          onPointerEnter={() => setHoveredWord(index)}
          onPointerLeave={() => setHoveredWord(null)}
          onClick={() => onWordClick?.(instance.word)}
          material-transparent
          material-opacity={instance.opacity}
        >
          {instance.word}
        </Text>
      ))}
    </group>
  );
}

// Scene Component
function Scene({ 
  spinSpeed, 
  maxWords, 
  words, 
  onWordClick 
}: { 
  spinSpeed?: number; 
  maxWords?: number; 
  words?: string[];
  onWordClick?: (word: string) => void;
}) {
  const [isPaused, setIsPaused] = useState(false);
  const [logoHovered, setLogoHovered] = useState(false);

  const handleCanvasHover = useCallback((hovering: boolean) => {
    setIsPaused(hovering || logoHovered);
  }, [logoHovered]);

  return (
    <>
      {/* Lighting Setup */}
      <ambientLight intensity={0.4} />
      <directionalLight 
        position={[5, 5, 5]} 
        intensity={0.8} 
        color="#fff8e1"
        castShadow
        shadow-mapSize={[2048, 2048]}
        shadow-camera-far={50}
        shadow-camera-left={-10}
        shadow-camera-right={10}
        shadow-camera-top={10}
        shadow-camera-bottom={-10}
      />
      <pointLight position={[-5, -5, -5]} intensity={0.3} color="#e0e7ff" />
      <pointLight position={[5, -5, 5]} intensity={0.2} color="#fef3c7" />

      {/* Rotating Logo */}
      <RotatingLogo 
        spinSpeed={spinSpeed} 
        onHover={setLogoHovered}
      />

      {/* Falling Words */}
      <FallingWords 
        words={words}
        maxWords={maxWords}
        isPaused={isPaused}
        onWordClick={onWordClick}
      />

      {/* Contact Shadows */}
      <ContactShadows 
        position={[0, -1.2, 0]} 
        opacity={0.6} 
        blur={2} 
        far={4.5}
        resolution={256}
        color="#000000"
      />

      {/* Orbit Controls */}
      <OrbitControls 
        enableZoom={false}
        enablePan={false}
        maxPolarAngle={Math.PI / 2.2}
        minPolarAngle={Math.PI / 3}
        maxAzimuthAngle={Math.PI / 4}
        minAzimuthAngle={-Math.PI / 4}
      />
    </>
  );
}

// Main HeroR3F Component
interface HeroR3FProps {
  spinSpeed?: number;
  maxWords?: number;
  words?: string[];
  onWordClick?: (word: string) => void;
  enablePoster?: boolean;
}

export default function HeroR3F({
  spinSpeed = 0.18,
  maxWords = 24,
  words = ['IDEA', 'PAPER', 'MENTOR', 'FUND', 'REVIEW', 'PUBLISH', 'GAPLY'],
  onWordClick,
  enablePoster = true
}: HeroR3FProps) {
  const [webglSupported, setWebglSupported] = useState(true);
  const [isLoading, setIsLoading] = useState(true);

  // Check WebGL support
  React.useEffect(() => {
    const canvas = document.createElement('canvas');
    const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
    setWebglSupported(!!gl);
    setIsLoading(false);
  }, []);

  const handleWordClick = useCallback((word: string) => {
    console.log(`Word clicked: ${word}`);
    onWordClick?.(word);
    // You can add routing or modal opening logic here
  }, [onWordClick]);

  // Fallback poster component
  const PosterFallback = () => (
    <div className="w-full h-full bg-gradient-to-br from-indigo-100 to-purple-100 flex items-center justify-center relative overflow-hidden">
      <div className="absolute inset-0 bg-gradient-to-b from-[rgba(120,120,128,0.5)] via-transparent to-transparent"></div>
      <div className="text-center z-10">
        <div className="w-32 h-32 mx-auto mb-6 bg-gradient-to-br from-indigo-500 to-purple-600 rounded-full flex items-center justify-center animate-spin">
          <span className="text-white text-4xl font-bold">G</span>
        </div>
        <h3 className="text-2xl font-bold text-gray-900 mb-2">GAPLY</h3>
        <p className="text-gray-600">Interactive 3D Experience</p>
        <p className="text-sm text-gray-500 mt-2">Loading...</p>
      </div>
    </div>
  );

  return (
    <section className="max-w-6xl mx-auto px-6 py-24 lg:grid lg:grid-cols-2 gap-12 items-center">
      {/* Left Column - Content */}
      <div className="lg:w-[52%]">
        <h1 
          className="text-4xl lg:text-6xl font-extrabold leading-tight text-gray-900 mb-6"
          style={{ 
            fontFamily: 'Inter, SF Pro, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif',
            fontWeight: 700,
            lineHeight: 1.02
          }}
        >
          THIS IS THE PLATFORM THAT MAKES <span className="text-indigo-600">GENIUS</span> LOOK EFFORTLESS
        </h1>

        <p className="text-lg md:text-xl text-gray-700 max-w-xl font-medium mb-8">
          Publish a 300-word idea. Get fast peer feedback. Find mentors & funding.
        </p>

        {/* CTA Buttons */}
        <div className="flex gap-4 mb-10">
          <a 
            href="/start" 
            className="px-6 py-3 rounded-xl bg-indigo-600 text-white font-medium shadow-md hover:scale-105 transition-transform duration-200"
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
        <div className="grid grid-cols-3 gap-6 max-w-md">
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
        <div className="w-[420px] md:w-[520px] lg:w-[620px] h-[420px] md:h-[520px] lg:h-[620px] rounded-2xl shadow-2xl bg-white/60 backdrop-blur-sm overflow-hidden relative">
          {isLoading ? (
            <PosterFallback />
          ) : webglSupported ? (
            <Canvas
              camera={{ position: [0, 0, 4.2], fov: 45 }}
              gl={{ antialias: true, alpha: true }}
              dpr={Math.min(window.devicePixelRatio, 1.5)}
            >
              <Suspense fallback={
                <Html center>
                  <div className="text-center">
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-600 mx-auto"></div>
                    <p className="mt-2 text-sm text-gray-600">Loading 3D Scene...</p>
                  </div>
                </Html>
              }>
                <Scene 
                  spinSpeed={spinSpeed}
                  maxWords={maxWords}
                  words={words}
                  onWordClick={handleWordClick}
                />
              </Suspense>
            </Canvas>
          ) : (
            <PosterFallback />
          )}
          
          {/* Soft drop shadow */}
          <div className="absolute -bottom-4 left-4 right-4 h-8 bg-gradient-to-t from-black/10 to-transparent rounded-full blur-xl"></div>
        </div>
      </div>

      {/* Screen Reader Accessibility */}
      <div className="sr-only">
        <h2>Interactive 3D Hero Section</h2>
        <p>This section contains a rotating GAPLY logo and falling words animation.</p>
        <ul>
          {words.map((word, index) => (
            <li key={index}>{word}</li>
          ))}
        </ul>
      </div>
    </section>
  );
}
