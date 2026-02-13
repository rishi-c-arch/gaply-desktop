import React, { useState, useRef, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls, Text } from '@react-three/drei';
import * as THREE from 'three';

interface PackageSelectionProps {
  onClose: () => void;
}

interface UserAccount {
  package: string;
  gapFinderUses: number;
  deepAnalysisUses: number;
  teamSupportSessions: number;
  whatsappSupport: boolean;
  validUntil: string;
}

// 3D Floating Particles Component
const FloatingParticles: React.FC = () => {
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const particles = useRef<THREE.Object3D[]>([]);

  useEffect(() => {
    if (meshRef.current) {
      for (let i = 0; i < 50; i++) {
        const particle = new THREE.Object3D();
        particle.position.set(
          (Math.random() - 0.5) * 20,
          (Math.random() - 0.5) * 20,
          (Math.random() - 0.5) * 20
        );
        particle.scale.setScalar(Math.random() * 0.5 + 0.5);
        particles.current.push(particle);
        meshRef.current.setMatrixAt(i, particle.matrix);
      }
      meshRef.current.instanceMatrix.needsUpdate = true;
    }
  }, []);

  useFrame((state) => {
    if (meshRef.current) {
      particles.current.forEach((particle, i) => {
        particle.rotation.y += 0.01;
        particle.rotation.x += 0.005;
        particle.position.y += Math.sin(state.clock.elapsedTime + i) * 0.001;
        particle.updateMatrix();
        meshRef.current!.setMatrixAt(i, particle.matrix);
      });
      meshRef.current.instanceMatrix.needsUpdate = true;
    }
  });

  return (
    <instancedMesh ref={meshRef} args={[undefined, undefined, 50]}>
      <sphereGeometry args={[0.1, 8, 8]} />
      <meshBasicMaterial color="#007AFF" transparent opacity={0.3} />
    </instancedMesh>
  );
};

// 3D Package Card Component
const PackageCard3D: React.FC<{
  position: [number, number, number];
  package: any;
  onClick: () => void;
  isPopular: boolean;
}> = ({ position, package: pkg, onClick, isPopular }) => {
  const meshRef = useRef<THREE.Mesh>(null);
  const [hovered, setHovered] = useState(false);

  useFrame((state) => {
    if (meshRef.current) {
      meshRef.current.rotation.y = Math.sin(state.clock.elapsedTime) * 0.1;
      meshRef.current.position.y = position[1] + Math.sin(state.clock.elapsedTime + position[0]) * 0.1;
    }
  });

  return (
    <group position={position}>
      {/* Main Card */}
      <mesh
        ref={meshRef}
        onClick={onClick}
        onPointerOver={() => setHovered(true)}
        onPointerOut={() => setHovered(false)}
        scale={hovered ? 1.1 : 1}
      >
        <boxGeometry args={[3, 4, 0.2]} />
        <meshStandardMaterial 
          color={isPopular ? "#007AFF" : "#1a1a1a"} 
          metalness={0.8}
          roughness={0.2}
          emissive={isPopular ? "#007AFF" : "#000000"}
          emissiveIntensity={0.1}
        />
      </mesh>

      {/* Popular Badge */}
      {isPopular && (
        <mesh position={[0, 1.8, 0.15]}>
          <boxGeometry args={[1.5, 0.3, 0.1]} />
          <meshStandardMaterial color="#FF9500" />
        </mesh>
      )}

      {/* Package Name */}
      <Text
        position={[0, 1.2, 0.15]}
        fontSize={0.3}
        color="#ffffff"
        anchorX="center"
        anchorY="middle"
      >
        {pkg.name}
      </Text>

      {/* Price */}
      <Text
        position={[0, 0.8, 0.15]}
        fontSize={0.4}
        color="#007AFF"
        anchorX="center"
        anchorY="middle"
      >
        {pkg.price}
      </Text>

      {/* Features */}
      {pkg.features.slice(0, 3).map((feature: string, index: number) => (
        <Text
          key={index}
          position={[0, 0.2 - index * 0.3, 0.15]}
          fontSize={0.15}
          color="#cccccc"
          anchorX="center"
          anchorY="middle"
        >
          ✓ {feature}
        </Text>
      ))}
    </group>
  );
};

