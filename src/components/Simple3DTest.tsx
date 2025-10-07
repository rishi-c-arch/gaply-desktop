import React from 'react';
import { Canvas } from '@react-three/fiber';
import { Sphere } from '@react-three/drei';

const Simple3DTest: React.FC = () => {
  return (
    <section className="py-20 bg-gradient-to-br from-blue-50 to-indigo-100">
      <div className="max-w-7xl mx-auto px-6">
        <div className="text-center mb-8">
          <h2 className="text-3xl font-bold text-gray-800 mb-4">3D Test Component</h2>
          <p className="text-gray-600">Testing Three.js integration</p>
        </div>
        
        <div className="bg-white rounded-xl shadow-lg p-8">
          <div className="h-[400px]">
            <Canvas camera={{ position: [0, 0, 5] }}>
              <ambientLight intensity={0.5} />
              <pointLight position={[10, 10, 10]} />
              <Sphere args={[1, 32, 32]} position={[0, 0, 0]}>
                <meshStandardMaterial color="#3b82f6" />
              </Sphere>
            </Canvas>
          </div>
        </div>
      </div>
    </section>
  );
};

export default Simple3DTest;
