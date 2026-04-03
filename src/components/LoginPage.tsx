import React, { useState, useRef, useEffect } from 'react';
import { useSearchParams, useNavigate } from 'react-router-dom';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import * as THREE from 'three';
import { authService } from '../services/authService';

interface LoginPageProps {
  onLoginSuccess: (token: string, user: any, redirect?: string) => void;
  onSwitchToSignup: () => void;
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
    </group>
  );
};

const LoginPage: React.FC<LoginPageProps> = ({ onLoginSuccess, onSwitchToSignup }) => {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const redirect = searchParams.get('redirect') || undefined;
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
      const response = await authService.login({
        email: formData.email,
        password: formData.password,
      });

      if (response.success && response.data && response.data.token) {
        localStorage.setItem('authToken', response.data.token);
        localStorage.setItem('user', JSON.stringify(response.data.user));
        localStorage.setItem('user_email', response.data.user.email);
        
        onLoginSuccess(response.data.token, response.data.user, redirect);
      } else {
        throw new Error(response.error || 'Login failed');
      }
    } catch (err: any) {
      console.error('Login error:', err);
      
      if (err.message.includes('Invalid email or password')) {
        setError('Invalid email or password. Please check your credentials and try again.');
      } else if (err.message.includes('User not found')) {
        setError('No account found with this email address. Please sign up first.');
      } else if (err.message.includes('Cannot read properties of undefined')) {
        setError('Server connection error. Please try again.');
      } else {
        setError(err.message || 'Login failed. Please try again.');
      }
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
      background: 'var(--app-bg)',
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
          background: 'var(--card-bg)',
          borderRadius: '20px',
          padding: '40px',
          maxWidth: '400px',
          width: '100%',
          backdropFilter: 'blur(20px)',
          border: '1px solid var(--card-border)',
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
              color: 'var(--app-text)',
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
              color: 'var(--muted-text)',
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
                color: 'var(--app-text)',
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
                  background: 'var(--toggle-bg)',
                  border: '1px solid var(--toggle-border)',
                  borderRadius: '8px',
                  padding: '12px 16px',
                  fontSize: '14px',
                  color: 'var(--app-text)',
                  outline: 'none',
                  transition: 'all 0.3s ease',
                  backdropFilter: 'blur(10px)',
                  transform: 'translateZ(0)',
                  transformStyle: 'preserve-3d'
                }}
                onFocus={(e) => {
                  e.currentTarget.style.borderColor = 'var(--button-bg)';
                  e.currentTarget.style.background = 'var(--card-bg)';
                  e.currentTarget.style.transform = 'translateZ(5px) scale(1.02)';
                }}
                onBlur={(e) => {
                  e.currentTarget.style.borderColor = 'var(--toggle-border)';
                  e.currentTarget.style.background = 'var(--toggle-bg)';
                  e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                }}
              />
            </div>

            <div>
              <label style={{
                display: 'block',
                fontSize: '12px',
                fontWeight: '500',
                color: 'var(--app-text)',
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
                  background: 'var(--toggle-bg)',
                  border: '1px solid var(--toggle-border)',
                  borderRadius: '8px',
                  padding: '12px 16px',
                  fontSize: '14px',
                  color: 'var(--app-text)',
                  outline: 'none',
                  transition: 'all 0.3s ease',
                  backdropFilter: 'blur(10px)',
                  transform: 'translateZ(0)',
                  transformStyle: 'preserve-3d'
                }}
                onFocus={(e) => {
                  e.currentTarget.style.borderColor = 'var(--button-bg)';
                  e.currentTarget.style.background = 'var(--card-bg)';
                  e.currentTarget.style.transform = 'translateZ(5px) scale(1.02)';
                }}
                onBlur={(e) => {
                  e.currentTarget.style.borderColor = 'var(--toggle-border)';
                  e.currentTarget.style.background = 'var(--toggle-bg)';
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
                background: loading ? '#FF9500' : 'var(--hero-cta-bg)',
                color: 'var(--hero-cta-text)',
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
                  e.currentTarget.style.background = 'var(--hero-cta-bg-hover)';
                  e.currentTarget.style.transform = 'translateZ(5px) scale(1.02)';
                }
              }}
              onMouseOut={(e) => {
                if (!loading) {
                  e.currentTarget.style.background = 'var(--hero-cta-bg)';
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
            borderTop: '1px solid var(--divider)'
          }}>
            <p style={{
              fontSize: '12px',
              color: 'var(--muted-text)',
              margin: '0 0 12px 0'
            }}>
              Don't have an account?
            </p>
            <button
              onClick={() => (redirect ? navigate(`/signup?redirect=${encodeURIComponent(redirect)}`) : onSwitchToSignup())}
              style={{
                background: 'transparent',
                color: 'var(--accent-blue)',
                border: 'none',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
                textDecoration: 'underline',
                transition: 'all 0.3s ease'
              }}
              onMouseOver={(e) => {
                e.currentTarget.style.color = '#2563eb';
              }}
              onMouseOut={(e) => {
                e.currentTarget.style.color = 'var(--accent-blue)';
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

export default LoginPage;