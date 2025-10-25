import React, { useState, useEffect, useRef } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls, Text, Box, Sphere, Torus } from '@react-three/drei';
import { buildApiUrl } from '../api/config';
import * as THREE from 'three';

interface UserAccount {
  planId: string;
  planName: string;
  gapFinderUsesRemaining: number;
  deepEvalUsesRemaining: number;
  hasSupport: boolean;
  expiresAt: string;
  createdAt: string;
}

interface PaymentHistory {
  id: string;
  package_name: string;
  amount: number;
  status: string;
  created_at: string;
}

// 3D Background Particles
const BackgroundParticles: React.FC = () => {
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
        particle.rotation.y += 0.005;
        particle.rotation.x += 0.002;
        particle.position.y += Math.sin(state.clock.elapsedTime + i * 0.1) * 0.002;
        particle.updateMatrix();
        meshRef.current!.setMatrixAt(i, particle.matrix);
      });
      meshRef.current.instanceMatrix.needsUpdate = true;
    }
  });

  return (
    <instancedMesh ref={meshRef} args={[undefined, undefined, 100]}>
      <sphereGeometry args={[0.05, 6, 6]} />
      <meshBasicMaterial color="#007AFF" transparent opacity={0.2} />
    </instancedMesh>
  );
};

// 3D Data Visualization Component
const DataVisualization: React.FC<{
  gapFinderUses: number;
  deepEvalUses: number;
}> = ({ gapFinderUses, deepEvalUses }) => {
  const gapFinderRef = useRef<THREE.Mesh>(null);
  const deepEvalRef = useRef<THREE.Mesh>(null);

  useFrame((state) => {
    if (gapFinderRef.current) {
      gapFinderRef.current.rotation.y = Math.sin(state.clock.elapsedTime) * 0.2;
      gapFinderRef.current.scale.setScalar(1 + Math.sin(state.clock.elapsedTime * 2) * 0.1);
    }
    if (deepEvalRef.current) {
      deepEvalRef.current.rotation.y = Math.sin(state.clock.elapsedTime + Math.PI) * 0.2;
      deepEvalRef.current.scale.setScalar(1 + Math.sin(state.clock.elapsedTime * 2 + Math.PI) * 0.1);
    }
  });

  return (
    <group>
      {/* Gap Finder Visualization */}
      <mesh ref={gapFinderRef} position={[-2, 0, 0]}>
        <torusGeometry args={[1, 0.3, 8, 16]} />
        <meshStandardMaterial 
          color="#007AFF" 
          metalness={0.8}
          roughness={0.2}
          emissive="#007AFF"
          emissiveIntensity={0.1}
        />
      </mesh>
      
      {/* Deep Evaluation Visualization */}
      <mesh ref={deepEvalRef} position={[2, 0, 0]}>
        <sphereGeometry args={[1, 16, 16]} />
        <meshStandardMaterial 
          color="#30D158" 
          metalness={0.8}
          roughness={0.2}
          emissive="#30D158"
          emissiveIntensity={0.1}
        />
      </mesh>

      {/* Usage Text */}
      <Text
        position={[-2, -1.5, 0]}
        fontSize={0.2}
        color="#ffffff"
        anchorX="center"
        anchorY="middle"
      >
        Gap Finder: {gapFinderUses}
      </Text>
      
      <Text
        position={[2, -1.5, 0]}
        fontSize={0.2}
        color="#ffffff"
        anchorX="center"
        anchorY="middle"
      >
        Deep Analysis: {deepEvalUses}
      </Text>
    </group>
  );
};

// 3D Tab Indicator
const TabIndicator3D: React.FC<{
  position: [number, number, number];
  active: boolean;
  label: string;
}> = ({ position, active, label }) => {
  const meshRef = useRef<THREE.Mesh>(null);

  useFrame((state) => {
    if (meshRef.current) {
      meshRef.current.position.y = position[1] + Math.sin(state.clock.elapsedTime + position[0]) * 0.05;
    }
  });

  return (
    <group position={position}>
      <mesh ref={meshRef}>
        <boxGeometry args={[2, 0.5, 0.1]} />
        <meshStandardMaterial 
          color={active ? "#007AFF" : "#333333"} 
          metalness={0.8}
          roughness={0.2}
          emissive={active ? "#007AFF" : "#000000"}
          emissiveIntensity={active ? 0.2 : 0}
        />
      </mesh>
      
      <Text
        position={[0, 0, 0.1]}
        fontSize={0.15}
        color="#ffffff"
        anchorX="center"
        anchorY="middle"
      >
        {label}
      </Text>
    </group>
  );
};

