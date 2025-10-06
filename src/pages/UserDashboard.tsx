import React, { useState, useEffect, useRef } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Environment } from '@react-three/drei';
import * as THREE from 'three';
import './UserDashboard.css';

// 3D Background for Dashboard
const DashboardBackground: React.FC = () => {
  const groupRef = useRef<THREE.Group>(null);

  useFrame((state) => {
    if (groupRef.current) {
      groupRef.current.rotation.y = state.clock.elapsedTime * 0.02;
    }
  });

  return (
    <group ref={groupRef}>
      {/* Floating Dashboard Elements */}
      {Array.from({ length: 4 }).map((_, i) => (
        <mesh 
          key={i} 
          position={[
            Math.sin(i * 1.5) * 8,
            Math.cos(i * 1.5) * 8,
            Math.sin(i * 0.8) * 2
          ]}
        >
          <cylinderGeometry args={[0.2, 0.2, 0.8, 8]} />
          <meshStandardMaterial 
            color="#ff7a1a" 
            transparent
            opacity={0.15}
            metalness={0.8}
            roughness={0.2}
          />
        </mesh>
      ))}
    </group>
  );
};

interface UserAccount {
  id: string;
  name: string;
  email: string;
  plan: {
    id: string;
    name: string;
    price: number;
    validUntil: string;
  };
  usage: {
    gapFinderUses: number;
    deepAnalysisUses: number;
    teamSupportSessions: number;
  };
  limits: {
    gapFinderLimit: number;
    deepAnalysisLimit: number;
    teamSupportLimit: number;
  };
}

