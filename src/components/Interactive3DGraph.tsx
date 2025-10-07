import React, { useRef, useMemo, useState } from 'react';
import { Canvas, useFrame, useThree } from '@react-three/fiber';
import { Line, Text, Sphere } from '@react-three/drei';
import * as THREE from 'three';

// Popular research topics data
const topicsData = [
  { name: 'AI/ML', popularity: 95, color: '#3b82f6', position: [0, 0, 0] as [number, number, number] },
  { name: 'Blockchain', popularity: 78, color: '#10b981', position: [2, 1, -1] as [number, number, number] },
  { name: 'Quantum Computing', popularity: 82, color: '#8b5cf6', position: [-2, 2, 1] as [number, number, number] },
  { name: 'Neuroscience', popularity: 88, color: '#f59e0b', position: [1, -1, 2] as [number, number, number] },
  { name: 'Climate Science', popularity: 92, color: '#ef4444', position: [-1, -2, -1] as [number, number, number] },
  { name: 'Biotechnology', popularity: 85, color: '#06b6d4', position: [3, 0, -2] as [number, number, number] },
  { name: 'Space Research', popularity: 76, color: '#84cc16', position: [-3, 1, 1] as [number, number, number] },
  { name: 'Renewable Energy', popularity: 89, color: '#f97316', position: [0, 3, -1] as [number, number, number] },
  { name: 'Cybersecurity', popularity: 91, color: '#ec4899', position: [-2, -1, 2] as [number, number, number] },
  { name: 'Materials Science', popularity: 83, color: '#6366f1', position: [2, -2, 0] as [number, number, number] }
];

// Animated data points component
function DataPoint({ position, color, size, topic, popularity, index }: {
  position: [number, number, number];
  color: string;
  size: number;
  topic: string;
  popularity: number;
  index: number;
}) {
  const meshRef = useRef<THREE.Mesh>(null);
  const [hovered, setHovered] = useState(false);

  useFrame((state) => {
    if (meshRef.current) {
      meshRef.current.rotation.x = Math.sin(state.clock.elapsedTime + index) * 0.1;
      meshRef.current.rotation.y = Math.sin(state.clock.elapsedTime + index) * 0.2;
      
      // Pulsing animation
      const scale = hovered ? 1.5 : 1 + Math.sin(state.clock.elapsedTime * 2 + index) * 0.1;
      meshRef.current.scale.setScalar(scale);
      
      // Floating animation
      meshRef.current.position.y = position[1] + Math.sin(state.clock.elapsedTime + index) * 0.3;
    }
  });

  return (
    <group position={position}>
      <Sphere
        ref={meshRef}
        args={[size, 32, 32]}
        onPointerOver={() => setHovered(true)}
        onPointerOut={() => setHovered(false)}
      >
        <meshStandardMaterial
          color={color}
          emissive={color}
          emissiveIntensity={hovered ? 0.3 : 0.1}
          metalness={0.8}
          roughness={0.2}
        />
      </Sphere>
      
      {/* Connection lines */}
      <Line
        points={[[0, 0, 0], [0, popularity * 0.1, 0]]}
        color={color}
        lineWidth={3}
        transparent
        opacity={0.6}
      />
      
      {/* Topic label */}
      <Text
        position={[0, popularity * 0.1 + 0.5, 0]}
        fontSize={0.3}
        color={color}
        anchorX="center"
        anchorY="middle"
      >
        {topic}
      </Text>
      
      {/* Popularity value */}
      <Text
        position={[0, popularity * 0.1 + 0.8, 0]}
        fontSize={0.25}
        color="#ffffff"
        anchorX="center"
        anchorY="middle"
      >
        {popularity}%
      </Text>
    </group>
  );
}

// Interactive line chart component
function LineChart({ data }: { data: typeof topicsData }) {
  const lineRef = useRef<THREE.Group>(null);
  
  useFrame((state) => {
    if (lineRef.current) {
      lineRef.current.rotation.y = Math.sin(state.clock.elapsedTime * 0.1) * 0.1;
    }
  });

  const points = useMemo(() => {
    return data.map((item, index) => [
      index * 0.8 - 4,
      (item.popularity / 100) * 3,
      Math.sin(index) * 0.5
    ] as [number, number, number]);
  }, [data]);

  return (
    <group ref={lineRef} position={[0, -2, 0]}>
      {/* Base grid */}
      {Array.from({ length: 10 }).map((_, i) => (
        <Line
          key={`grid-${i}`}
          points={[[-4, i * 0.3, 0], [4, i * 0.3, 0]]}
          color="#374151"
          lineWidth={1}
          transparent
          opacity={0.3}
        />
      ))}
      
      {/* Main trend line */}
      <Line
        points={points}
        color="#3b82f6"
        lineWidth={4}
        transparent
        opacity={0.8}
      />
      
      {/* Data points on line */}
      {points.map((point, index) => (
        <Sphere key={`point-${index}`} args={[0.1]} position={point}>
          <meshStandardMaterial color="#3b82f6" emissive="#3b82f6" emissiveIntensity={0.2} />
        </Sphere>
      ))}
    </group>
  );
}

