import React, { useState, useRef, useEffect } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls, Text } from '@react-three/drei';
import * as THREE from 'three';
import { authService } from '../services/authService';

interface SignupPageProps {
  onSignupSuccess: (token: string, user: any) => void;
  onSwitchToLogin: () => void;
}

// 3D Floating Signup Particles
const SignupParticles: React.FC = () => {
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const particles = useRef<THREE.Object3D[]>([]);

  useEffect(() => {
    if (meshRef.current) {
      for (let i = 0; i < 100; i++) {
        const particle = new THREE.Object3D();
        particle.position.set(
          (Math.random() - 0.5) * 30,
          (Math.random() - 0.5) * 30,
          (Math.random() - 0.5) * 30
        );
        particle.scale.setScalar(Math.random() * 0.3 + 0.1);
        particles.current.push(particle);
        meshRef.current.setMatrixAt(i, particle.matrix);
      }
      meshRef.current.instanceMatrix.needsUpdate = true;
    }
  }, []);

  useFrame((state) => {
    if (meshRef.current) {
      particles.current.forEach((particle, i) => {
        particle.rotation.y += 0.006;
        particle.rotation.x += 0.003;
        particle.position.y += Math.sin(state.clock.elapsedTime + i * 0.08) * 0.001;
        particle.updateMatrix();
        meshRef.current!.setMatrixAt(i, particle.matrix);
      });
      meshRef.current.instanceMatrix.needsUpdate = true;
    }
  });

  return (
    <instancedMesh ref={meshRef} args={[undefined, undefined, 100]}>
      <sphereGeometry args={[0.06, 6, 6]} />
      <meshBasicMaterial color="#30D158" transparent opacity={0.2} />
    </instancedMesh>
  );
};

