import React, { useRef } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Text, Center } from '@react-three/drei';
import { Mesh } from 'three';

const AnimatedText: React.FC<{ text: string; delay: number; color: string }> = ({ text, delay, color }) => {
  const textRef = useRef<Mesh>(null);

  useFrame((state) => {
    if (textRef.current) {
      const time = state.clock.elapsedTime + delay;
      
      // Create a circular motion around the globe
      const radius = 3.5; // Distance from globe center
      const angle = time * 0.3; // Rotation speed
      
      // Position text in a circle around the globe
      textRef.current.position.x = Math.cos(angle) * radius;
      textRef.current.position.z = Math.sin(angle) * radius;
      textRef.current.position.y = Math.sin(time * 0.5) * 0.2; // Gentle floating
      
      // Rotate text to face the camera
      textRef.current.lookAt(0, textRef.current.position.y, 0);
      
      // Scale animation - comes from behind (small) to front (normal) then back
      const scaleProgress = (Math.sin(angle) + 1) / 2; // 0 to 1
      const scale = 0.3 + (scaleProgress * 0.7); // Scale from 0.3 to 1.0
      textRef.current.scale.setScalar(scale);
      
      // Opacity animation - fade in when coming to front
      const opacity = Math.max(0.1, scaleProgress);
      if (textRef.current.material) {
        (textRef.current.material as any).opacity = opacity;
      }
    }
  });

  return (
    <Center>
      <Text
        ref={textRef}
        fontSize={0.3}
        color={color}
        anchorX="center"
        anchorY="middle"
        font="/fonts/inter-bold.woff"
      >
        {text}
      </Text>
    </Center>
  );
};

const ThreeJSTextAnimation: React.FC = () => {
  return (
    <div className="threejs-text-container">
      <Canvas
        camera={{ position: [0, 0, 8], fov: 45 }}
        style={{ width: '100%', height: '100%' }}
        gl={{ 
          antialias: true, 
          alpha: true,
          powerPreference: "high-performance"
        }}
      >
        {/* Lighting for text */}
        <ambientLight intensity={0.6} />
        <directionalLight position={[10, 10, 5]} intensity={1.0} />
        <pointLight position={[-10, -10, -5]} intensity={0.5} color="#222222" />
        
        {/* Text animations with different delays for sequence */}
        <AnimatedText text="10K+ RESEARCH PAPERS" delay={0} color="#222222" />
        <AnimatedText text="500+ JOURNALS" delay={2} color="#222222" />
        <AnimatedText text="95% ACCURACY" delay={4} color="#222222" />
      </Canvas>
    </div>
  );
};

export default ThreeJSTextAnimation;