// Camera controller
function CameraController() {
  const { camera } = useThree();
  
  useFrame((state) => {
    camera.position.x = Math.sin(state.clock.elapsedTime * 0.1) * 8;
    camera.position.y = Math.cos(state.clock.elapsedTime * 0.1) * 2 + 2;
    camera.position.z = Math.cos(state.clock.elapsedTime * 0.1) * 8;
    camera.lookAt(0, 0, 0);
  });
  
  return null;
}

// Main 3D Graph Component
const Interactive3DGraph: React.FC = () => {
  return (
    <section className="py-20 bg-gradient-to-br from-slate-50 via-blue-50 to-indigo-100 relative overflow-hidden">
      {/* Background decorative elements */}
      <div className="absolute inset-0 opacity-10">
        <div className="absolute top-20 left-20 w-32 h-32 bg-blue-400 rounded-full blur-3xl"></div>
        <div className="absolute bottom-20 right-20 w-40 h-40 bg-purple-400 rounded-full blur-3xl"></div>
        <div className="absolute top-1/2 left-1/2 w-24 h-24 bg-indigo-400 rounded-full blur-2xl"></div>
      </div>
      
      <div className="max-w-7xl mx-auto px-6 relative z-10">
        {/* Header */}
        <div className="text-center mb-16">
          <h2 className="text-4xl md:text-5xl font-bold bg-gradient-to-r from-gray-900 via-blue-900 to-indigo-900 bg-clip-text text-transparent mb-6">
            Research Trends Analysis
          </h2>
          <p className="text-xl text-gray-600 max-w-3xl mx-auto leading-relaxed">
            Interactive visualization of trending research topics with real-time popularity metrics
          </p>
        </div>

        {/* 3D Graph Container */}
        <div className="bg-white/80 backdrop-blur-sm rounded-3xl shadow-2xl border border-white/20 p-8 mb-12">
          <div className="h-[600px] relative">
            <Canvas
              camera={{ position: [8, 4, 8], fov: 60 }}
              style={{ background: 'transparent' }}
            >
              {/* Lighting */}
              <ambientLight intensity={0.4} />
              <directionalLight position={[10, 10, 5]} intensity={1} />
              <pointLight position={[-10, -10, -5]} color="#3b82f6" intensity={0.5} />
              
              {/* Camera controller */}
              <CameraController />
              
              {/* Data points */}
              {topicsData.map((topic, index) => (
                <DataPoint
                  key={topic.name}
                  position={topic.position}
                  color={topic.color}
                  size={topic.popularity / 100 + 0.3}
                  topic={topic.name}
                  popularity={topic.popularity}
                  index={index}
                />
              ))}
              
              {/* Line chart */}
              <LineChart data={topicsData} />
              
              {/* Connection lines between topics */}
              {topicsData.map((topic, index) => {
                const nextTopic = topicsData[(index + 1) % topicsData.length];
                return (
                  <Line
                    key={`connection-${index}`}
                    points={[topic.position, nextTopic.position]}
                    color="#64748b"
                    lineWidth={1}
                    transparent
                    opacity={0.3}
                    dashed
                  />
                );
              })}
            </Canvas>
            
            {/* Overlay information */}
            <div className="absolute top-4 left-4 bg-black/20 backdrop-blur-sm rounded-lg p-4 text-white">
              <h3 className="font-semibold mb-2">Interactive Controls</h3>
              <p className="text-sm opacity-90">• Hover over spheres to highlight</p>
              <p className="text-sm opacity-90">• Camera auto-rotates</p>
              <p className="text-sm opacity-90">• 3D visualization</p>
            </div>
          </div>
        </div>

        {/* Statistics Grid */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-6 mb-12">
          {topicsData.slice(0, 8).map((topic, index) => (
            <div
              key={topic.name}
              className="bg-white/60 backdrop-blur-sm rounded-xl p-6 border border-white/20 hover:bg-white/80 transition-all duration-300 group"
              style={{
                animationDelay: `${index * 100}ms`,
                animation: 'fadeInUp 0.6s ease-out forwards'
              }}
            >
              <div className="flex items-center justify-between mb-3">
                <div
                  className="w-4 h-4 rounded-full"
                  style={{ backgroundColor: topic.color }}
                ></div>
                <span className="text-2xl font-bold text-gray-800">{topic.popularity}%</span>
              </div>
              <h3 className="font-semibold text-gray-800 mb-1">{topic.name}</h3>
              <div className="w-full bg-gray-200 rounded-full h-2">
                <div
                  className="h-2 rounded-full transition-all duration-1000 group-hover:h-3"
                  style={{
                    width: `${topic.popularity}%`,
                    backgroundColor: topic.color
                  }}
                ></div>
              </div>
            </div>
          ))}
        </div>

        {/* Call to Action */}
        <div className="text-center">
          <button className="bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-700 hover:to-indigo-700 text-white px-8 py-4 rounded-xl font-semibold text-lg transition-all duration-300 transform hover:scale-105 shadow-lg hover:shadow-xl">
            Explore Research Insights
          </button>
        </div>
      </div>

    </section>
  );
};

export default Interactive3DGraph;