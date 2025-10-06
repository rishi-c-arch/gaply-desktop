import React, { useState, useEffect, useRef } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Environment } from '@react-three/drei';
import * as THREE from 'three';
import './PremiumLanding.css';

// 3D Background Elements for Premium Page
const PremiumBackground: React.FC = () => {
  const groupRef = useRef<THREE.Group>(null);

  useFrame((state) => {
    if (groupRef.current) {
      groupRef.current.rotation.y = state.clock.elapsedTime * 0.05;
    }
  });

  return (
    <group ref={groupRef}>
      {/* Floating Premium Elements */}
      {Array.from({ length: 8 }).map((_, i) => (
        <mesh 
          key={i} 
          position={[
            Math.sin(i * 0.8) * 12,
            Math.cos(i * 0.8) * 12,
            Math.sin(i * 0.4) * 4
          ]}
        >
          <octahedronGeometry args={[0.4, 0]} />
          <meshStandardMaterial 
            color="#ff7a1a" 
            transparent
            opacity={0.3}
            metalness={0.8}
            roughness={0.2}
          />
        </mesh>
      ))}
      
      {/* Central Premium Orb */}
      <mesh position={[0, 0, -3]}>
        <sphereGeometry args={[1.2, 32, 32]} />
        <meshStandardMaterial 
          color="#ff7a1a"
          metalness={0.9}
          roughness={0.1}
          transparent
          opacity={0.2}
        />
      </mesh>
    </group>
  );
};

interface PremiumFeature {
  id: string;
  title: string;
  description: string;
  icon: string;
  features: string[];
  price: string;
  uses: number;
  popular?: boolean;
}

