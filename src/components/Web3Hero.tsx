import React, { useEffect, useRef, useState } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Text, OrbitControls, Environment, ContactShadows } from '@react-three/drei';
import * as THREE from 'three';
import gsap from 'gsap';

// Floating particles component
function FloatingParticles({ count = 50 }) {
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const particles = useRef<THREE.Object3D[]>([]);

  useEffect(() => {
    if (meshRef.current) {
      const temp = new THREE.Object3D();
      particles.current = [];
      
      for (let i = 0; i < count; i++) {
        const x = (Math.random() - 0.5) * 20;
        const y = (Math.random() - 0.5) * 20;
        const z = (Math.random() - 0.5) * 20;
        
        temp.position.set(x, y, z);
        temp.scale.setScalar(Math.random() * 0.5 + 0.5);
        temp.updateMatrix();
        meshRef.current.setMatrixAt(i, temp.matrix);
        particles.current.push(temp.clone());
      }
      meshRef.current.instanceMatrix.needsUpdate = true;
    }
  }, [count]);

  useFrame((state) => {
    if (meshRef.current) {
      particles.current.forEach((particle, i) => {
        particle.rotation.x += 0.01;
        particle.rotation.y += 0.01;
        particle.position.y += Math.sin(state.clock.elapsedTime + i) * 0.005;
        particle.updateMatrix();
        meshRef.current!.setMatrixAt(i, particle.matrix);
      });
      meshRef.current.instanceMatrix.needsUpdate = true;
    }
  });

  return (
    <instancedMesh ref={meshRef} args={[undefined, undefined, count]}>
      <sphereGeometry args={[0.02, 8, 8]} />
      <meshStandardMaterial color="#ffffff" emissive="#ffffff" emissiveIntensity={0.2} />
    </instancedMesh>
  );
}

// Rotating geometric logo
function GeometricLogo({ scale = 1 }) {
  const groupRef = useRef<THREE.Group>(null);
  const [hovered, setHovered] = useState(false);

  useFrame((state) => {
    if (groupRef.current) {
      groupRef.current.rotation.y += hovered ? 0.02 : 0.005;
      groupRef.current.scale.lerp(new THREE.Vector3(hovered ? scale * 1.1 : scale), 0.1);
    }
  });

  return (
    <group 
      ref={groupRef}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
    >
      {/* Main geometric shape */}
      <mesh position={[0, 0, 0]}>
        <octahedronGeometry args={[1.2, 0]} />
        <meshStandardMaterial 
          color="#ffffff" 
          metalness={0.8} 
          roughness={0.2}
          emissive="#ffffff"
          emissiveIntensity={0.1}
        />
      </mesh>
      
      {/* Inner shape */}
      <mesh position={[0, 0, 0]}>
        <tetrahedronGeometry args={[0.8, 0]} />
        <meshStandardMaterial 
          color="#f8fafc" 
          metalness={0.9} 
          roughness={0.1}
          transparent
          opacity={0.8}
        />
      </mesh>

      {/* Accent rings */}
      <mesh rotation={[Math.PI / 2, 0, 0]}>
        <torusGeometry args={[1.8, 0.05, 8, 32]} />
        <meshStandardMaterial 
          color="#ffffff" 
          metalness={0.7} 
          roughness={0.3}
          emissive="#ffffff"
          emissiveIntensity={0.2}
        />
      </mesh>
    </group>
  );
}

// Animated text component
function AnimatedText({ children, delay = 0 }: { children: string; delay?: number }) {
  const textRef = useRef<THREE.Group>(null);

  useEffect(() => {
    if (textRef.current) {
      gsap.fromTo(textRef.current, 
        { opacity: 0, y: 50 },
        { opacity: 1, y: 0, duration: 1.5, delay, ease: "power3.out" }
      );
    }
  }, [delay]);

  return (
    <group ref={textRef}>
      <Text
        fontSize={0.8}
        color="#ffffff"
        anchorX="center"
        anchorY="middle"
        font="/fonts/inter.woff"
        maxWidth={8}
        textAlign="center"
      >
        {children}
      </Text>
    </group>
  );
}

// Main 3D scene
function Scene() {
  return (
    <>
      <Environment preset="studio" />
      <ambientLight intensity={0.4} />
      <directionalLight position={[10, 10, 5]} intensity={1} color="#ffffff" />
      <pointLight position={[-10, -10, -5]} intensity={0.5} color="#f0f9ff" />
      
      <FloatingParticles count={80} />
      <GeometricLogo scale={0.8} />
      
      <AnimatedText delay={0.5}>GAPLY</AnimatedText>
      <AnimatedText delay={1}>GENIUS LOOKS EFFORTLESS</AnimatedText>
      
      <ContactShadows 
        position={[0, -2, 0]} 
        opacity={0.4} 
        blur={2} 
        far={4.5}
        resolution={256}
        color="#000000"
      />
      
      <OrbitControls 
        enableZoom={false}
        enablePan={false}
        maxPolarAngle={Math.PI / 2.2}
        minPolarAngle={Math.PI / 3}
        autoRotate
        autoRotateSpeed={0.5}
      />
    </>
  );
}

