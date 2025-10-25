import React, { useState, useRef, useEffect } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls, Text, Box, Sphere } from '@react-three/drei';
import { authService } from '../services/authService';
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
  const [currentView, setCurrentView] = useState<'plans' | 'account'>('plans');
  const [userAccount, setUserAccount] = useState<UserAccount>({
    package: 'Gaply Basic',
    gapFinderUses: 3,
    deepAnalysisUses: 0,
    teamSupportSessions: 0,
    whatsappSupport: false,
    validUntil: '2025-10-04'
  });

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
      background: '#000000',
      zIndex: 1000,
      fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif',
      overflow: 'hidden'
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

      {/* UI Overlay */}
      <div style={{
        position: 'relative',
        zIndex: 2,
        height: '100vh',
        display: 'flex',
        flexDirection: 'column',
        padding: '20px'
      }}>
      {/* Header */}
        <div style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '30px',
          paddingBottom: '20px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.1)'
        }}>
          <button
            onClick={onClose}
            style={{
              background: 'rgba(255, 255, 255, 0.1)',
              border: 'none',
              borderRadius: '8px',
              padding: '8px 16px',
              cursor: 'pointer',
              fontSize: '14px',
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
          ← Back
        </button>
          
          <div style={{ textAlign: 'center' }}>
            <h1 style={{
              fontSize: '32px',
              fontWeight: '600',
              color: '#ffffff',
              margin: '0 0 4px 0',
              letterSpacing: '-0.02em',
              lineHeight: '1.1',
              textShadow: '0 0 20px rgba(0, 122, 255, 0.5)'
            }}>
              Choose your plan.
            </h1>
            <p style={{
              fontSize: '16px',
              fontWeight: '400',
              color: '#888888',
              margin: 0,
              lineHeight: '1.3'
            }}>
              Select the research plan that fits your needs.
            </p>
          </div>
          
          <div style={{ width: '80px' }}></div>
        </div>
        
        {/* View Toggle */}
        <div style={{
          display: 'flex',
          justifyContent: 'center',
          marginBottom: '30px'
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
          <button 
            onClick={() => setCurrentView('plans')}
              style={{
                background: currentView === 'plans' ? '#007AFF' : 'transparent',
                color: currentView === 'plans' ? 'white' : '#ffffff',
                border: 'none',
                borderRadius: '8px',
                padding: '8px 20px',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
                transition: 'all 0.3s ease',
                transform: 'translateZ(0)',
                transformStyle: 'preserve-3d'
              }}
              onMouseOver={(e) => {
                if (currentView !== 'plans') {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
                  e.currentTarget.style.transform = 'translateZ(5px) scale(1.05)';
                }
              }}
              onMouseOut={(e) => {
                if (currentView !== 'plans') {
                  e.currentTarget.style.background = 'transparent';
                  e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                }
              }}
            >
              Plans
          </button>
          <button 
            onClick={() => setCurrentView('account')}
              style={{
                background: currentView === 'account' ? '#007AFF' : 'transparent',
                color: currentView === 'account' ? 'white' : '#ffffff',
                border: 'none',
                borderRadius: '8px',
                padding: '8px 20px',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
                transition: 'all 0.3s ease',
                transform: 'translateZ(0)',
                transformStyle: 'preserve-3d'
              }}
              onMouseOver={(e) => {
                if (currentView !== 'account') {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
                  e.currentTarget.style.transform = 'translateZ(5px) scale(1.05)';
                }
              }}
              onMouseOut={(e) => {
                if (currentView !== 'account') {
                  e.currentTarget.style.background = 'transparent';
                  e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                }
              }}
          >
            My Account
          </button>
        </div>
      </div>

        {/* 2D Package Fallback */}
        {currentView === 'plans' && (
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
            gap: '20px',
            marginBottom: '30px',
            maxWidth: '900px',
            margin: '0 auto 30px'
          }}>
            {packages.map((pkg) => (
              <div 
                key={pkg.id} 
                style={{
                  background: 'rgba(255, 255, 255, 0.05)',
                  borderRadius: '16px',
                  padding: '24px',
                  border: pkg.popular ? '2px solid #007AFF' : '1px solid rgba(255, 255, 255, 0.1)',
                  position: 'relative',
                  transition: 'all 0.3s ease',
                  cursor: 'pointer',
                  backdropFilter: 'blur(20px)',
                  transform: 'translateZ(0)',
                  transformStyle: 'preserve-3d',
                  boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)'
                }}
                onMouseOver={(e) => {
                  e.currentTarget.style.transform = 'translateZ(10px) scale(1.02)';
                  e.currentTarget.style.boxShadow = '0 12px 40px rgba(0, 0, 0, 0.4)';
                }}
                onMouseOut={(e) => {
                  e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                  e.currentTarget.style.boxShadow = '0 8px 32px rgba(0, 0, 0, 0.3)';
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
                    color: 'white',
                    padding: '6px 16px',
                    borderRadius: '16px',
                    fontSize: '12px',
                    fontWeight: '600'
                  }}>
                    Most Popular
                  </div>
                )}

                <h3 style={{
                  fontSize: '20px',
                  fontWeight: '600',
                  color: '#ffffff',
                  margin: '0 0 8px 0',
                  letterSpacing: '-0.01em'
                }}>
                  {pkg.name}
                </h3>

                <div style={{
                  fontSize: '32px',
                  fontWeight: '700',
                  color: '#007AFF',
                  margin: '0 0 12px 0',
                  letterSpacing: '-0.02em'
                }}>
                  {pkg.price}
                </div>
                
                <p style={{
                  fontSize: '14px',
                  color: '#888888',
                  margin: '0 0 20px 0',
                  lineHeight: '1.4'
                }}>
                  {pkg.description}
                </p>

                <ul style={{
                  listStyle: 'none',
                  padding: 0,
                  margin: '0 0 20px 0'
                }}>
                  {pkg.features.map((feature, index) => (
                    <li key={index} style={{
                      fontSize: '13px',
                      color: '#cccccc',
                      margin: '0 0 8px 0',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px'
                    }}>
                      <span style={{
                        color: '#30D158',
                        fontSize: '14px',
                        fontWeight: '600'
                      }}>✓</span>
                      {feature}
                    </li>
                  ))}
                </ul>
                
                <button style={{
                  width: '100%',
                  background: pkg.popular ? '#007AFF' : 'rgba(255, 255, 255, 0.1)',
                  color: pkg.popular ? 'white' : '#ffffff',
                  border: 'none',
                  borderRadius: '8px',
                  padding: '12px 20px',
                  fontSize: '14px',
                  fontWeight: '600',
                  cursor: 'pointer',
                  transition: 'all 0.3s ease',
                  transform: 'translateZ(0)',
                  transformStyle: 'preserve-3d'
                }}
                onMouseOver={(e) => {
                  if (!pkg.popular) {
                    e.currentTarget.style.background = 'rgba(255, 255, 255, 0.2)';
                    e.currentTarget.style.transform = 'translateZ(5px) scale(1.05)';
                  }
                }}
                onMouseOut={(e) => {
                  if (!pkg.popular) {
                    e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
                    e.currentTarget.style.transform = 'translateZ(0) scale(1)';
                  }
                }}
                >
                  Get {pkg.name}
                </button>
              </div>
            ))}
          </div>
        )}

        {/* Account View */}
        {currentView === 'account' && (
          <div style={{
            maxWidth: '500px',
            margin: '0 auto',
            background: 'rgba(255, 255, 255, 0.05)',
            borderRadius: '16px',
            padding: '30px',
            backdropFilter: 'blur(20px)',
            border: '1px solid rgba(255, 255, 255, 0.1)',
            transform: 'translateZ(0)',
            transformStyle: 'preserve-3d',
            boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)'
          }}>
            <h2 style={{
              fontSize: '24px',
              fontWeight: '600',
              color: '#ffffff',
              margin: '0 0 20px 0',
              letterSpacing: '-0.01em'
            }}>
              Current Package
            </h2>

            <div style={{
              background: 'rgba(0, 122, 255, 0.1)',
              borderRadius: '12px',
              padding: '20px',
              marginBottom: '20px',
              border: '1px solid rgba(0, 122, 255, 0.2)'
            }}>
              <h3 style={{
                fontSize: '18px',
                fontWeight: '600',
                color: '#007AFF',
                margin: '0 0 8px 0'
              }}>
                {userAccount.package}
              </h3>
              <p style={{
                fontSize: '14px',
                color: '#888888',
                margin: 0
              }}>
                Valid until: {userAccount.validUntil}
              </p>
                </div>

            <h3 style={{
              fontSize: '20px',
              fontWeight: '600',
              color: '#ffffff',
              margin: '0 0 16px 0'
            }}>
              Usage Statistics
            </h3>

            <div style={{ marginBottom: '20px' }}>
              <div style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                marginBottom: '8px'
              }}>
                <span style={{
                  fontSize: '14px',
                  fontWeight: '500',
                  color: '#ffffff'
                }}>
                  Research Gap Finder
                </span>
                <span style={{
                  fontSize: '14px',
                  color: '#888888'
                }}>
                  {userAccount.gapFinderUses}/5 uses left
                </span>
              </div>
              <div style={{
                background: 'rgba(255, 255, 255, 0.1)',
                borderRadius: '8px',
                height: '6px',
                overflow: 'hidden'
              }}>
                <div style={{
                  background: 'linear-gradient(90deg, #007AFF, #30D158)',
                  height: '100%',
                  width: `${(userAccount.gapFinderUses / 5) * 100}%`,
                  transition: 'width 0.3s ease'
                }}></div>
              </div>
            </div>

            <div style={{
              display: 'flex',
              gap: '12px',
              flexWrap: 'wrap'
            }}>
              <button style={{
                background: '#007AFF',
                color: 'white',
                border: 'none',
                borderRadius: '8px',
                padding: '12px 20px',
                fontSize: '14px',
                fontWeight: '600',
                cursor: 'pointer',
                transition: 'all 0.3s ease',
                flex: '1',
                minWidth: '120px',
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
                View Dashboard
              </button>
              
              <button style={{
                background: 'rgba(255, 255, 255, 0.1)',
                color: '#ffffff',
                border: 'none',
                borderRadius: '8px',
                padding: '12px 20px',
                fontSize: '14px',
                fontWeight: '600',
                cursor: 'pointer',
                transition: 'all 0.3s ease',
                flex: '1',
                minWidth: '120px',
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
                  Upgrade Package
                </button>
            </div>

            <div style={{
              textAlign: 'center',
              marginTop: '20px'
            }}>
              <button style={{
                background: 'transparent',
                color: '#007AFF',
                border: 'none',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
                textDecoration: 'underline',
                transition: 'all 0.3s ease'
              }}
              onMouseOver={(e) => {
                e.currentTarget.style.color = '#0056CC';
              }}
              onMouseOut={(e) => {
                e.currentTarget.style.color = '#007AFF';
              }}
              >
                  Contact Support
                </button>
            </div>
          </div>
        )}

        {/* Footer */}
        <div style={{
          textAlign: 'center',
          marginTop: 'auto',
          paddingTop: '20px',
          borderTop: '1px solid rgba(255, 255, 255, 0.1)'
        }}>
          <p style={{
            fontSize: '12px',
            color: '#666666',
            margin: '0 0 4px 0'
          }}>
            All plans include 30-day money-back guarantee
          </p>
          <p style={{
            fontSize: '12px',
            color: '#666666',
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