// 3D Signup Form Elements
const SignupForm3D: React.FC<{
  firstName: string;
  lastName: string;
  email: string;
  password: string;
  confirmPassword: string;
  loading: boolean;
}> = ({ firstName, lastName, email, password, confirmPassword, loading }) => {
  const firstNameRef = useRef<THREE.Mesh>(null);
  const lastNameRef = useRef<THREE.Mesh>(null);
  const emailRef = useRef<THREE.Mesh>(null);
  const passwordRef = useRef<THREE.Mesh>(null);
  const confirmPasswordRef = useRef<THREE.Mesh>(null);
  const buttonRef = useRef<THREE.Mesh>(null);

  useFrame((state) => {
    const time = state.clock.elapsedTime;
    
    if (firstNameRef.current) {
      firstNameRef.current.rotation.y = Math.sin(time) * 0.03;
      firstNameRef.current.position.y = Math.sin(time * 0.3) * 0.01;
    }
    if (lastNameRef.current) {
      lastNameRef.current.rotation.y = Math.sin(time + Math.PI/2) * 0.03;
      lastNameRef.current.position.y = Math.sin(time * 0.3 + Math.PI/2) * 0.01;
    }
    if (emailRef.current) {
      emailRef.current.rotation.y = Math.sin(time + Math.PI) * 0.03;
      emailRef.current.position.y = Math.sin(time * 0.3 + Math.PI) * 0.01;
    }
    if (passwordRef.current) {
      passwordRef.current.rotation.y = Math.sin(time + Math.PI * 1.5) * 0.03;
      passwordRef.current.position.y = Math.sin(time * 0.3 + Math.PI * 1.5) * 0.01;
    }
    if (confirmPasswordRef.current) {
      confirmPasswordRef.current.rotation.y = Math.sin(time + Math.PI * 2) * 0.03;
      confirmPasswordRef.current.position.y = Math.sin(time * 0.3 + Math.PI * 2) * 0.01;
    }
    if (buttonRef.current) {
      buttonRef.current.scale.setScalar(1 + Math.sin(time * 3) * 0.03);
    }
  });

  return (
    <group>
      {/* First Name Field */}
      <mesh ref={firstNameRef} position={[-2, 2, 0]}>
        <boxGeometry args={[1.8, 0.6, 0.08]} />
        <meshStandardMaterial 
          color={firstName ? "#007AFF" : "#333333"} 
          metalness={0.8}
          roughness={0.2}
          emissive={firstName ? "#007AFF" : "#000000"}
          emissiveIntensity={firstName ? 0.1 : 0}
        />
      </mesh>
      
      {/* Last Name Field */}
      <mesh ref={lastNameRef} position={[2, 2, 0]}>
        <boxGeometry args={[1.8, 0.6, 0.08]} />
        <meshStandardMaterial 
          color={lastName ? "#007AFF" : "#333333"} 
          metalness={0.8}
          roughness={0.2}
          emissive={lastName ? "#007AFF" : "#000000"}
          emissiveIntensity={lastName ? 0.1 : 0}
        />
      </mesh>

      {/* Email Field */}
      <mesh ref={emailRef} position={[0, 1, 0]}>
        <boxGeometry args={[4, 0.6, 0.08]} />
        <meshStandardMaterial 
          color={email ? "#30D158" : "#333333"} 
          metalness={0.8}
          roughness={0.2}
          emissive={email ? "#30D158" : "#000000"}
          emissiveIntensity={email ? 0.1 : 0}
        />
      </mesh>
      
      {/* Password Field */}
      <mesh ref={passwordRef} position={[0, 0, 0]}>
        <boxGeometry args={[4, 0.6, 0.08]} />
        <meshStandardMaterial 
          color={password ? "#FF9500" : "#333333"} 
          metalness={0.8}
          roughness={0.2}
          emissive={password ? "#FF9500" : "#000000"}
          emissiveIntensity={password ? 0.1 : 0}
        />
      </mesh>

      {/* Confirm Password Field */}
      <mesh ref={confirmPasswordRef} position={[0, -1, 0]}>
        <boxGeometry args={[4, 0.6, 0.08]} />
        <meshStandardMaterial 
          color={confirmPassword ? "#FF9500" : "#333333"} 
          metalness={0.8}
          roughness={0.2}
          emissive={confirmPassword ? "#FF9500" : "#000000"}
          emissiveIntensity={confirmPassword ? 0.1 : 0}
        />
      </mesh>

      {/* Signup Button */}
      <mesh ref={buttonRef} position={[0, -2.5, 0]}>
        <boxGeometry args={[3, 0.5, 0.08]} />
        <meshStandardMaterial 
          color={loading ? "#FF3B30" : "#30D158"} 
          metalness={0.8}
          roughness={0.2}
          emissive={loading ? "#FF3B30" : "#30D158"}
          emissiveIntensity={0.2}
        />
      </mesh>
    </group>
  );
};

