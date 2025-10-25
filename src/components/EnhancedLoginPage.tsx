import React, { useState, useRef, useEffect } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls, Text } from '@react-three/drei';
import * as THREE from 'three';
import { useAuth } from '../contexts/AuthContext';

interface EnhancedLoginPageProps {
  onSwitchToSignup: () => void;
  onLoginSuccess?: () => void;
}

// 3D Floating Auth Particles
const AuthParticles: React.FC = () => {
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const particles = useRef<THREE.Object3D[]>([]);

  useEffect(() => {
    if (meshRef.current) {
      for (let i = 0; i < 80; i++) {
        const particle = new THREE.Object3D();
        particle.position.set(
          (Math.random() - 0.5) * 25,
          (Math.random() - 0.5) * 25,
          (Math.random() - 0.5) * 25
        );
        particle.scale.setScalar(Math.random() * 0.4 + 0.2);
        particles.current.push(particle);
        meshRef.current.setMatrixAt(i, particle.matrix);
      }
      meshRef.current.instanceMatrix.needsUpdate = true;
    }
  }, []);

  useFrame((state) => {
    if (meshRef.current) {
      particles.current.forEach((particle, i) => {
        particle.rotation.y += 0.008;
        particle.rotation.x += 0.004;
        particle.position.y += Math.sin(state.clock.elapsedTime + i * 0.1) * 0.001;
        particle.updateMatrix();
        meshRef.current!.setMatrixAt(i, particle.matrix);
      });
      meshRef.current.instanceMatrix.needsUpdate = true;
    }
  });

  return (
    <instancedMesh ref={meshRef} args={[undefined, undefined, 80]}>
      <sphereGeometry args={[0.08, 6, 6]} />
      <meshBasicMaterial color="#007AFF" transparent opacity={0.25} />
    </instancedMesh>
  );
};

// 3D Login Form Elements
const LoginForm3D: React.FC<{
  email: string;
  password: string;
  loading: boolean;
}> = ({ email, password, loading }) => {
  const emailRef = useRef<THREE.Mesh>(null);
  const passwordRef = useRef<THREE.Mesh>(null);
  const buttonRef = useRef<THREE.Mesh>(null);

  useFrame((state) => {
    if (emailRef.current) {
      emailRef.current.rotation.y = Math.sin(state.clock.elapsedTime) * 0.05;
      emailRef.current.position.y = Math.sin(state.clock.elapsedTime * 0.5) * 0.02;
    }
    if (passwordRef.current) {
      passwordRef.current.rotation.y = Math.sin(state.clock.elapsedTime + Math.PI) * 0.05;
      passwordRef.current.position.y = Math.sin(state.clock.elapsedTime * 0.5 + Math.PI) * 0.02;
    }
    if (buttonRef.current) {
      buttonRef.current.scale.setScalar(1 + Math.sin(state.clock.elapsedTime * 2) * 0.05);
    }
  });

  return (
    <group>
      {/* Email Field */}
      <mesh ref={emailRef} position={[0, 1, 0]}>
        <boxGeometry args={[4, 0.8, 0.1]} />
        <meshStandardMaterial 
          color={email ? "#007AFF" : "#333333"} 
          metalness={0.8}
          roughness={0.2}
          emissive={email ? "#007AFF" : "#000000"}
          emissiveIntensity={email ? 0.1 : 0}
        />
      </mesh>
      
      {/* Password Field */}
      <mesh ref={passwordRef} position={[0, 0, 0]}>
        <boxGeometry args={[4, 0.8, 0.1]} />
        <meshStandardMaterial 
          color={password ? "#30D158" : "#333333"} 
          metalness={0.8}
          roughness={0.2}
          emissive={password ? "#30D158" : "#000000"}
          emissiveIntensity={password ? 0.1 : 0}
        />
      </mesh>

      {/* Login Button */}
      <mesh ref={buttonRef} position={[0, -1, 0]}>
        <boxGeometry args={[3, 0.6, 0.1]} />
        <meshStandardMaterial 
          color={loading ? "#FF9500" : "#007AFF"} 
          metalness={0.8}
          roughness={0.2}
          emissive={loading ? "#FF9500" : "#007AFF"}
          emissiveIntensity={0.2}
        />
      </mesh>

      {/* Labels */}
      <Text
        position={[0, 1.6, 0.1]}
        fontSize={0.2}
        color="#ffffff"
        anchorX="center"
        anchorY="middle"
      >
        Email Address
      </Text>
      
      <Text
        position={[0, 0.6, 0.1]}
        fontSize={0.2}
        color="#ffffff"
        anchorX="center"
        anchorY="middle"
      >
        Password
      </Text>
      
      <Text
        position={[0, -0.4, 0.1]}
        fontSize={0.25}
        color="#ffffff"
        anchorX="center"
        anchorY="middle"
      >
        {loading ? "Signing In..." : "Sign In"}
      </Text>
    </group>
  );
};

