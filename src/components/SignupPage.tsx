import React, { useState } from 'react';
import './PremiumSignupBackground.css';
import './GaplySignupStyles.css';
import PremiumSignupBackground from './PremiumSignupBackground';


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
    <div className="gaply-signup-page">
      {/* Premium 3D Background */}
      <PremiumSignupBackground />

      {/* Signup Form */}
      <div className="gaply-signup-container">
        <div className="gaply-signup-card">
          <div className="gaply-signup-header">
            <h1>Join GAPLY</h1>
            <p>Create your account to get started</p>
          </div>

          <form onSubmit={handleSubmit} className="gaply-signup-form">
            <div className="gaply-form-row">
              <div className="gaply-form-group">
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

              <div className="gaply-form-group">
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

            <div className="gaply-form-group">
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

            <div className="gaply-form-group">
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

            <div className="gaply-form-group">
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
              className="gaply-signup-btn"
              disabled={loading}
            >
              {loading ? 'Creating Account...' : 'Create Account'}
            </button>
          </form>

          <div className="gaply-signup-footer">
            <p>
              Already have an account?{' '}
              <button 
                type="button" 
                className="gaply-switch-btn"
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