// Main Web3 Hero Component
export default function Web3Hero() {
  const heroRef = useRef<HTMLElement>(null);
  const titleRef = useRef<HTMLHeadingElement>(null);
  const subtitleRef = useRef<HTMLParagraphElement>(null);
  const statsRef = useRef<HTMLDivElement>(null);
  const ctaRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Entrance animations
    const tl = gsap.timeline();
    
    tl.fromTo(titleRef.current, 
      { opacity: 0, y: 60 },
      { opacity: 1, y: 0, duration: 1.2, ease: "power3.out" }
    )
    .fromTo(subtitleRef.current,
      { opacity: 0, y: 30 },
      { opacity: 1, y: 0, duration: 1, ease: "power3.out" },
      "-=0.8"
    )
    .fromTo(statsRef.current?.children || [],
      { opacity: 0, y: 20 },
      { opacity: 1, y: 0, duration: 0.8, stagger: 0.1, ease: "power3.out" },
      "-=0.6"
    )
    .fromTo(ctaRef.current,
      { opacity: 0, y: 20 },
      { opacity: 1, y: 0, duration: 0.8, ease: "power3.out" },
      "-=0.4"
    );

    // Scroll-triggered animations
    const handleScroll = () => {
      const scrollY = window.scrollY;
      
      if (heroRef.current) {
        gsap.to(heroRef.current, {
          y: scrollY * 0.5,
          duration: 0.3,
          ease: "none"
        });
      }
    };

    window.addEventListener('scroll', handleScroll);
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  return (
    <section 
      ref={heroRef}
      className="relative min-h-screen flex items-center justify-center overflow-hidden"
      style={{
        background: 'linear-gradient(135deg, #0f172a 0%, #1e293b 50%, #334155 100%)',
        backgroundAttachment: 'fixed'
      }}
    >
      {/* Animated background gradient */}
      <div className="absolute inset-0 opacity-30">
        <div className="absolute top-0 left-0 w-full h-full bg-gradient-to-br from-blue-900/20 via-transparent to-purple-900/20 animate-pulse"></div>
      </div>

      <div className="relative z-10 max-w-7xl mx-auto px-6 grid lg:grid-cols-2 gap-16 items-center">
        {/* Left Content */}
        <div className="space-y-8">
          <div className="space-y-6">
            <h1 
              ref={titleRef}
              className="text-5xl lg:text-7xl font-light leading-tight text-white tracking-tight"
              style={{ fontFamily: 'Inter, system-ui, sans-serif' }}
            >
              THIS IS THE PLATFORM THAT MAKES{' '}
              <span className="font-extralight text-blue-200">GENIUS</span> LOOK EFFORTLESS
            </h1>
            
            <p 
              ref={subtitleRef}
              className="text-xl text-slate-300 font-light leading-relaxed max-w-lg"
            >
              Publish a 300-word idea. Get fast peer feedback. Find mentors & funding.
            </p>
          </div>

          {/* Stats */}
          <div 
            ref={statsRef}
            className="grid grid-cols-3 gap-8 py-8 border-t border-slate-700/50"
          >
            <div className="text-center">
              <div className="text-4xl font-light text-white mb-2">10K+</div>
              <div className="text-sm text-slate-400 uppercase tracking-wider">Research Papers</div>
            </div>
            <div className="text-center">
              <div className="text-4xl font-light text-white mb-2">500+</div>
              <div className="text-sm text-slate-400 uppercase tracking-wider">Journals</div>
            </div>
            <div className="text-center">
              <div className="text-4xl font-light text-white mb-2">95%</div>
              <div className="text-sm text-slate-400 uppercase tracking-wider">Accuracy</div>
            </div>
          </div>

          {/* CTA Buttons */}
          <div 
            ref={ctaRef}
            className="flex flex-col sm:flex-row gap-4"
          >
            <button className="group relative px-8 py-4 bg-white text-slate-900 font-medium rounded-full transition-all duration-300 hover:bg-slate-100 hover:scale-105 hover:shadow-2xl">
              <span className="relative z-10">Start Free</span>
              <div className="absolute inset-0 bg-gradient-to-r from-blue-400 to-purple-500 rounded-full opacity-0 group-hover:opacity-20 transition-opacity duration-300"></div>
            </button>
            
            <button className="px-8 py-4 border border-slate-600 text-white font-medium rounded-full transition-all duration-300 hover:border-slate-400 hover:bg-slate-800/50 hover:scale-105">
              Explore Mentors
            </button>
          </div>
        </div>

        {/* Right 3D Canvas */}
        <div className="relative">
          <div className="w-full h-[600px] lg:h-[700px] rounded-3xl overflow-hidden bg-gradient-to-br from-slate-800/50 to-slate-900/50 backdrop-blur-xl border border-slate-700/50 shadow-2xl">
            <Canvas
              camera={{ position: [0, 0, 5], fov: 45 }}
              gl={{ antialias: true, alpha: true }}
              dpr={Math.min(window.devicePixelRatio, 2)}
            >
              <Scene />
            </Canvas>
          </div>
          
          {/* Subtle glow effect */}
          <div className="absolute -inset-1 bg-gradient-to-r from-blue-500/20 via-purple-500/20 to-blue-500/20 rounded-3xl blur-xl opacity-50"></div>
        </div>
      </div>

      {/* Scroll indicator */}
      <div className="absolute bottom-8 left-1/2 transform -translate-x-1/2">
        <div className="w-6 h-10 border-2 border-slate-400 rounded-full flex justify-center">
          <div className="w-1 h-3 bg-slate-400 rounded-full mt-2 animate-bounce"></div>
        </div>
      </div>
    </section>
  );
}