const UserDashboard: React.FC = () => {
  const [userAccount, setUserAccount] = useState<UserAccount | null>(null);
  const [paymentHistory, setPaymentHistory] = useState<PaymentHistory[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<'overview' | 'usage' | 'payments'>('overview');

  const getToken = () => localStorage.getItem('authToken') || localStorage.getItem('token') || localStorage.getItem('gaply_token');

  useEffect(() => {
    loadUserData();
  }, []);

  const loadUserData = async () => {
    const token = getToken();
    const userEmail = localStorage.getItem('user_email');
    
    if (userEmail === 'testadmin@gaply.com') {
      setUserAccount({
        planId: 'TEST-ADMIN',
        planName: 'Test Admin Plan',
        gapFinderUsesRemaining: 999,
        deepEvalUsesRemaining: 999,
        hasSupport: true,
        expiresAt: 'Never expires',
        createdAt: new Date().toISOString()
      });
      setPaymentHistory([]);
      setLoading(false);
      return;
    }

    if (!token) {
      setLoading(false);
      return;
    }

    try {
      const accountResponse = await fetch(`${buildApiUrl('/api/premium/subscription')}`, {
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json',
        },
      });

      if (accountResponse.ok) {
        const accountData = await accountResponse.json();
        if (accountData.success) {
          setUserAccount(accountData.data);
        }
      }

      const paymentResponse = await fetch(`${buildApiUrl('/api/premium/payment-history')}`, {
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json',
        },
      });

      if (paymentResponse.ok) {
        const paymentData = await paymentResponse.json();
        if (paymentData.success) {
          setPaymentHistory(paymentData.data);
        }
      }
    } catch (error) {
      console.error('Failed to load user data:', error);
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: '#000000',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif'
      }}>
        <div style={{
          position: 'absolute',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0
        }}>
          <Canvas camera={{ position: [0, 0, 5], fov: 60 }}>
            <ambientLight intensity={0.2} />
            <pointLight position={[5, 5, 5]} intensity={0.5} />
            <BackgroundParticles />
            <OrbitControls enableZoom={false} enablePan={false} />
          </Canvas>
        </div>
        
        <div style={{
          textAlign: 'center',
          zIndex: 2,
          position: 'relative'
        }}>
          <div style={{
            width: '30px',
            height: '30px',
            border: '2px solid rgba(0, 122, 255, 0.2)',
            borderTop: '2px solid #007AFF',
            borderRadius: '50%',
            animation: 'spin 1s linear infinite',
            margin: '0 auto 16px'
          }}></div>
          <p style={{
            fontSize: '14px',
            color: '#888888',
            margin: 0
          }}>
            Loading your account...
          </p>
        </div>
        
        <style dangerouslySetInnerHTML={{
          __html: `
            @keyframes spin {
              0% { transform: rotate(0deg); }
              100% { transform: rotate(360deg); }
            }
          `
        }} />
      </div>
    );
  }

  if (!userAccount) {
    return (
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: '#000000',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif'
      }}>
        <div style={{
          position: 'absolute',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0
        }}>
          <Canvas camera={{ position: [0, 0, 5], fov: 60 }}>
            <ambientLight intensity={0.2} />
            <pointLight position={[5, 5, 5]} intensity={0.5} />
            <BackgroundParticles />
            <OrbitControls enableZoom={false} enablePan={false} />
          </Canvas>
        </div>
        
        <div style={{
          textAlign: 'center',
          maxWidth: '400px',
          padding: '30px',
          zIndex: 2,
          position: 'relative',
          background: 'rgba(255, 255, 255, 0.05)',
          borderRadius: '16px',
          backdropFilter: 'blur(20px)',
          border: '1px solid rgba(255, 255, 255, 0.1)'
        }}>
          <h2 style={{
            fontSize: '24px',
            fontWeight: '600',
            color: '#ffffff',
            margin: '0 0 12px 0'
          }}>
            No subscription found
          </h2>
          <p style={{
            fontSize: '14px',
            color: '#888888',
            margin: '0 0 20px 0',
            lineHeight: '1.4'
          }}>
            You don't have an active subscription. Choose a plan to get started with GAPLY Premium.
          </p>
          <button
            onClick={() => window.location.href = '/premium'}
            style={{
              background: '#007AFF',
              color: 'white',
              border: 'none',
              borderRadius: '8px',
              padding: '12px 24px',
              fontSize: '14px',
              fontWeight: '600',
              cursor: 'pointer',
              transition: 'all 0.3s ease',
              transform: 'translateZ(0)',
              transformStyle: 'preserve-3d'
            }}
            onMouseOver={(e) => {
              e.currentTarget.style.background = '#0056CC';
              e.currentTarget.style.transform = 'translateZ(5px) scale(1.05)';
            }}
            onMouseOut={(e) => {
              e.currentTarget.style.background = '#007AFF';
              e.currentTarget.style.transform = 'translateZ(0) scale(1)';
            }}
          >
            View Plans
          </button>
        </div>
      </div>
    );
  }

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
          
          <BackgroundParticles />
          
          {/* 3D Tab Indicators */}
          <TabIndicator3D
            position={[-4, 2, 0]}
            active={activeTab === 'overview'}
            label="Overview"
          />
          <TabIndicator3D
            position={[0, 2, 0]}
            active={activeTab === 'usage'}
            label="Usage"
          />
          <TabIndicator3D
            position={[4, 2, 0]}
            active={activeTab === 'payments'}
            label="Payments"
          />
          
          {/* 3D Data Visualization */}
          {activeTab === 'usage' && (
            <DataVisualization
              gapFinderUses={userAccount.gapFinderUsesRemaining}
              deepEvalUses={userAccount.deepEvalUsesRemaining}
            />
          )}
          
          <OrbitControls enableZoom={false} enablePan={false} />
        </Canvas>
      </div>

      {/* UI Overlay */}
      <div style={{
        position: 'relative',
        zIndex: 2,
        height: '100vh',
        display: 'flex',
        flexDirection: 'column',
        padding: '16px'
      }}>
        {/* Header */}
        <div style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '24px',
          paddingBottom: '16px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.1)'
        }}>
          <div>
            <h1 style={{
              fontSize: '28px',
              fontWeight: '600',
              color: '#ffffff',
              margin: '0 0 4px 0',
              letterSpacing: '-0.02em',
              lineHeight: '1.1',
              textShadow: '0 0 20px rgba(0, 122, 255, 0.5)'
            }}>
              My Account
            </h1>
            <p style={{
              fontSize: '14px',
              fontWeight: '400',
              color: '#888888',
              margin: 0,
              lineHeight: '1.3'
            }}>
              Manage your subscription and usage
            </p>
          </div>
          
          <button
            onClick={() => window.location.href = '/premium'}
            style={{
              background: 'rgba(255, 255, 255, 0.1)',
              border: 'none',
              borderRadius: '8px',
              padding: '8px 16px',
              cursor: 'pointer',
              fontSize: '12px',
              fontWeight: '500',
              color: '#ffffff',
              transition: 'all 0.3s ease',
              backdropFilter: 'blur(10px)',
              transform: 'translateZ(0)',
              transformStyle: 'preserve-3d'
            }}
            onMouseOver={(e) => {
              e.currentTarget.style.background = 'rgba(255, 255, 255, 0.2)';
              e.currentTarget.style.transform = 'translateZ(10px) scale(1.05)';
            }}
            onMouseOut={(e) => {
              e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
              e.currentTarget.style.transform = 'translateZ(0) scale(1)';
            }}
          >
            ← Back to Plans
          </button>
        </div>

        {/* Tab Navigation */}
        <div style={{
          display: 'flex',
          justifyContent: 'center',
          marginBottom: '24px'
        }}>
          <div style={{
            background: 'rgba(255, 255, 255, 0.1)',
            borderRadius: '12px',
            padding: '4px',
            display: 'flex',
            gap: '4px',
            backdropFilter: 'blur(10px)',
            transform: 'translateZ(0)',
            transformStyle: 'preserve-3d'
          }}>
            {[
              { id: 'overview', label: 'Overview' },
              { id: 'usage', label: 'Usage' },
              { id: 'payments', label: 'Payments' }
            ].map((tab) => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id as any)}
                style={{
                  background: activeTab === tab.id ? '#007AFF' : 'transparent',
                  color: activeTab === tab.id ? 'white' : '#ffffff',
                  border: 'none',
                  borderRadius: '8px',
                  padding: '8px 16px',
                  fontSize: '12px',
                  fontWeight: '500',
                  cursor: 'pointer',
                  transition: 'all 0.3s ease',
                  transform: 'translateZ(0)',
                  transformStyle: 'preserve-3d'
                }}
                onMouseOver={(e) => {
                  if (activeTab !== tab.id) {
                    e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
                    e.currentTarget.style.transform = 'translateZ(5px) scale(1.05)';
                  }
                }}
                onMouseOut={(e) => {
                  if (activeTab !== tab.id) {
                    e.currentTarget.style.background = 'transparent';
                    e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                  }
                }}
              >
                {tab.label}
              </button>
            ))}
          </div>
        </div>

        {/* Overview Tab */}
        {activeTab === 'overview' && (
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))',
            gap: '20px',
            maxWidth: '800px',
            margin: '0 auto'
          }}>
            {/* Current Plan Card */}
            <div style={{
              background: 'rgba(255, 255, 255, 0.05)',
              borderRadius: '16px',
              padding: '24px',
              backdropFilter: 'blur(20px)',
              border: '1px solid rgba(255, 255, 255, 0.1)',
              transform: 'translateZ(0)',
              transformStyle: 'preserve-3d',
              boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)'
            }}>
              <h3 style={{
                fontSize: '18px',
                fontWeight: '600',
                color: '#ffffff',
                margin: '0 0 16px 0'
              }}>
                Current Plan
              </h3>
              
              <div style={{
                background: 'rgba(0, 122, 255, 0.1)',
                borderRadius: '12px',
                padding: '16px',
                marginBottom: '16px',
                border: '1px solid rgba(0, 122, 255, 0.2)'
              }}>
                <h4 style={{
                  fontSize: '16px',
                  fontWeight: '600',
                  color: '#007AFF',
                  margin: '0 0 8px 0'
                }}>
                  {userAccount.planName}
                </h4>
                <p style={{
                  fontSize: '12px',
                  color: '#888888',
                  margin: '0 0 4px 0'
                }}>
                  Plan ID: {userAccount.planId}
                </p>
                <p style={{
                  fontSize: '12px',
                  color: '#888888',
                  margin: 0
                }}>
                  Expires: {userAccount.expiresAt}
                </p>
              </div>

              <div style={{
                display: 'flex',
                gap: '8px',
                flexWrap: 'wrap'
              }}>
                <button style={{
                  background: '#007AFF',
                  color: 'white',
                  border: 'none',
                  borderRadius: '8px',
                  padding: '8px 16px',
                  fontSize: '12px',
                  fontWeight: '600',
                  cursor: 'pointer',
                  transition: 'all 0.3s ease',
                  flex: '1',
                  minWidth: '80px',
                  transform: 'translateZ(0)',
                  transformStyle: 'preserve-3d'
                }}
                onMouseOver={(e) => {
                  e.currentTarget.style.background = '#0056CC';
                  e.currentTarget.style.transform = 'translateZ(5px) scale(1.05)';
                }}
                onMouseOut={(e) => {
                  e.currentTarget.style.background = '#007AFF';
                  e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                }}
                >
                  Upgrade
                </button>
                
                <button style={{
                  background: 'rgba(255, 255, 255, 0.1)',
                  color: '#ffffff',
                  border: 'none',
                  borderRadius: '8px',
                  padding: '8px 16px',
                  fontSize: '12px',
                  fontWeight: '600',
                  cursor: 'pointer',
                  transition: 'all 0.3s ease',
                  flex: '1',
                  minWidth: '80px',
                  transform: 'translateZ(0)',
                  transformStyle: 'preserve-3d'
                }}
                onMouseOver={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.2)';
                  e.currentTarget.style.transform = 'translateZ(5px) scale(1.05)';
                }}
                onMouseOut={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
                  e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                }}
                >
                  Manage
                </button>
              </div>
            </div>

            {/* Quick Stats */}
            <div style={{
              background: 'rgba(255, 255, 255, 0.05)',
              borderRadius: '16px',
              padding: '24px',
              backdropFilter: 'blur(20px)',
              border: '1px solid rgba(255, 255, 255, 0.1)',
              transform: 'translateZ(0)',
              transformStyle: 'preserve-3d',
              boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)'
            }}>
              <h3 style={{
                fontSize: '18px',
                fontWeight: '600',
                color: '#ffffff',
                margin: '0 0 16px 0'
              }}>
                Quick Stats
              </h3>
              
              <div style={{ marginBottom: '16px' }}>
                <div style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  marginBottom: '6px'
                }}>
                  <span style={{
                    fontSize: '12px',
                    fontWeight: '500',
                    color: '#ffffff'
                  }}>
                    Gap Finder
                  </span>
                  <span style={{
                    fontSize: '12px',
                    color: '#007AFF',
                    fontWeight: '600'
                  }}>
                    {userAccount.gapFinderUsesRemaining}
                  </span>
                </div>
                <div style={{
                  background: 'rgba(255, 255, 255, 0.1)',
                  borderRadius: '6px',
                  height: '4px',
                  overflow: 'hidden'
                }}>
                  <div style={{
                    background: '#007AFF',
                    height: '100%',
                    width: `${Math.min((userAccount.gapFinderUsesRemaining / 10) * 100, 100)}%`,
                    transition: 'width 0.3s ease'
                  }}></div>
                </div>
              </div>

              <div style={{ marginBottom: '16px' }}>
                <div style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  marginBottom: '6px'
                }}>
                  <span style={{
                    fontSize: '12px',
                    fontWeight: '500',
                    color: '#ffffff'
                  }}>
                    Deep Analysis
                  </span>
                  <span style={{
                    fontSize: '12px',
                    color: '#30D158',
                    fontWeight: '600'
                  }}>
                    {userAccount.deepEvalUsesRemaining}
                  </span>
                </div>
                <div style={{
                  background: 'rgba(255, 255, 255, 0.1)',
                  borderRadius: '6px',
                  height: '4px',
                  overflow: 'hidden'
                }}>
                  <div style={{
                    background: '#30D158',
                    height: '100%',
                    width: `${Math.min((userAccount.deepEvalUsesRemaining / 10) * 100, 100)}%`,
                    transition: 'width 0.3s ease'
                  }}></div>
                </div>
              </div>

              {userAccount.hasSupport && (
                <div style={{
                  background: 'rgba(48, 209, 88, 0.1)',
                  borderRadius: '6px',
                  padding: '8px',
                  textAlign: 'center'
                }}>
                  <span style={{
                    fontSize: '10px',
                    color: '#30D158',
                    fontWeight: '600'
                  }}>
                    ✓ Team Support Included
                  </span>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Usage Tab */}
        {activeTab === 'usage' && (
          <div style={{
            background: 'rgba(255, 255, 255, 0.05)',
            borderRadius: '16px',
            padding: '30px',
            backdropFilter: 'blur(20px)',
            border: '1px solid rgba(255, 255, 255, 0.1)',
            maxWidth: '600px',
            margin: '0 auto',
            transform: 'translateZ(0)',
            transformStyle: 'preserve-3d',
            boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)'
          }}>
            <h3 style={{
              fontSize: '24px',
              fontWeight: '600',
              color: '#ffffff',
              margin: '0 0 24px 0'
            }}>
              Usage Details
            </h3>

            <div style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
              gap: '20px'
            }}>
              <div>
                <h4 style={{
                  fontSize: '16px',
                  fontWeight: '600',
                  color: '#ffffff',
                  margin: '0 0 12px 0'
                }}>
                  Research Gap Finder
                </h4>
                <div style={{
                  background: 'rgba(0, 122, 255, 0.1)',
                  borderRadius: '12px',
                  padding: '16px',
                  textAlign: 'center'
                }}>
                  <div style={{
                    fontSize: '28px',
                    fontWeight: '700',
                    color: '#007AFF',
                    margin: '0 0 6px 0'
                  }}>
                    {userAccount.gapFinderUsesRemaining}
                  </div>
                  <p style={{
                    fontSize: '12px',
                    color: '#888888',
                    margin: 0
                  }}>
                    uses remaining
                  </p>
                </div>
              </div>

              <div>
                <h4 style={{
                  fontSize: '16px',
                  fontWeight: '600',
                  color: '#ffffff',
                  margin: '0 0 12px 0'
                }}>
                  Deep Paper Analysis
                </h4>
                <div style={{
                  background: 'rgba(48, 209, 88, 0.1)',
                  borderRadius: '12px',
                  padding: '16px',
                  textAlign: 'center'
                }}>
                  <div style={{
                    fontSize: '28px',
                    fontWeight: '700',
                    color: '#30D158',
                    margin: '0 0 6px 0'
                  }}>
                    {userAccount.deepEvalUsesRemaining}
                  </div>
                  <p style={{
                    fontSize: '12px',
                    color: '#888888',
                    margin: 0
                  }}>
                    uses remaining
                  </p>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* Payments Tab */}
        {activeTab === 'payments' && (
          <div style={{
            background: 'rgba(255, 255, 255, 0.05)',
            borderRadius: '16px',
            padding: '30px',
            backdropFilter: 'blur(20px)',
            border: '1px solid rgba(255, 255, 255, 0.1)',
            maxWidth: '600px',
            margin: '0 auto',
            transform: 'translateZ(0)',
            transformStyle: 'preserve-3d',
            boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)'
          }}>
            <h3 style={{
              fontSize: '24px',
              fontWeight: '600',
              color: '#ffffff',
              margin: '0 0 24px 0'
            }}>
              Payment History
            </h3>

            {paymentHistory.length === 0 ? (
              <div style={{
                textAlign: 'center',
                padding: '40px 20px'
              }}>
                <div style={{
                  fontSize: '36px',
                  margin: '0 0 16px 0'
                }}>
                  💳
                </div>
                <h4 style={{
                  fontSize: '16px',
                  fontWeight: '600',
                  color: '#ffffff',
                  margin: '0 0 8px 0'
                }}>
                  No payment history
                </h4>
                <p style={{
                  fontSize: '12px',
                  color: '#888888',
                  margin: 0
                }}>
                  Your payment history will appear here once you make a purchase.
                </p>
              </div>
            ) : (
              <div style={{
                display: 'flex',
                flexDirection: 'column',
                gap: '12px'
              }}>
                {paymentHistory.map((payment) => (
                  <div
                    key={payment.id}
                    style={{
                      background: 'rgba(255, 255, 255, 0.05)',
                      borderRadius: '12px',
                      padding: '16px',
                      border: '1px solid rgba(255, 255, 255, 0.1)',
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center'
                    }}
                  >
                    <div>
                      <h5 style={{
                        fontSize: '14px',
                        fontWeight: '600',
                        color: '#ffffff',
                        margin: '0 0 4px 0'
                      }}>
                        {payment.package_name}
                      </h5>
                      <p style={{
                        fontSize: '12px',
                        color: '#888888',
                        margin: 0
                      }}>
                        {new Date(payment.created_at).toLocaleDateString()}
                      </p>
                    </div>
                    <div style={{ textAlign: 'right' }}>
                      <div style={{
                        fontSize: '16px',
                        fontWeight: '600',
                        color: '#ffffff',
                        margin: '0 0 4px 0'
                      }}>
                        ₹{payment.amount}
                      </div>
                      <span style={{
                        fontSize: '10px',
                        color: payment.status === 'completed' ? '#30D158' : '#FF9500',
                        fontWeight: '600',
                        background: payment.status === 'completed' ? 'rgba(48, 209, 88, 0.1)' : 'rgba(255, 149, 0, 0.1)',
                        padding: '2px 6px',
                        borderRadius: '4px'
                      }}>
                        {payment.status}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

export default UserDashboard;