const PremiumLanding: React.FC = () => {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [userPlan, setUserPlan] = useState<any>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    checkAuthentication();
  }, []);

  const checkAuthentication = async () => {
    const token = localStorage.getItem('gaply_token');
    if (token) {
      try {
        const response = await fetch('https://backend.gaply.in/api/v1/user/profile', {
          headers: {
            'Authorization': `Bearer ${token}`,
            'Content-Type': 'application/json'
          }
        });
        
        if (response.ok) {
          const userData = await response.json();
          setIsAuthenticated(true);
          setUserPlan(userData.plan);
        }
      } catch (error) {
        console.error('Auth check failed:', error);
      }
    }
    setLoading(false);
  };

  const premiumFeatures: PremiumFeature[] = [
    {
      id: 'basic',
      title: 'Gaply Basic',
      description: 'Research Gap Finder - Analyze 5 base papers to identify research gaps and opportunities',
      icon: '🔍',
      features: [
        'Research Gap Finder (1 use)',
        'Analyze 5 base papers',
        'Publishable problem statement',
        'Clear objectives & hypotheses',
        'Valid for 365 days'
      ],
      price: '₹471.00',
      uses: 1
    },
    {
      id: 'plus',
      title: 'Gaply Plus',
      description: 'Research Pro - Gap Finder (2 uses) + Deep Paper Analysis (1 use) with 70+ parameter evaluation',
      icon: '🚀',
      features: [
        'Research Gap Finder (2 uses)',
        'Deep Paper Analysis (1 use)',
        '70+ parameter evaluation',
        'Journal compliance checking',
        'Valid for 365 days'
      ],
      price: '₹1061.00',
      uses: 2,
      popular: true
    },
    {
      id: 'pro',
      title: 'Gaply Pro',
      description: 'Research Elite - Full access (5 uses each) + Team support with 30min session + WhatsApp support',
      icon: '⭐',
      features: [
        'Research Gap Finder (5 uses)',
        'Deep Paper Analysis (5 uses)',
        'Team support (30min session)',
        'WhatsApp support',
        'Valid for 365 days'
      ],
      price: '₹2241.00',
      uses: 5
    }
  ];

  const handleGetPremium = (planId: string) => {
    if (!isAuthenticated) {
      // Redirect to login
      window.location.href = '/login';
      return;
    }
    
    // Open package selection modal
    window.openPackageSelection?.(planId);
  };

  if (loading) {
    return (
      <div className="premium-loading">
        <div className="loading-spinner"></div>
        <p>Loading premium features...</p>
      </div>
    );
  }

  return (
    <div className="premium-landing">
      {/* 3D Background */}
      <div className="premium-background-3d">
        <Canvas camera={{ position: [0, 0, 8], fov: 75 }}>
          <ambientLight intensity={0.3} />
          <directionalLight position={[10, 10, 5]} intensity={1} />
          <pointLight position={[-10, -10, -5]} intensity={0.5} color="#ff7a1a" />
          <Environment preset="night" />
          <PremiumBackground />
        </Canvas>
      </div>

      {/* Header */}
      <div className="premium-header">
        <div className="premium-nav">
          <a href="/" className="premium-logo">Gaply</a>
          <div className="premium-nav-links">
            <a href="/#features">Features</a>
            <a href="/#pricing">Pricing</a>
            <a href="/#contact">Contact</a>
            {isAuthenticated ? (
              <a href="/dashboard" className="premium-btn">Dashboard</a>
            ) : (
              <a href="/login" className="premium-btn">Login</a>
            )}
          </div>
        </div>
      </div>

      {/* Hero Section */}
      <div className="premium-hero">
        <div className="premium-hero-content">
          <h1 className="premium-hero-title">
            Unlock Your Research Potential
          </h1>
          <p className="premium-hero-subtitle">
            Advanced AI-powered tools to accelerate your academic journey and publish breakthrough research
          </p>
          
          {userPlan ? (
            <div className="current-plan-badge">
              <span>Current Plan: {userPlan.name}</span>
              <a href="/dashboard" className="manage-plan-btn">Manage Plan</a>
            </div>
          ) : (
            <div className="premium-cta">
              <button 
                className="premium-cta-btn"
                onClick={() => handleGetPremium('plus')}
              >
                Get Premium Access
              </button>
              <p className="premium-cta-note">Start with our most popular plan</p>
            </div>
          )}
        </div>
      </div>

      {/* Features Showcase */}
      <div className="premium-features-showcase">
        <div className="container">
          <h2 className="showcase-title">Premium Research Tools</h2>
          <p className="showcase-subtitle">Everything you need to excel in academic research</p>
          
          <div className="features-grid">
            <div className="feature-card">
              <div className="feature-icon">🔍</div>
              <h3>Research Gap Finder</h3>
              <p>Analyze multiple research papers to identify unexplored areas and research opportunities</p>
              <ul>
                <li>Upload up to 5 research papers</li>
                <li>AI-powered gap identification</li>
                <li>Professional report generation</li>
                <li>Publishable problem statements</li>
              </ul>
            </div>
            
            <div className="feature-card">
              <div className="feature-icon">📊</div>
              <h3>Deep Paper Analysis</h3>
              <p>Comprehensive evaluation with 70+ parameters for journal compliance and quality assessment</p>
              <ul>
                <li>70+ parameter evaluation</li>
                <li>Journal compliance checking</li>
                <li>Evidence-based analysis</li>
                <li>Professional HTML reports</li>
              </ul>
            </div>
            
            <div className="feature-card">
              <div className="feature-icon">👥</div>
              <h3>Team Support</h3>
              <p>Expert guidance and personalized assistance for your research journey</p>
              <ul>
                <li>30-minute expert sessions</li>
                <li>WhatsApp support</li>
                <li>Research methodology guidance</li>
                <li>Publication strategy advice</li>
              </ul>
            </div>
          </div>
        </div>
      </div>

      {/* Pricing Section */}
      <div className="premium-pricing">
        <div className="container">
          <h2 className="pricing-title">Choose Your Research Plan</h2>
          <p className="pricing-subtitle">Flexible plans designed for researchers at every level</p>
          
          <div className="pricing-grid">
            {premiumFeatures.map((plan) => (
              <div 
                key={plan.id} 
                className={`pricing-card ${plan.popular ? 'popular' : ''}`}
              >
                {plan.popular && (
                  <div className="popular-badge">Most Popular</div>
                )}
                
                <div className="plan-header">
                  <div className="plan-icon">{plan.icon}</div>
                  <h3 className="plan-name">{plan.title}</h3>
                  <div className="plan-price">{plan.price}</div>
                </div>
                
                <p className="plan-description">{plan.description}</p>
                
                <ul className="plan-features">
                  {plan.features.map((feature, index) => (
                    <li key={index} className="feature-item">
                      <span className="checkmark">✓</span>
                      {feature}
                    </li>
                  ))}
                </ul>
                
                <button 
                  className="choose-plan-btn"
                  onClick={() => handleGetPremium(plan.id)}
                >
                  {userPlan ? 'Upgrade Plan' : 'Choose Plan'}
                </button>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Testimonials */}
      <div className="premium-testimonials">
        <div className="container">
          <h2 className="testimonials-title">What Researchers Say</h2>
          <div className="testimonials-grid">
            <div className="testimonial-card">
              <p>"Gaply's Research Gap Finder helped me identify a completely new research direction that led to my first publication."</p>
              <div className="testimonial-author">
                <strong>Dr. Sarah Chen</strong>
                <span>PhD in Computer Science, MIT</span>
              </div>
            </div>
            
            <div className="testimonial-card">
              <p>"The Deep Paper Analysis feature saved me months of work. The 70+ parameter evaluation is incredibly thorough."</p>
              <div className="testimonial-author">
                <strong>Prof. Michael Rodriguez</strong>
                <span>Professor of Engineering, Stanford</span>
              </div>
            </div>
            
            <div className="testimonial-card">
              <p>"The team support is exceptional. They helped me refine my research methodology and improve my paper's impact."</p>
              <div className="testimonial-author">
                <strong>Dr. Priya Sharma</strong>
                <span>Postdoc Researcher, Oxford</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Footer CTA */}
      <div className="premium-footer-cta">
        <div className="container">
          <h2>Ready to Accelerate Your Research?</h2>
          <p>Join thousands of researchers who have transformed their academic journey with Gaply</p>
          <button 
            className="premium-footer-btn"
            onClick={() => handleGetPremium('plus')}
          >
            Start Your Premium Journey
          </button>
        </div>
      </div>
    </div>
  );
};

export default PremiumLanding;