const PackageSelection: React.FC<PackageSelectionProps> = ({ onClose }) => {
  const navigate = useNavigate();
  const [currentView, setCurrentView] = useState<'plans' | 'account'>('plans');
  const [userAccount, setUserAccount] = useState<UserAccount>({
    package: 'Gaply Basic',
    gapFinderUses: 3,
    deepAnalysisUses: 0,
    teamSupportSessions: 0,
    whatsappSupport: false,
    validUntil: '2025-10-04'
  });

  const gapFinderTotal = 5;
  const deepAnalysisTotal = userAccount.package === 'Gaply Pro' ? 5 : userAccount.package === 'Gaply Plus' ? 1 : 0;
  const showDeepAnalysis = deepAnalysisTotal > 0;

  const packages = [
    {
      id: 'basic',
      name: 'Gaply Basic',
      price: '₹471',
      description: 'Research Gap Finder - Analyze 5 base papers to identify research gaps and opportunities',
      features: [
        'Research Gap Finder (1 use)',
        'Analyze 5 base papers',
        'Publishable problem statement',
        'Clear objectives & hypotheses',
        'Valid for 365 days'
      ],
      popular: false
    },
    {
      id: 'plus',
      name: 'Gaply Plus',
      price: '₹1061',
      description: 'Research Pro - Gap Finder (2 uses) + Deep Paper Analysis (1 use) with 70+ parameter evaluation',
      features: [
        'Research Gap Finder (2 uses)',
        'Deep Paper Analysis (1 use)',
        '70+ parameter evaluation',
        'Journal compliance checking',
        'Valid for 365 days'
      ],
      popular: true
    },
    {
      id: 'pro',
      name: 'Gaply Pro',
      price: '₹2241',
      description: 'Research Elite - Full access (5 uses each) + Team support with 30min session + WhatsApp support',
      features: [
        'Research Gap Finder (5 uses)',
        'Deep Paper Analysis (5 uses)',
        'Team support (30min session)',
        'WhatsApp support',
        'Valid for 365 days'
      ],
      popular: false
    }
  ];

  const handlePackageSelect = async (packageId: string) => {
    const userEmail = localStorage.getItem('user_email');
    
    if (userEmail === 'testadmin@gaply.com') {
      alert('Test Account: Payment bypassed! Redirecting to premium page...');
      window.location.href = '/premium';
      return;
    }

    try {
      console.log('Selected package:', packageId);
    } catch (error) {
      console.error('Package selection error:', error);
    }
  };

  return (
    <div style={{
      position: 'fixed',
      top: 0,
      left: 0,
      right: 0,
      bottom: 0,
      background: 'var(--dash-bg)',
      zIndex: 1000,
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Segoe UI", Roboto, Helvetica, Arial, sans-serif',
      overflow: 'auto'
    }}>
      {/* 3D Background Canvas - Simplified */}
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
          <pointLight position={[-10, -10, -10]} intensity={0.3} color="#007AFF" />
          
          <FloatingParticles />
          
          <OrbitControls enableZoom={false} enablePan={false} />
        </Canvas>
      </div>

      {/* UI Overlay - Design spec: max-width 1024px, spacing scale, typography */}
      <div style={{
        position: 'relative',
        zIndex: 2,
        minHeight: '100vh',
        display: 'flex',
        flexDirection: 'column',
        padding: 'clamp(24px, 5vw, 80px)',
        maxWidth: 'var(--dash-content-max)',
        margin: '0 auto'
      }}>
        {/* Header */}
        <div style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: 'var(--dash-space-md)',
          paddingBottom: 'var(--dash-space-md)',
          borderBottom: '1px solid var(--dash-border)'
        }}>
          <button
            onClick={() => { onClose(); navigate('/'); }}
            style={{
              background: 'var(--dash-glow)',
              border: '1px solid var(--dash-border)',
              borderRadius: '12px',
              padding: '10px 18px',
              cursor: 'pointer',
              fontSize: '17px',
              fontWeight: '500',
              color: 'var(--dash-text)',
              transition: 'all 0.25s ease',
              backdropFilter: 'blur(10px)'
            }}
            onMouseOver={(e) => {
              e.currentTarget.style.background = 'var(--dash-text-secondary)';
              e.currentTarget.style.borderColor = 'var(--dash-border)';
            }}
            onMouseOut={(e) => {
              e.currentTarget.style.background = 'var(--dash-glow)';
              e.currentTarget.style.borderColor = 'var(--dash-border)';
            }}
          >
            ← Back
          </button>

          <div style={{ textAlign: 'center' }}>
            <h1 style={{
              fontSize: 'clamp(28px, 4vw, 48px)',
              fontWeight: 600,
              color: 'var(--dash-text)',
              margin: '0 0 8px 0',
              letterSpacing: '-0.02em',
              lineHeight: 1.1
            }}>
              Choose your plan.
            </h1>
            <p style={{
              fontSize: '17px',
              fontWeight: 400,
              color: 'var(--dash-text-secondary)',
              margin: 0,
              lineHeight: 1.5
            }}>
              Select the research plan that fits your needs.
            </p>
          </div>

          <div style={{ width: '80px' }} />
        </div>

        {/* View Toggle - Plans | My Account */}
        <div style={{
          display: 'flex',
          justifyContent: 'center',
          marginBottom: 'var(--dash-space-lg)'
        }}>
          <div style={{
            background: 'var(--dash-glow)',
            borderRadius: '12px',
            padding: '4px',
            display: 'flex',
            gap: '4px',
            border: '1px solid var(--dash-border)'
          }}>
            <button
              onClick={() => setCurrentView('plans')}
              style={{
                background: currentView === 'plans' ? '#007AFF' : 'transparent',
                color: 'var(--dash-text)',
                border: 'none',
                borderRadius: '8px',
                padding: '10px 24px',
                fontSize: '17px',
                fontWeight: 500,
                cursor: 'pointer',
                transition: 'all 0.25s ease'
              }}
              onMouseOver={(e) => {
                if (currentView !== 'plans') e.currentTarget.style.background = 'var(--dash-border)';
              }}
              onMouseOut={(e) => {
                if (currentView !== 'plans') e.currentTarget.style.background = 'transparent';
              }}
            >
              Plans
            </button>
            <button
              onClick={() => setCurrentView('account')}
              style={{
                background: currentView === 'account' ? '#007AFF' : 'transparent',
                color: 'var(--dash-text)',
                border: 'none',
                borderRadius: '8px',
                padding: '10px 24px',
                fontSize: '17px',
                fontWeight: 500,
                cursor: 'pointer',
                transition: 'all 0.25s ease'
              }}
              onMouseOver={(e) => {
                if (currentView !== 'account') e.currentTarget.style.background = 'var(--dash-border)';
              }}
              onMouseOut={(e) => {
                if (currentView !== 'account') e.currentTarget.style.background = 'transparent';
              }}
            >
              My Account
            </button>
          </div>
        </div>

        {/* Plans grid - 12-col feel, 24px gutters */}
        {currentView === 'plans' && (
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
            gap: 'var(--dash-space-md)',
            marginBottom: 'var(--dash-space-lg)'
          }}>
            {packages.map((pkg) => (
              <div
                key={pkg.id}
                style={{
                  background: 'var(--card-bg)',
                  borderRadius: '20px',
                  padding: 'var(--dash-space-md)',
                  border: pkg.popular ? '2px solid #007AFF' : '1px solid var(--dash-border)',
                  position: 'relative',
                  transition: 'all 0.25s ease',
                  cursor: 'pointer',
                  boxShadow: 'var(--card-shadow)'
                }}
                onMouseOver={(e) => {
                  e.currentTarget.style.background = 'var(--card-bg-hover)';
                  e.currentTarget.style.boxShadow = 'var(--card-shadow-hover)';
                }}
                onMouseOut={(e) => {
                  e.currentTarget.style.background = 'var(--card-bg)';
                  e.currentTarget.style.boxShadow = 'var(--card-shadow)';
                }}
                onClick={() => handlePackageSelect(pkg.id)}
              >
                {pkg.popular && (
                  <div style={{
                    position: 'absolute',
                    top: '-12px',
                    left: '50%',
                    transform: 'translateX(-50%)',
                    background: '#007AFF',
                    color: '#ffffff',
                    padding: '6px 16px',
                    borderRadius: '20px',
                    fontSize: '12px',
                    fontWeight: 600,
                    letterSpacing: '0.02em',
                    textTransform: 'uppercase'
                  }}>
                    Most Popular
                  </div>
                )}

                <h3 style={{
                  fontSize: '28px',
                  fontWeight: 600,
                  color: 'var(--dash-text)',
                  margin: '0 0 8px 0',
                  letterSpacing: '-0.01em'
                }}>
                  {pkg.name}
                </h3>

                <div style={{
                  fontSize: 'clamp(28px, 3vw, 36px)',
                  fontWeight: 700,
                  color: '#007AFF',
                  margin: '0 0 12px 0',
                  letterSpacing: '-0.02em'
                }}>
                  {pkg.price}
                </div>

                <p style={{
                  fontSize: '17px',
                  color: 'var(--dash-text-secondary)',
                  margin: '0 0 20px 0',
                  lineHeight: 1.5
                }}>
                  {pkg.description}
                </p>

                <ul style={{ listStyle: 'none', padding: 0, margin: '0 0 24px 0' }}>
                  {pkg.features.map((feature, index) => (
                    <li key={index} style={{
                      fontSize: '17px',
                      color: 'var(--dash-text-secondary)',
                      margin: '0 0 12px 0',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '12px'
                    }}>
                      <span style={{ color: '#30D158', fontWeight: 600 }}>✓</span>
                      {feature}
                    </li>
                  ))}
                </ul>

                <button
                  style={{
                    width: '100%',
                    background: pkg.popular ? '#007AFF' : 'var(--dash-glow)',
                    color: 'var(--dash-text)',
                    border: pkg.popular ? 'none' : '1px solid var(--dash-border)',
                    borderRadius: '12px',
                    padding: '14px 24px',
                    fontSize: '17px',
                    fontWeight: 600,
                    cursor: 'pointer',
                    transition: 'all 0.25s ease'
                  }}
                  onMouseOver={(e) => {
                    if (!pkg.popular) e.currentTarget.style.background = 'var(--dash-border)';
                  }}
                  onMouseOut={(e) => {
                    if (!pkg.popular) e.currentTarget.style.background = 'var(--dash-glow)';
                  }}
                >
                  Get {pkg.name}
                </button>
              </div>
            ))}
          </div>
        )}

        {/* My Account View - Design spec typography & spacing */}
        {currentView === 'account' && (
          <div style={{
            maxWidth: '560px',
            margin: '0 auto',
            background: 'var(--card-bg)',
            borderRadius: '24px',
            padding: 'var(--dash-space-lg)',
            border: '1px solid var(--dash-border)',
            boxShadow: 'var(--card-shadow)'
          }}>
            <h2 style={{
              fontSize: '28px',
              fontWeight: 600,
              color: 'var(--dash-text)',
              margin: '0 0 var(--dash-space-md) 0',
              letterSpacing: '-0.01em'
            }}>
              Current Package
            </h2>

            <div style={{
              background: 'var(--dash-glow)',
              borderRadius: '16px',
              padding: 'var(--dash-space-md)',
              marginBottom: 'var(--dash-space-lg)',
              border: '1px solid var(--dash-border)'
            }}>
              <h3 style={{
                fontSize: '21px',
                fontWeight: 600,
                color: '#007AFF',
                margin: '0 0 8px 0'
              }}>
                {userAccount.package}
              </h3>
              <p style={{
                fontSize: '17px',
                color: 'var(--dash-text-secondary)',
                margin: 0
              }}>
                Valid until: {userAccount.validUntil}
              </p>
            </div>

            <h3 style={{
              fontSize: '28px',
              fontWeight: 600,
              color: 'var(--dash-text)',
              margin: '0 0 var(--dash-space-md) 0'
            }}>
              Usage Statistics
            </h3>

            <div style={{ marginBottom: 'var(--dash-space-md)' }}>
              <div style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                marginBottom: '8px'
              }}>
                <span style={{
                  fontSize: '17px',
                  fontWeight: 500,
                  color: 'var(--dash-text)'
                }}>
                  Research Gap Finder
                </span>
                <span style={{ fontSize: '17px', color: 'var(--dash-text-secondary)' }}>
                  {userAccount.gapFinderUses}/{gapFinderTotal} uses left
                </span>
              </div>
              <div style={{
                background: 'var(--dash-border)',
                borderRadius: '8px',
                height: '8px',
                overflow: 'hidden'
              }}>
                <div style={{
                  background: 'linear-gradient(90deg, #007AFF, #30D158)',
                  height: '100%',
                  width: `${(userAccount.gapFinderUses / gapFinderTotal) * 100}%`,
                  transition: 'width 0.3s ease'
                }} />
              </div>
            </div>

            {showDeepAnalysis && (
              <div style={{ marginBottom: 'var(--dash-space-md)' }}>
                <div style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  marginBottom: '8px'
                }}>
                  <span style={{
                    fontSize: '17px',
                    fontWeight: 500,
                    color: 'var(--dash-text)'
                  }}>
                    Deep Paper Analysis
                  </span>
                  <span style={{ fontSize: '17px', color: 'var(--dash-text-secondary)' }}>
                    {userAccount.deepAnalysisUses}/{deepAnalysisTotal} uses left
                  </span>
                </div>
                <div style={{
                  background: 'var(--dash-border)',
                  borderRadius: '8px',
                  height: '8px',
                  overflow: 'hidden'
                }}>
                  <div style={{
                    background: 'linear-gradient(90deg, #007AFF, #30D158)',
                    height: '100%',
                    width: `${(userAccount.deepAnalysisUses / deepAnalysisTotal) * 100}%`,
                    transition: 'width 0.3s ease'
                  }} />
                </div>
              </div>
            )}

            <div style={{
              display: 'flex',
              gap: 'var(--dash-space-sm)',
              flexWrap: 'wrap'
            }}>
              <button
                onClick={() => navigate('/dashboard')}
                style={{
                  background: '#007AFF',
                  color: '#ffffff',
                  border: 'none',
                  borderRadius: '12px',
                  padding: '14px 24px',
                  fontSize: '17px',
                  fontWeight: 600,
                  cursor: 'pointer',
                  transition: 'all 0.25s ease',
                  flex: '1',
                  minWidth: '140px'
                }}
                onMouseOver={(e) => { e.currentTarget.style.background = '#0056CC'; }}
                onMouseOut={(e) => { e.currentTarget.style.background = '#007AFF'; }}
              >
                View Dashboard
              </button>
              <button
                onClick={() => setCurrentView('plans')}
                style={{
                  background: 'var(--dash-glow)',
                  color: 'var(--dash-text)',
                  border: '1px solid var(--dash-border)',
                  borderRadius: '12px',
                  padding: '14px 24px',
                  fontSize: '17px',
                  fontWeight: 600,
                  cursor: 'pointer',
                  transition: 'all 0.25s ease',
                  flex: '1',
                  minWidth: '140px'
                }}
                onMouseOver={(e) => { e.currentTarget.style.background = 'var(--dash-border)'; }}
                onMouseOut={(e) => { e.currentTarget.style.background = 'var(--dash-glow)'; }}
              >
                Upgrade Package
              </button>
            </div>

            <div style={{ textAlign: 'center', marginTop: 'var(--dash-space-md)' }}>
              <button
                onClick={() => navigate('/contact')}
                style={{
                  background: 'transparent',
                  color: '#007AFF',
                  border: 'none',
                  fontSize: '17px',
                  fontWeight: 500,
                  cursor: 'pointer',
                  textDecoration: 'underline',
                  transition: 'color 0.2s ease'
                }}
                onMouseOver={(e) => { e.currentTarget.style.color = '#0056CC'; }}
                onMouseOut={(e) => { e.currentTarget.style.color = '#007AFF'; }}
              >
                Contact Support
              </button>
            </div>
          </div>
        )}

        {/* Footer - Design spec caption */}
        <div style={{
          textAlign: 'center',
          marginTop: 'auto',
          paddingTop: 'var(--dash-space-md)',
          borderTop: '1px solid var(--dash-border)'
        }}>
          <p style={{
            fontSize: '12px',
            fontWeight: 500,
            letterSpacing: '0.02em',
            color: 'var(--dash-text-tertiary)',
            margin: '0 0 4px 0'
          }}>
            All plans include 30-day money-back guarantee
          </p>
          <p style={{
            fontSize: '12px',
            color: 'var(--dash-text-tertiary)',
            margin: 0
          }}>
            Need help choosing? Contact our support team
          </p>
        </div>
      </div>
    </div>
  );
};

export default PackageSelection;