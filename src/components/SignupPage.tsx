import React, { useState } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Environment } from '@react-three/drei';
import * as THREE from 'three';
import './SignupPage.css';

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
      {Array.from({ length: 10 }).map((_, i) => (
        <mesh 
          key={i} 
          position={[
            Math.sin(i * 0.6) * 7,
            Math.cos(i * 0.6) * 7,
            Math.sin(i * 0.3) * 3
          ]}
        >
          <torusGeometry args={[0.25, 0.08, 8, 16]} />
          <meshStandardMaterial 
            color="#535353" 
            transparent
            opacity={0.25}
          />
        </mesh>
      ))}
      
      {/* Central Orb */}
      <mesh position={[0, 0, -1.5]}>
        <sphereGeometry args={[0.4, 32, 32]} />
        <meshStandardMaterial 
          color="#6c6c6c"
          metalness={0.7}
          roughness={0.3}
          transparent
          opacity={0.35}
        />
      </mesh>
    </group>
  );
};

interface SignupPageProps {
  onSignupSuccess: (token: string, user: any) => void;
  onSwitchToLogin: () => void;
}

const SignupPage: React.FC<SignupPageProps> = ({ onSignupSuccess, onSwitchToLogin }) => {
  const [formData, setFormData] = useState({
    email: '',
    password: '',
    confirmPassword: '',
    firstName: '',
    lastName: ''
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

  const validateForm = () => {
    if (formData.password !== formData.confirmPassword) {
      setError('Passwords do not match');
      return false;
    }
    if (formData.password.length < 8) {
      setError('Password must be at least 8 characters long');
      return false;
    }
    return true;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!validateForm()) return;
    
    setLoading(true);
    setError('');

    try {
      const response = await fetch('https://backend.gaply.in/api/v1/auth/register', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          email: formData.email,
          password: formData.password,
          first_name: formData.firstName,
          last_name: formData.lastName
        }),
      });

      const data = await response.json();

      if (data.success) {
        // Store token and user data
        localStorage.setItem('authToken', data.token);
        localStorage.setItem('user', JSON.stringify(data.user));
        
        // Call success callback
        onSignupSuccess(data.token, data.user);
      } else {
        setError(data.message || 'Registration failed. Please try again.');
      }
    } catch (err) {
      console.error('Signup error:', err);
      setError('Network error. Please try again.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="signup-page">
      {/* 3D Background */}
      <div className="signup-background-canvas">
        <Canvas camera={{ position: [0, 0, 8], fov: 75 }}>
          <ambientLight intensity={0.4} />
          <pointLight position={[5, 5, 5]} intensity={0.8} />
          <Environment preset="city" />
          <BackgroundElements />
        </Canvas>
      </div>

      {/* Signup Form */}
      <div className="signup-container">
        <div className="signup-card">
          <div className="signup-header">
            <h1>Join GAPLY</h1>
            <p>Create your account to get started</p>
          </div>

          <form onSubmit={handleSubmit} className="signup-form">
            <div className="form-row">
              <div className="form-group">
                <label htmlFor="firstName">First Name</label>
                <input
                  type="text"
                  id="firstName"
                  name="firstName"
                  value={formData.firstName}
                  onChange={handleInputChange}
                  required
                  placeholder="Enter your first name"
                  className={error ? 'error' : ''}
                />
              </div>

              <div className="form-group">
                <label htmlFor="lastName">Last Name</label>
                <input
                  type="text"
                  id="lastName"
                  name="lastName"
                  value={formData.lastName}
                  onChange={handleInputChange}
                  required
                  placeholder="Enter your last name"
                  className={error ? 'error' : ''}
                />
              </div>
            </div>

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
                placeholder="Create a password (min 8 characters)"
                className={error ? 'error' : ''}
              />
            </div>

            <div className="form-group">
              <label htmlFor="confirmPassword">Confirm Password</label>
              <input
                type="password"
                id="confirmPassword"
                name="confirmPassword"
                value={formData.confirmPassword}
                onChange={handleInputChange}
                required
                placeholder="Confirm your password"
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
              className="signup-btn"
              disabled={loading}
            >
              {loading ? 'Creating Account...' : 'Create Account'}
            </button>
          </form>

          <div className="signup-footer">
            <p>
              Already have an account?{' '}
              <button 
                type="button" 
                className="switch-btn"
                onClick={onSwitchToLogin}
              >
                Sign In
              </button>
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SignupPage;
