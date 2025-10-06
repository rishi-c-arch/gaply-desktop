import React, { useState } from 'react';
import './PremiumSignupBackground.css';
import './GaplySignupStyles.css';
import PremiumSignupBackground from './PremiumSignupBackground';


interface SignupPageProps {
  onSignupSuccess: (token: string, user: any) => void;
  onSwitchToLogin: () => void;
}

const SignupPage: React.FC<SignupPageProps> = ({ onSignupSuccess, onSwitchToLogin }) => {
  console.log('SignupPage component rendering'); // Debug log
  
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
    <div 
      className="gaply-signup-page"
      style={{
        position: 'relative',
        width: '100%',
        height: '100vh',
        overflow: 'hidden',
        background: 'linear-gradient(135deg, #0A0A0A 0%, #1a1a1a 30%, #2a2a2a 70%, #0A0A0A 100%)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: '100vh'
      }}
    >
      {/* Premium 3D Background */}
      <PremiumSignupBackground />

      {/* Signup Form */}
      <div 
        className="gaply-signup-container"
        style={{
          position: 'relative',
          zIndex: 15,
          width: '100%',
          maxWidth: '550px',
          padding: '25px',
          transform: 'perspective(1200px) rotateX(3deg) rotateY(-1deg)',
          transition: 'all 0.4s cubic-bezier(0.4, 0, 0.2, 1)'
        }}
      >
        <div 
          className="gaply-signup-card"
          style={{
            background: 'rgba(255, 255, 255, 0.02)',
            backdropFilter: 'blur(40px)',
            border: '2px solid rgba(255, 122, 26, 0.2)',
            borderRadius: '32px',
            padding: '60px',
            boxShadow: '0 35px 70px rgba(0, 0, 0, 0.7), 0 0 0 1px rgba(255, 255, 255, 0.1), inset 0 2px 0 rgba(255, 255, 255, 0.15), 0 0 50px rgba(255, 122, 26, 0.1)',
            position: 'relative',
            overflow: 'hidden',
            transform: 'translateZ(30px)',
            transition: 'all 0.4s cubic-bezier(0.4, 0, 0.2, 1)'
          }}
        >
          <div 
            className="gaply-signup-header"
            style={{
              textAlign: 'center',
              marginBottom: '50px',
              transform: 'translateZ(20px)'
            }}
          >
            <h1 
              style={{
                color: '#ffffff',
                fontSize: '3.5rem',
                fontWeight: 900,
                marginBottom: '20px',
                fontFamily: "'Inter', sans-serif",
                background: 'linear-gradient(135deg, #ffffff 0%, #ff7a1a 30%, #ffffff 60%, #ff7a1a 100%)',
                backgroundSize: '300% 300%',
                WebkitBackgroundClip: 'text',
                WebkitTextFillColor: 'transparent',
                backgroundClip: 'text',
                textShadow: '0 0 40px rgba(255, 122, 26, 0.4)',
                letterSpacing: '-0.02em'
              }}
            >
              Join GAPLY
            </h1>
            <p 
              style={{
                color: '#cecece',
                fontSize: '1.2rem',
                fontWeight: 500,
                opacity: 0.9,
                textShadow: '0 2px 4px rgba(0, 0, 0, 0.5)'
              }}
            >
              Create your account to get started
            </p>
          </div>

          <form 
            onSubmit={handleSubmit} 
            className="gaply-signup-form"
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: '30px',
              transform: 'translateZ(10px)'
            }}
          >
            <div 
              className="gaply-form-row"
              style={{
                display: 'grid',
                gridTemplateColumns: '1fr 1fr',
                gap: '25px'
              }}
            >
              <div 
                className="gaply-form-group"
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '12px',
                  position: 'relative'
                }}
              >
                <label 
                  htmlFor="firstName"
                  style={{
                    color: '#ffffff',
                    fontSize: '1rem',
                    fontWeight: 700,
                    textTransform: 'uppercase',
                    letterSpacing: '1px',
                    textShadow: '0 2px 4px rgba(0, 0, 0, 0.5)'
                  }}
                >
                  First Name
                </label>
                <input
                  type="text"
                  id="firstName"
                  name="firstName"
                  value={formData.firstName}
                  onChange={handleInputChange}
                  required
                  placeholder="Enter your first name"
                  className={error ? 'error' : ''}
                  style={{
                    padding: '22px 24px',
                    border: '2px solid rgba(255, 255, 255, 0.15)',
                    borderRadius: '16px',
                    background: 'rgba(255, 255, 255, 0.05)',
                    color: '#ffffff',
                    fontSize: '1.1rem',
                    transition: 'all 0.4s cubic-bezier(0.4, 0, 0.2, 1)',
                    backdropFilter: 'blur(15px)',
                    position: 'relative',
                    zIndex: 1,
                    boxShadow: 'inset 0 2px 4px rgba(0, 0, 0, 0.2)'
                  }}
                />
              </div>

              <div 
                className="gaply-form-group"
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '12px',
                  position: 'relative'
                }}
              >
                <label 
                  htmlFor="lastName"
                  style={{
                    color: '#ffffff',
                    fontSize: '1rem',
                    fontWeight: 700,
                    textTransform: 'uppercase',
                    letterSpacing: '1px',
                    textShadow: '0 2px 4px rgba(0, 0, 0, 0.5)'
                  }}
                >
                  Last Name
                </label>
                <input
                  type="text"
                  id="lastName"
                  name="lastName"
                  value={formData.lastName}
                  onChange={handleInputChange}
                  required
                  placeholder="Enter your last name"
                  className={error ? 'error' : ''}
                  style={{
                    padding: '22px 24px',
                    border: '2px solid rgba(255, 255, 255, 0.15)',
                    borderRadius: '16px',
                    background: 'rgba(255, 255, 255, 0.05)',
                    color: '#ffffff',
                    fontSize: '1.1rem',
                    transition: 'all 0.4s cubic-bezier(0.4, 0, 0.2, 1)',
                    backdropFilter: 'blur(15px)',
                    position: 'relative',
                    zIndex: 1,
                    boxShadow: 'inset 0 2px 4px rgba(0, 0, 0, 0.2)'
                  }}
                />
              </div>
            </div>

            <div 
              className="gaply-form-group"
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: '12px',
                position: 'relative'
              }}
            >
              <label 
                htmlFor="email"
                style={{
                  color: '#ffffff',
                  fontSize: '1rem',
                  fontWeight: 700,
                  textTransform: 'uppercase',
                  letterSpacing: '1px',
                  textShadow: '0 2px 4px rgba(0, 0, 0, 0.5)'
                }}
              >
                Email Address
              </label>
              <input
                type="email"
                id="email"
                name="email"
                value={formData.email}
                onChange={handleInputChange}
                required
                placeholder="Enter your email"
                className={error ? 'error' : ''}
                style={{
                  padding: '22px 24px',
                  border: '2px solid rgba(255, 255, 255, 0.15)',
                  borderRadius: '16px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  color: '#ffffff',
                  fontSize: '1.1rem',
                  transition: 'all 0.4s cubic-bezier(0.4, 0, 0.2, 1)',
                  backdropFilter: 'blur(15px)',
                  position: 'relative',
                  zIndex: 1,
                  boxShadow: 'inset 0 2px 4px rgba(0, 0, 0, 0.2)'
                }}
              />
            </div>

            <div 
              className="gaply-form-group"
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: '12px',
                position: 'relative'
              }}
            >
              <label 
                htmlFor="password"
                style={{
                  color: '#ffffff',
                  fontSize: '1rem',
                  fontWeight: 700,
                  textTransform: 'uppercase',
                  letterSpacing: '1px',
                  textShadow: '0 2px 4px rgba(0, 0, 0, 0.5)'
                }}
              >
                Password
              </label>
              <input
                type="password"
                id="password"
                name="password"
                value={formData.password}
                onChange={handleInputChange}
                required
                placeholder="Create a password (min 8 characters)"
                className={error ? 'error' : ''}
                style={{
                  padding: '22px 24px',
                  border: '2px solid rgba(255, 255, 255, 0.15)',
                  borderRadius: '16px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  color: '#ffffff',
                  fontSize: '1.1rem',
                  transition: 'all 0.4s cubic-bezier(0.4, 0, 0.2, 1)',
                  backdropFilter: 'blur(15px)',
                  position: 'relative',
                  zIndex: 1,
                  boxShadow: 'inset 0 2px 4px rgba(0, 0, 0, 0.2)'
                }}
              />
            </div>

            <div 
              className="gaply-form-group"
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: '12px',
                position: 'relative'
              }}
            >
              <label 
                htmlFor="confirmPassword"
                style={{
                  color: '#ffffff',
                  fontSize: '1rem',
                  fontWeight: 700,
                  textTransform: 'uppercase',
                  letterSpacing: '1px',
                  textShadow: '0 2px 4px rgba(0, 0, 0, 0.5)'
                }}
              >
                Confirm Password
              </label>
              <input
                type="password"
                id="confirmPassword"
                name="confirmPassword"
                value={formData.confirmPassword}
                onChange={handleInputChange}
                required
                placeholder="Confirm your password"
                className={error ? 'error' : ''}
                style={{
                  padding: '22px 24px',
                  border: '2px solid rgba(255, 255, 255, 0.15)',
                  borderRadius: '16px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  color: '#ffffff',
                  fontSize: '1.1rem',
                  transition: 'all 0.4s cubic-bezier(0.4, 0, 0.2, 1)',
                  backdropFilter: 'blur(15px)',
                  position: 'relative',
                  zIndex: 1,
                  boxShadow: 'inset 0 2px 4px rgba(0, 0, 0, 0.2)'
                }}
              />
            </div>

            {error && (
              <div 
                className="error-message"
                style={{
                  color: '#ff4444',
                  fontSize: '1rem',
                  textAlign: 'center',
                  padding: '18px',
                  background: 'rgba(255, 68, 68, 0.15)',
                  borderRadius: '16px',
                  border: '2px solid rgba(255, 68, 68, 0.3)',
                  backdropFilter: 'blur(15px)',
                  boxShadow: '0 8px 25px rgba(255, 68, 68, 0.2)'
                }}
              >
                {error}
              </div>
            )}

            <button 
              type="submit" 
              className="gaply-signup-btn"
              disabled={loading}
              style={{
                padding: '24px',
                background: 'linear-gradient(135deg, #ff7a1a 0%, #ff8c42 25%, #ff7a1a 50%, #ff8c42 75%, #ff7a1a 100%)',
                backgroundSize: '300% 300%',
                color: '#ffffff',
                border: 'none',
                borderRadius: '16px',
                fontSize: '1.2rem',
                fontWeight: 800,
                cursor: 'pointer',
                transition: 'all 0.4s cubic-bezier(0.4, 0, 0.2, 1)',
                marginTop: '20px',
                textTransform: 'uppercase',
                letterSpacing: '2px',
                position: 'relative',
                overflow: 'hidden',
                boxShadow: '0 15px 40px rgba(255, 122, 26, 0.4), inset 0 2px 0 rgba(255, 255, 255, 0.3), 0 0 0 2px rgba(255, 122, 26, 0.2)'
              }}
            >
              {loading ? 'Creating Account...' : 'Create Account'}
            </button>
          </form>

          <div 
            className="gaply-signup-footer"
            style={{
              textAlign: 'center',
              marginTop: '40px',
              transform: 'translateZ(10px)'
            }}
          >
            <p 
              style={{
                color: '#cecece',
                fontSize: '1.1rem',
                textShadow: '0 2px 4px rgba(0, 0, 0, 0.5)'
              }}
            >
              Already have an account?{' '}
              <button 
                type="button" 
                className="gaply-switch-btn"
                onClick={onSwitchToLogin}
                style={{
                  background: 'none',
                  border: 'none',
                  color: '#ff7a1a',
                  fontWeight: 800,
                  cursor: 'pointer',
                  textDecoration: 'underline',
                  transition: 'all 0.3s ease',
                  fontSize: '1.1rem',
                  textShadow: '0 0 15px rgba(255, 122, 26, 0.5)'
                }}
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