const UserDashboard: React.FC = () => {
  const [userAccount, setUserAccount] = useState<UserAccount | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<'overview' | 'usage' | 'billing' | 'support'>('overview');

  useEffect(() => {
    fetchUserAccount();
  }, []);

  const fetchUserAccount = async () => {
    const token = localStorage.getItem('gaply_token');
    if (!token) {
      window.location.href = '/login';
      return;
    }

    try {
      const response = await fetch('https://backend.gaply.in/api/v1/user/account', {
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json'
        }
      });

      if (response.ok) {
        const accountData = await response.json();
        setUserAccount(accountData);
      } else {
        console.error('Failed to fetch user account');
      }
    } catch (error) {
      console.error('Error fetching user account:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleUpgrade = () => {
    window.location.href = '/premium';
  };

  const handleLogout = () => {
    localStorage.removeItem('gaply_token');
    window.location.href = '/';
  };

  const formatDate = (dateString: string) => {
    return new Date(dateString).toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'long',
      day: 'numeric'
    });
  };

  const getUsagePercentage = (used: number, limit: number) => {
    return Math.min((used / limit) * 100, 100);
  };

  if (loading) {
    return (
      <div className="dashboard-loading">
        <div className="loading-spinner"></div>
        <p>Loading dashboard...</p>
      </div>
    );
  }

  if (!userAccount) {
    return (
      <div className="dashboard-error">
        <h2>Unable to load account</h2>
        <p>Please try logging in again</p>
        <a href="/login" className="login-btn">Login</a>
      </div>
    );
  }

  return (
    <div className="user-dashboard">
      {/* 3D Background */}
      <div className="dashboard-background-3d">
        <Canvas camera={{ position: [0, 0, 8], fov: 75 }}>
          <ambientLight intensity={0.3} />
          <directionalLight position={[10, 10, 5]} intensity={1} />
          <pointLight position={[-10, -10, -5]} intensity={0.5} color="#ff7a1a" />
          <Environment preset="night" />
          <DashboardBackground />
        </Canvas>
      </div>

      {/* Header */}
      <div className="dashboard-header">
        <div className="dashboard-nav">
          <a href="/" className="dashboard-logo">Gaply</a>
          <div className="dashboard-nav-links">
            <a href="/premium">Premium</a>
            <button onClick={handleLogout} className="logout-btn">Logout</button>
          </div>
        </div>
      </div>

      {/* Main Content */}
      <div className="dashboard-main">
        <div className="container">
          {/* Welcome Section */}
          <div className="welcome-section">
            <h1>Welcome back, {userAccount.name}!</h1>
            <p>Manage your research tools and track your progress</p>
          </div>

          {/* Current Plan Card */}
          <div className="plan-card">
            <div className="plan-header">
              <h3>Current Plan</h3>
              <button className="upgrade-btn" onClick={handleUpgrade}>
                Upgrade Plan
              </button>
            </div>
            <div className="plan-info">
              <div className="plan-name">{userAccount.plan.name}</div>
              <div className="plan-price">₹{userAccount.plan.price}</div>
              <div className="plan-validity">Valid until: {formatDate(userAccount.plan.validUntil)}</div>
            </div>
          </div>

          {/* Tabs */}
          <div className="dashboard-tabs">
            <button 
              className={`tab-btn ${activeTab === 'overview' ? 'active' : ''}`}
              onClick={() => setActiveTab('overview')}
            >
              Overview
            </button>
            <button 
              className={`tab-btn ${activeTab === 'usage' ? 'active' : ''}`}
              onClick={() => setActiveTab('usage')}
            >
              Usage
            </button>
            <button 
              className={`tab-btn ${activeTab === 'billing' ? 'active' : ''}`}
              onClick={() => setActiveTab('billing')}
            >
              Billing
            </button>
            <button 
              className={`tab-btn ${activeTab === 'support' ? 'active' : ''}`}
              onClick={() => setActiveTab('support')}
            >
              Support
            </button>
          </div>

          {/* Tab Content */}
          <div className="tab-content">
            {activeTab === 'overview' && (
              <div className="overview-tab">
                <div className="stats-grid">
                  <div className="stat-card">
                    <div className="stat-icon">🔍</div>
                    <div className="stat-info">
                      <div className="stat-label">Research Gap Finder</div>
                      <div className="stat-value">
                        {userAccount.usage.gapFinderUses}/{userAccount.limits.gapFinderLimit} uses
                      </div>
                      <div className="stat-bar">
                        <div 
                          className="stat-fill"
                          style={{ width: `${getUsagePercentage(userAccount.usage.gapFinderUses, userAccount.limits.gapFinderLimit)}%` }}
                        ></div>
                      </div>
                    </div>
                  </div>

                  <div className="stat-card">
                    <div className="stat-icon">📊</div>
                    <div className="stat-info">
                      <div className="stat-label">Deep Paper Analysis</div>
                      <div className="stat-value">
                        {userAccount.usage.deepAnalysisUses}/{userAccount.limits.deepAnalysisLimit} uses
                      </div>
                      <div className="stat-bar">
                        <div 
                          className="stat-fill"
                          style={{ width: `${getUsagePercentage(userAccount.usage.deepAnalysisUses, userAccount.limits.deepAnalysisLimit)}%` }}
                        ></div>
                      </div>
                    </div>
                  </div>

                  <div className="stat-card">
                    <div className="stat-icon">👥</div>
                    <div className="stat-info">
                      <div className="stat-label">Team Support</div>
                      <div className="stat-value">
                        {userAccount.usage.teamSupportSessions}/{userAccount.limits.teamSupportLimit} sessions
                      </div>
                      <div className="stat-bar">
                        <div 
                          className="stat-fill"
                          style={{ width: `${getUsagePercentage(userAccount.usage.teamSupportSessions, userAccount.limits.teamSupportLimit)}%` }}
                        ></div>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="quick-actions">
                  <h3>Quick Actions</h3>
                  <div className="actions-grid">
                    <a href="/premium/gap-finder" className="action-card">
                      <div className="action-icon">🔍</div>
                      <div className="action-title">Research Gap Finder</div>
                      <div className="action-desc">Analyze papers for research gaps</div>
                    </a>
                    <a href="/premium/deep-analysis" className="action-card">
                      <div className="action-icon">📊</div>
                      <div className="action-title">Deep Paper Analysis</div>
                      <div className="action-desc">Comprehensive paper evaluation</div>
                    </a>
                    <a href="/premium/support" className="action-card">
                      <div className="action-icon">👥</div>
                      <div className="action-title">Team Support</div>
                      <div className="action-desc">Get expert guidance</div>
                    </a>
                  </div>
                </div>
              </div>
            )}

            {activeTab === 'usage' && (
              <div className="usage-tab">
                <h3>Usage Statistics</h3>
                <div className="usage-details">
                  <div className="usage-item">
                    <h4>Research Gap Finder</h4>
                    <div className="usage-bar">
                      <div 
                        className="usage-fill"
                        style={{ width: `${getUsagePercentage(userAccount.usage.gapFinderUses, userAccount.limits.gapFinderLimit)}%` }}
                      ></div>
                    </div>
                    <div className="usage-text">
                      {userAccount.usage.gapFinderUses} of {userAccount.limits.gapFinderLimit} uses
                    </div>
                  </div>

                  <div className="usage-item">
                    <h4>Deep Paper Analysis</h4>
                    <div className="usage-bar">
                      <div 
                        className="usage-fill"
                        style={{ width: `${getUsagePercentage(userAccount.usage.deepAnalysisUses, userAccount.limits.deepAnalysisLimit)}%` }}
                      ></div>
                    </div>
                    <div className="usage-text">
                      {userAccount.usage.deepAnalysisUses} of {userAccount.limits.deepAnalysisLimit} uses
                    </div>
                  </div>

                  <div className="usage-item">
                    <h4>Team Support Sessions</h4>
                    <div className="usage-bar">
                      <div 
                        className="usage-fill"
                        style={{ width: `${getUsagePercentage(userAccount.usage.teamSupportSessions, userAccount.limits.teamSupportLimit)}%` }}
                      ></div>
                    </div>
                    <div className="usage-text">
                      {userAccount.usage.teamSupportSessions} of {userAccount.limits.teamSupportLimit} sessions
                    </div>
                  </div>
                </div>
              </div>
            )}

            {activeTab === 'billing' && (
              <div className="billing-tab">
                <h3>Billing Information</h3>
                <div className="billing-details">
                  <div className="billing-item">
                    <span className="billing-label">Current Plan:</span>
                    <span className="billing-value">{userAccount.plan.name}</span>
                  </div>
                  <div className="billing-item">
                    <span className="billing-label">Plan Price:</span>
                    <span className="billing-value">₹{userAccount.plan.price}</span>
                  </div>
                  <div className="billing-item">
                    <span className="billing-label">Valid Until:</span>
                    <span className="billing-value">{formatDate(userAccount.plan.validUntil)}</span>
                  </div>
                  <div className="billing-item">
                    <span className="billing-label">Auto Renewal:</span>
                    <span className="billing-value">Disabled</span>
                  </div>
                </div>
                <button className="upgrade-btn" onClick={handleUpgrade}>
                  Upgrade Plan
                </button>
              </div>
            )}

            {activeTab === 'support' && (
              <div className="support-tab">
                <h3>Get Support</h3>
                <div className="support-options">
                  <div className="support-card">
                    <div className="support-icon">💬</div>
                    <h4>WhatsApp Support</h4>
                    <p>Get instant help via WhatsApp</p>
                    <a href="https://wa.me/919876543210" className="support-btn">
                      Contact WhatsApp
                    </a>
                  </div>
                  
                  <div className="support-card">
                    <div className="support-icon">📧</div>
                    <h4>Email Support</h4>
                    <p>Send us your questions via email</p>
                    <a href="mailto:support@gaply.in" className="support-btn">
                      Send Email
                    </a>
                  </div>
                  
                  <div className="support-card">
                    <div className="support-icon">📞</div>
                    <h4>Team Support Session</h4>
                    <p>Schedule a 30-minute expert session</p>
                    <button 
                      className="support-btn"
                      disabled={userAccount.usage.teamSupportSessions >= userAccount.limits.teamSupportLimit}
                    >
                      {userAccount.usage.teamSupportSessions >= userAccount.limits.teamSupportLimit 
                        ? 'No Sessions Left' 
                        : 'Schedule Session'
                      }
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default UserDashboard;