const EnhancedLoginPage: React.FC<EnhancedLoginPageProps> = ({ 
  onSwitchToSignup, 
  onLoginSuccess 
}) => {
  const { login } = useAuth();
  const [formData, setFormData] = useState({
    email: '',
    password: ''
  });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const { name, value } = e.target;
    setFormData(prev => ({
      ...prev,
      [name]: value
    }));
    setError('');
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);

    try {
      const result = await login({
        email: formData.email,
        password: formData.password,
      });

      if (result.success) {
        onLoginSuccess?.();
      } else {
        setError(result.error || 'Login failed. Please try again.');
      }
    } catch (err: any) {
      console.error('Login error:', err);
      setError(err.message || 'Login failed. Please try again.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{
      position: 'fixed',
      top: 0,
      left: 0,
      right: 0,
      bottom: 0,
      background: '#000000',
      zIndex: 1000,
      fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif',
      overflow: 'hidden'
    }}>
      {/* 3D Background Canvas */}
      <div style={{
        position: 'absolute',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        zIndex: 1
      }}>
        <Canvas camera={{ position: [0, 0, 8], fov: 60 }}>
          <ambientLight intensity={0.2} />
          <pointLight position={[10, 10, 10]} intensity={0.5} />
          <pointLight position={[-10, -10, -10]} intensity={0.3} color="#007AFF" />
          
          <AuthParticles />
          <LoginForm3D 
            email={formData.email}
            password={formData.password}
            loading={loading}
          />
          
          <OrbitControls enableZoom={false} enablePan={false} />
        </Canvas>
      </div>

      {/* UI Overlay */}
      <div style={{
        position: 'relative',
        zIndex: 2,
        height: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: '20px'
      }}>
        <div style={{
          background: 'rgba(255, 255, 255, 0.05)',
          borderRadius: '20px',
          padding: '40px',
          maxWidth: '400px',
          width: '100%',
          backdropFilter: 'blur(20px)',
          border: '1px solid rgba(255, 255, 255, 0.1)',
          transform: 'translateZ(0)',
          transformStyle: 'preserve-3d',
          boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)'
        }}>
          {/* Header */}
          <div style={{
            textAlign: 'center',
            marginBottom: '30px'
          }}>
            <h1 style={{
              fontSize: '28px',
              fontWeight: '600',
              color: '#ffffff',
              margin: '0 0 8px 0',
              letterSpacing: '-0.02em',
              lineHeight: '1.1',
              textShadow: '0 0 20px rgba(0, 122, 255, 0.5)'
            }}>
              Welcome back.
            </h1>
            <p style={{
              fontSize: '14px',
              fontWeight: '400',
              color: '#888888',
              margin: 0,
              lineHeight: '1.3'
            }}>
              Sign in to your GAPLY account
            </p>
          </div>

          {/* Login Form */}
          <form onSubmit={handleSubmit} style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '16px'
          }}>
            <div>
              <label style={{
                display: 'block',
                fontSize: '12px',
                fontWeight: '500',
                color: '#ffffff',
                marginBottom: '6px'
              }}>
                Email Address
              </label>
              <input
                type="email"
                name="email"
                value={formData.email}
                onChange={handleInputChange}
                required
                placeholder="Enter your email"
                style={{
                  width: '100%',
                  background: 'rgba(255, 255, 255, 0.05)',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  borderRadius: '8px',
                  padding: '12px 16px',
                  fontSize: '14px',
                  color: '#ffffff',
                  outline: 'none',
                  transition: 'all 0.3s ease',
                  backdropFilter: 'blur(10px)',
                  transform: 'translateZ(0)',
                  transformStyle: 'preserve-3d'
                }}
                onFocus={(e) => {
                  e.currentTarget.style.borderColor = '#007AFF';
                  e.currentTarget.style.background = 'rgba(0, 122, 255, 0.1)';
                  e.currentTarget.style.transform = 'translateZ(5px) scale(1.02)';
                }}
                onBlur={(e) => {
                  e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.05)';
                  e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                }}
              />
            </div>

            <div>
              <label style={{
                display: 'block',
                fontSize: '12px',
                fontWeight: '500',
                color: '#ffffff',
                marginBottom: '6px'
              }}>
                Password
              </label>
              <input
                type="password"
                name="password"
                value={formData.password}
                onChange={handleInputChange}
                required
                placeholder="Enter your password"
                style={{
                  width: '100%',
                  background: 'rgba(255, 255, 255, 0.05)',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  borderRadius: '8px',
                  padding: '12px 16px',
                  fontSize: '14px',
                  color: '#ffffff',
                  outline: 'none',
                  transition: 'all 0.3s ease',
                  backdropFilter: 'blur(10px)',
                  transform: 'translateZ(0)',
                  transformStyle: 'preserve-3d'
                }}
                onFocus={(e) => {
                  e.currentTarget.style.borderColor = '#30D158';
                  e.currentTarget.style.background = 'rgba(48, 209, 88, 0.1)';
                  e.currentTarget.style.transform = 'translateZ(5px) scale(1.02)';
                }}
                onBlur={(e) => {
                  e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.05)';
                  e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                }}
              />
            </div>

            {error && (
              <div style={{
                background: 'rgba(255, 59, 48, 0.1)',
                border: '1px solid rgba(255, 59, 48, 0.2)',
                borderRadius: '8px',
                padding: '12px',
                fontSize: '12px',
                color: '#FF3B30',
                textAlign: 'center'
              }}>
                {error}
              </div>
            )}

            <button
              type="submit"
              disabled={loading}
              style={{
                width: '100%',
                background: loading ? '#FF9500' : '#007AFF',
                color: 'white',
                border: 'none',
                borderRadius: '8px',
                padding: '12px 20px',
                fontSize: '14px',
                fontWeight: '600',
                cursor: loading ? 'not-allowed' : 'pointer',
                transition: 'all 0.3s ease',
                transform: 'translateZ(0)',
                transformStyle: 'preserve-3d',
                marginTop: '8px'
              }}
              onMouseOver={(e) => {
                if (!loading) {
                  e.currentTarget.style.background = '#0056CC';
                  e.currentTarget.style.transform = 'translateZ(5px) scale(1.02)';
                }
              }}
              onMouseOut={(e) => {
                if (!loading) {
                  e.currentTarget.style.background = '#007AFF';
                  e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                }
              }}
            >
              {loading ? 'Signing In...' : 'Sign In'}
            </button>
          </form>

          {/* Footer */}
          <div style={{
            textAlign: 'center',
            marginTop: '24px',
            paddingTop: '20px',
            borderTop: '1px solid rgba(255, 255, 255, 0.1)'
          }}>
            <p style={{
              fontSize: '12px',
              color: '#888888',
              margin: '0 0 12px 0'
            }}>
              Don't have an account?
            </p>
            <button
              onClick={onSwitchToSignup}
              style={{
                background: 'transparent',
                color: '#007AFF',
                border: 'none',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
                textDecoration: 'underline',
                transition: 'all 0.3s ease'
              }}
              onMouseOver={(e) => {
                e.currentTarget.style.color = '#0056CC';
              }}
              onMouseOut={(e) => {
                e.currentTarget.style.color = '#007AFF';
              }}
            >
              Create Account
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default EnhancedLoginPage;