const SignupPage: React.FC<SignupPageProps> = ({ onSignupSuccess, onSwitchToLogin }) => {
  const [formData, setFormData] = useState({
    firstName: '',
    lastName: '',
    email: '',
    password: '',
    confirmPassword: ''
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

    // Validate passwords match
    if (formData.password !== formData.confirmPassword) {
      setError('Passwords do not match. Please try again.');
      setLoading(false);
      return;
    }

    // Validate password length
    if (formData.password.length < 8) {
      setError('Password must be at least 8 characters long.');
      setLoading(false);
      return;
    }

    try {
      const response = await authService.register({
        email: formData.email,
        password: formData.password,
        first_name: formData.firstName,
        last_name: formData.lastName,
      });

      if (response.success && response.data) {
        localStorage.setItem('authToken', response.data.token);
        localStorage.setItem('user', JSON.stringify(response.data.user));
        localStorage.setItem('user_email', response.data.user.email);
        onSignupSuccess(response.data.token, response.data.user);
      } else {
        throw new Error(response.error || 'Registration failed');
      }
    } catch (err: any) {
      console.error('Signup error:', err);
      
      if (err.message.includes('already exists')) {
        setError('An account with this email already exists. Please try logging in instead.');
      } else if (err.message.includes('Password must be at least 8 characters')) {
        setError('Password must be at least 8 characters long.');
      } else {
        setError(err.message || 'Registration failed. Please try again.');
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
        <Canvas camera={{ position: [0, 0, 10], fov: 60 }}>
          <ambientLight intensity={0.2} />
          <pointLight position={[10, 10, 10]} intensity={0.5} />
          <pointLight position={[-10, -10, -10]} intensity={0.3} color="#30D158" />
          
          <SignupParticles />
          <SignupForm3D 
            firstName={formData.firstName}
            lastName={formData.lastName}
            email={formData.email}
            password={formData.password}
            confirmPassword={formData.confirmPassword}
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
          maxWidth: '500px',
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
              textShadow: '0 0 20px rgba(48, 209, 88, 0.5)'
            }}>
              Join GAPLY
            </h1>
            <p style={{
              fontSize: '14px',
              fontWeight: '400',
              color: '#888888',
              margin: 0,
              lineHeight: '1.3'
            }}>
              Create your account to get started
            </p>
          </div>

          {/* Signup Form */}
          <form onSubmit={handleSubmit} style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '16px'
          }}>
            {/* Name Fields */}
            <div style={{
              display: 'grid',
              gridTemplateColumns: '1fr 1fr',
              gap: '12px'
            }}>
              <div>
                <label style={{
                  display: 'block',
                  fontSize: '12px',
                  fontWeight: '500',
                  color: '#ffffff',
                  marginBottom: '6px'
                }}>
                  First Name
                </label>
                <input
                  type="text"
                  name="firstName"
                  value={formData.firstName}
                  onChange={handleInputChange}
                  required
                  placeholder="First name"
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
                  Last Name
                </label>
                <input
                  type="text"
                  name="lastName"
                  value={formData.lastName}
                  onChange={handleInputChange}
                  required
                  placeholder="Last name"
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
            </div>

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
                  e.currentTarget.style.borderColor = '#FF9500';
                  e.currentTarget.style.background = 'rgba(255, 149, 0, 0.1)';
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
                Confirm Password
              </label>
              <input
                type="password"
                name="confirmPassword"
                value={formData.confirmPassword}
                onChange={handleInputChange}
                required
                placeholder="Confirm your password"
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
                  e.currentTarget.style.borderColor = '#FF9500';
                  e.currentTarget.style.background = 'rgba(255, 149, 0, 0.1)';
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
                {error.includes('already exists') && (
                  <button 
                    type="button"
                    onClick={onSwitchToLogin}
                    style={{
                      background: 'transparent',
                      color: '#007AFF',
                      border: '1px solid #007AFF',
                      padding: '6px 12px',
                      borderRadius: '16px',
                      fontSize: '10px',
                      fontWeight: '600',
                      cursor: 'pointer',
                      marginTop: '8px',
                      transition: 'all 0.3s ease',
                      display: 'block',
                      margin: '8px auto 0'
                    }}
                    onMouseOver={(e) => {
                      e.currentTarget.style.background = '#007AFF';
                      e.currentTarget.style.color = 'white';
                    }}
                    onMouseOut={(e) => {
                      e.currentTarget.style.background = 'transparent';
                      e.currentTarget.style.color = '#007AFF';
                    }}
                  >
                    Go to Login
                  </button>
                )}
              </div>
            )}

            <button
              type="submit"
              disabled={loading}
              style={{
                width: '100%',
                background: loading ? '#FF3B30' : '#30D158',
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
                  e.currentTarget.style.background = '#28A745';
                  e.currentTarget.style.transform = 'translateZ(5px) scale(1.02)';
                }
              }}
              onMouseOut={(e) => {
                if (!loading) {
                  e.currentTarget.style.background = '#30D158';
                  e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                }
              }}
            >
              {loading ? 'Creating Account...' : 'Create Account'}
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
              Already have an account?
            </p>
            <button
              onClick={onSwitchToLogin}
              style={{
                background: 'transparent',
                color: '#30D158',
                border: 'none',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
                textDecoration: 'underline',
                transition: 'all 0.3s ease'
              }}
              onMouseOver={(e) => {
                e.currentTarget.style.color = '#28A745';
              }}
              onMouseOut={(e) => {
                e.currentTarget.style.color = '#30D158';
              }}
            >
              Sign In
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SignupPage;