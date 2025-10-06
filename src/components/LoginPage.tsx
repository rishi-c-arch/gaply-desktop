import React, { useState } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Environment } from '@react-three/drei';
import * as THREE from 'three';
import './LoginPage.css';

// 3D Background Elements
const BackgroundElements: React.FC = () => {
  const groupRef = React.useRef<THREE.Group>(null);

  useFrame((state) => {
    if (groupRef.current) {
      groupRef.current.rotation.y = state.clock.elapsedTime * 0.1;
    }
  });

  return (
    <group ref={groupRef}>
      {/* Floating Geometric Shapes */}
      {Array.from({ length: 8 }).map((_, i) => (
        <mesh 
          key={i} 
          position={[
            Math.sin(i * 0.8) * 6,
            Math.cos(i * 0.8) * 6,
            Math.sin(i * 0.4) * 2
          ]}
        >
          <torusGeometry args={[0.2, 0.05, 8, 16]} />
          <meshStandardMaterial 
            color="#535353" 
            transparent
            opacity={0.2}
          />
        </mesh>
      ))}
      
      {/* Central Orb */}
      <mesh position={[0, 0, -1]}>
        <sphereGeometry args={[0.3, 32, 32]} />
        <meshStandardMaterial 
          color="#6c6c6c"
          metalness={0.7}
          roughness={0.3}
          transparent
          opacity={0.3}
        />
      </mesh>
    </group>
  );
};

interface LoginPageProps {
  onLoginSuccess: (token: string, user: any) => void;
  onSwitchToSignup: () => void;
}

const LoginPage: React.FC<LoginPageProps> = ({ onLoginSuccess, onSwitchToSignup }) => {
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
    // Clear error when user starts typing
    if (error) setError('');
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError('');

    try {
      const response = await fetch('https://backend.gaply.in/api/v1/auth/login', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          email: formData.email,
          password: formData.password
        }),
      });

      const data = await response.json();

      if (data.success) {
        // Store token and user data
        localStorage.setItem('authToken', data.token);
        localStorage.setItem('user', JSON.stringify(data.user));
        
        // Call success callback
        onLoginSuccess(data.token, data.user);
      } else {
        setError(data.message || 'Login failed. Please check your credentials.');
      }
    } catch (err) {
      console.error('Login error:', err);
      setError('Network error. Please try again.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="login-page">
      {/* 3D Background */}
      <div className="login-background-canvas">
        <Canvas camera={{ position: [0, 0, 8], fov: 75 }}>
          <ambientLight intensity={0.4} />
          <pointLight position={[5, 5, 5]} intensity={0.8} />
          <Environment preset="city" />
          <BackgroundElements />
        </Canvas>
      </div>

      {/* Login Form */}
      <div className="login-container">
        <div className="login-card">
          <div className="login-header">
            <h1>Welcome Back</h1>
            <p>Sign in to your GAPLY account</p>
          </div>

          <form onSubmit={handleSubmit} className="login-form">
            <div className="form-group">
              <label htmlFor="email">Email Address</label>
              <input
                type="email"
                id="email"
                name="email"
                value={formData.email}
                onChange={handleInputChange}
                required
                placeholder="Enter your email"
                className={error ? 'error' : ''}
              />
            </div>

            <div className="form-group">
              <label htmlFor="password">Password</label>
              <input
                type="password"
                id="password"
                name="password"
                value={formData.password}
                onChange={handleInputChange}
                required
                placeholder="Enter your password"
                className={error ? 'error' : ''}
              />
            </div>

            {error && (
              <div className="error-message">
                {error}
              </div>
            )}

            <button 
              type="submit" 
              className="login-btn"
              disabled={loading}
            >
              {loading ? 'Signing In...' : 'Sign In'}
            </button>
          </form>

          <div className="login-footer">
            <p>
              Don't have an account?{' '}
              <button 
                type="button" 
                className="switch-btn"
                onClick={onSwitchToSignup}
              >
                Sign Up
              </button>
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};

export default LoginPage;
