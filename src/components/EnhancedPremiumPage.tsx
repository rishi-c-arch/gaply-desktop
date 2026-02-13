import React, { useState, useEffect } from 'react';
import { useAuth } from '../contexts/AuthContext';
import { premiumService, PlanConfig } from '../services/premiumService';

declare global {
  interface Window {
    Razorpay: any;
  }
}

const EnhancedPremiumPage: React.FC = () => {
  const { user, subscription, isAuthenticated, logout } = useAuth();
  const [plans, setPlans] = useState<PlanConfig[]>([]);
  const [loading, setLoading] = useState(true);
  const [processing, setProcessing] = useState<string | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    loadPlans();
  }, []);

  const loadPlans = async () => {
    try {
      setLoading(true);
      const availablePlans = await premiumService.getPlans();
      setPlans(availablePlans);
    } catch (error) {
      console.error('Error loading plans:', error);
      setError('Failed to load pricing plans. Please try again.');
    } finally {
      setLoading(false);
    }
  };

  const handlePurchase = async (planId: string) => {
    if (!isAuthenticated || !user) {
      setError('Please log in to purchase a plan');
      return;
    }

    console.log('User email:', user.email);
    console.log('Is test admin?', user.email === 'testadmin@gaply.com');

    // Special test account - bypass payment entirely
    if (user.email === 'testadmin@gaply.com') {
      console.log('Bypassing payment for test account');
      try {
        setProcessing(planId);
        setError('');
        
        // Simulate successful payment for test account
        alert('Test Account: Payment bypassed! You now have unlimited access to all premium features.');
        
        // Update subscription in context (this would normally happen after payment verification)
        // For test account, redirect to premium page to test features
        window.location.href = '/premium';
        return;
      } catch (error) {
        console.error('Test account setup error:', error);
        setError('An error occurred while setting up test account');
      } finally {
        setProcessing(null);
      }
    }

    console.log('Proceeding with normal payment flow');
    try {
      setProcessing(planId);
      setError('');

      // Create payment order
      const orderResult = await premiumService.createPaymentOrder(planId);
      
      if (!orderResult.success || !orderResult.data) {
        setError(orderResult.error || 'Failed to create payment order');
        return;
      }

      const { order, key_id: keyId } = orderResult.data;

      // Load Razorpay script
      if (!window.Razorpay) {
        const script = document.createElement('script');
        script.src = 'https://checkout.razorpay.com/v1/checkout.js';
        script.onload = () => {
          openRazorpayModal(order, keyId);
        };
        script.onerror = () => {
          setError('Failed to load payment gateway');
        };
        document.head.appendChild(script);
      } else {
        openRazorpayModal(order, keyId);
      }
    } catch (error) {
      console.error('Purchase error:', error);
      setError('An error occurred while processing your request');
    } finally {
      setProcessing(null);
    }
  };

  const openRazorpayModal = (order: any, keyId?: string) => {
    const options = {
      key: keyId || process.env.REACT_APP_RAZORPAY_KEY_ID || 'rzp_test_your_key_here',
      amount: order.amount,
      currency: order.currency,
      name: 'GAPLY',
      description: `Premium Plan - ${order.receipt}`,
      order_id: order.id,
      prefill: {
        name: user ? `${user.first_name} ${user.last_name}` : '',
        email: user?.email || '',
      },
      theme: {
        color: '#667eea'
      },
      handler: async function (response: any) {
        try {
          const verifyResult = await premiumService.verifyPayment(
            response.razorpay_order_id,
            response.razorpay_payment_id,
            response.razorpay_signature
          );

          if (verifyResult.success) {
            alert('Payment successful! Your premium features are now active.');
            // Refresh the page or update UI
            window.location.reload();
          } else {
            alert('Payment verification failed. Please contact support.');
          }
        } catch (error) {
          console.error('Payment verification error:', error);
          alert('Payment verification failed. Please contact support.');
        }
      },
      modal: {
        ondismiss: function() {
          setProcessing(null);
        }
      }
    };

    const rzp = new window.Razorpay(options);
    rzp.open();
  };

  const getPlanPrice = (plan: PlanConfig): string => {
    return `₹${plan.price.toLocaleString()}`;
  };

  const getPlanFeatures = (plan: PlanConfig): string[] => {
    const features: string[] = [];
    
    if (plan.features.gap_finder.included && plan.features.gap_finder.uses > 0) {
      features.push(`${plan.features.gap_finder.uses}x ${plan.features.gap_finder.name}`);
    }
    
    if (plan.features.deep_eval.included && plan.features.deep_eval.uses > 0) {
      features.push(`${plan.features.deep_eval.uses}x ${plan.features.deep_eval.name}`);
    }
    
    if (plan.features.support.included) {
      features.push('Priority Support');
    }
    
    return features;
  };

  if (loading) {
    return (
      <div style={{
        minHeight: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'linear-gradient(135deg, #f7fafc 0%, #edf2f7 100%)'
      }}>
        <div style={{
          textAlign: 'center',
          padding: '40px',
          background: 'white',
          borderRadius: '20px',
          boxShadow: '0 20px 60px rgba(0, 0, 0, 0.1)'
        }}>
          <div style={{
            width: '40px',
            height: '40px',
            border: '4px solid #e2e8f0',
            borderTop: '4px solid #667eea',
            borderRadius: '50%',
            animation: 'spin 1s linear infinite',
            margin: '0 auto 20px'
          }} />
          <p style={{
            fontSize: '16px',
            color: '#4a5568',
            fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
          }}>
            Loading premium plans...
          </p>
        </div>
      </div>
    );
  }

  return (
    <div style={{
      minHeight: '100vh',
      background: 'linear-gradient(135deg, #f7fafc 0%, #edf2f7 100%)',
      padding: '40px 20px'
    }}>
      <div style={{
        maxWidth: '1200px',
        margin: '0 auto'
      }}>
        {/* Header */}
        <div style={{
          textAlign: 'center',
          marginBottom: '60px'
        }}>
          <h1 style={{
            fontSize: 'clamp(2rem, 4vw, 3rem)',
            fontWeight: '700',
            color: '#2d3748',
            marginBottom: '16px',
            fontFamily: "'Space Grotesk', -apple-system, BlinkMacSystemFont, sans-serif"
          }}>
            Choose Your Premium Plan
          </h1>
          <p style={{
            fontSize: '1.2rem',
            color: '#718096',
            lineHeight: 1.6,
            fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
          }}>
            Unlock advanced research tools and accelerate your academic journey
          </p>
        </div>

        {/* User Status */}
        {isAuthenticated && user && (
          <div style={{
            background: 'white',
            borderRadius: '16px',
            padding: '24px',
            marginBottom: '40px',
            boxShadow: '0 4px 20px rgba(0, 0, 0, 0.1)',
            border: '1px solid #e2e8f0'
          }}>
            <div style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              flexWrap: 'wrap',
              gap: '16px'
            }}>
              <div>
                <h3 style={{
                  fontSize: '18px',
                  fontWeight: '600',
                  color: '#2d3748',
                  marginBottom: '4px',
                  fontFamily: "'Space Grotesk', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  Welcome, {user.first_name} {user.last_name}
                </h3>
                <p style={{
                  fontSize: '14px',
                  color: '#718096',
                  fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  {user.email}
                </p>
              </div>
              <div style={{
                display: 'flex',
                alignItems: 'center',
                gap: '12px'
              }}>
                <div style={{
                  padding: '8px 16px',
                  borderRadius: '20px',
                  background: user.is_premium 
                    ? 'linear-gradient(135deg, #48bb78, #38a169)' 
                    : 'linear-gradient(135deg, #a0aec0, #718096)',
                  color: 'white',
                  fontSize: '12px',
                  fontWeight: '600',
                  fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  {user.is_premium ? 'Premium User' : 'Free User'}
                </div>
                <button
                  onClick={logout}
                  style={{
                    padding: '8px 16px',
                    borderRadius: '8px',
                    border: '1px solid #e2e8f0',
                    background: 'white',
                    color: '#718096',
                    fontSize: '14px',
                    cursor: 'pointer',
                    transition: 'all 0.3s ease',
                    fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
                  }}
                  onMouseOver={(e) => {
                    e.currentTarget.style.borderColor = '#fc8181';
                    e.currentTarget.style.color = '#c53030';
                  }}
                  onMouseOut={(e) => {
                    e.currentTarget.style.borderColor = '#e2e8f0';
                    e.currentTarget.style.color = '#718096';
                  }}
                >
                  Logout
                </button>
              </div>
            </div>

            {/* Current Subscription */}
            {subscription && (
              <div style={{
                marginTop: '20px',
                padding: '16px',
                background: 'linear-gradient(135deg, #f7fafc, #edf2f7)',
                borderRadius: '12px',
                border: '1px solid #e2e8f0'
              }}>
                <h4 style={{
                  fontSize: '16px',
                  fontWeight: '600',
                  color: '#2d3748',
                  marginBottom: '8px',
                  fontFamily: "'Space Grotesk', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  Current Plan: {subscription.package_name}
                </h4>
                <div style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(150px, 1fr))',
                  gap: '16px',
                  fontSize: '14px',
                  fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  <div>
                    <span style={{ color: '#718096' }}>Gap Finder Uses:</span>
                    <span style={{ color: '#2d3748', fontWeight: '600', marginLeft: '8px' }}>
                      {subscription.remaining_uses.gap_finder || 0}
                    </span>
                  </div>
                  <div>
                    <span style={{ color: '#718096' }}>Deep Eval Uses:</span>
                    <span style={{ color: '#2d3748', fontWeight: '600', marginLeft: '8px' }}>
                      {subscription.remaining_uses.deep_eval || 0}
                    </span>
                  </div>
                  <div>
                    <span style={{ color: '#718096' }}>Support:</span>
                    <span style={{ color: '#2d3748', fontWeight: '600', marginLeft: '8px' }}>
                      {subscription.remaining_uses.support ? 'Yes' : 'No'}
                    </span>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Error Message */}
        {error && (
          <div style={{
            background: 'linear-gradient(135deg, #fed7d7, #feb2b2)',
            border: '1px solid #fc8181',
            borderRadius: '12px',
            padding: '16px',
            marginBottom: '40px',
            color: '#c53030',
            fontSize: '14px',
            fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
          }}>
            {error}
          </div>
        )}

        {/* Pricing Plans */}
        <div style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(350px, 1fr))',
          gap: '30px',
          marginBottom: '60px'
        }}>
          {plans.map((plan, index) => (
            <div
              key={plan.id}
              style={{
                background: 'white',
                borderRadius: '20px',
                padding: '32px',
                boxShadow: '0 10px 40px rgba(0, 0, 0, 0.1)',
                border: '2px solid #e2e8f0',
                position: 'relative',
                transition: 'all 0.3s ease',
                transform: index === 1 ? 'scale(1.05)' : 'scale(1)',
                zIndex: index === 1 ? 2 : 1
              }}
              onMouseOver={(e) => {
                e.currentTarget.style.transform = index === 1 ? 'scale(1.08)' : 'scale(1.03)';
                e.currentTarget.style.boxShadow = '0 20px 60px rgba(0, 0, 0, 0.15)';
              }}
              onMouseOut={(e) => {
                e.currentTarget.style.transform = index === 1 ? 'scale(1.05)' : 'scale(1)';
                e.currentTarget.style.boxShadow = '0 10px 40px rgba(0, 0, 0, 0.1)';
              }}
            >
              {/* Popular Badge */}
              {index === 1 && (
                <div style={{
                  position: 'absolute',
                  top: '-12px',
                  left: '50%',
                  transform: 'translateX(-50%)',
                  background: 'linear-gradient(135deg, #667eea, #764ba2)',
                  color: 'white',
                  padding: '8px 20px',
                  borderRadius: '20px',
                  fontSize: '12px',
                  fontWeight: '600',
                  fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  Most Popular
                </div>
              )}

              {/* Plan Header */}
              <div style={{ textAlign: 'center', marginBottom: '24px' }}>
                <h3 style={{
                  fontSize: '24px',
                  fontWeight: '700',
                  color: '#2d3748',
                  marginBottom: '8px',
                  fontFamily: "'Space Grotesk', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  {plan.name}
                </h3>
                <p style={{
                  fontSize: '14px',
                  color: '#718096',
                  marginBottom: '16px',
                  fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  {plan.description}
                </p>
                <div style={{
                  fontSize: '36px',
                  fontWeight: '700',
                  color: '#2d3748',
                  fontFamily: "'Space Grotesk', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  {getPlanPrice(plan)}
                </div>
                <p style={{
                  fontSize: '12px',
                  color: '#718096',
                  marginTop: '4px',
                  fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  One-time payment
                </p>
              </div>

              {/* Plan Features */}
              <div style={{ marginBottom: '32px' }}>
                <h4 style={{
                  fontSize: '16px',
                  fontWeight: '600',
                  color: '#2d3748',
                  marginBottom: '16px',
                  fontFamily: "'Space Grotesk', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  What's Included:
                </h4>
                <ul style={{
                  listStyle: 'none',
                  padding: 0,
                  margin: 0,
                  fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
                }}>
                  {getPlanFeatures(plan).map((feature, featureIndex) => (
                    <li key={featureIndex} style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '12px',
                      marginBottom: '12px',
                      fontSize: '14px',
                      color: '#4a5568'
                    }}>
                      <div style={{
                        width: '20px',
                        height: '20px',
                        borderRadius: '50%',
                        background: 'linear-gradient(135deg, #48bb78, #38a169)',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: 'white',
                        fontSize: '12px',
                        fontWeight: '600'
                      }}>
                        ✓
                      </div>
                      {feature}
                    </li>
                  ))}
                </ul>
              </div>

              {/* Purchase Button */}
              <button
                onClick={() => handlePurchase(plan.id)}
                disabled={processing === plan.id || !isAuthenticated}
                style={{
                  width: '100%',
                  padding: '16px 24px',
                  borderRadius: '12px',
                  border: 'none',
                  background: processing === plan.id
                    ? 'linear-gradient(135deg, #a0aec0, #cbd5e0)'
                    : !isAuthenticated
                    ? 'linear-gradient(135deg, #a0aec0, #cbd5e0)'
                    : 'linear-gradient(135deg, #667eea, #764ba2)',
                  color: 'white',
                  fontSize: '16px',
                  fontWeight: '600',
                  fontFamily: "'Space Grotesk', -apple-system, BlinkMacSystemFont, sans-serif",
                  cursor: processing === plan.id || !isAuthenticated ? 'not-allowed' : 'pointer',
                  transition: 'all 0.3s ease',
                  boxShadow: processing === plan.id || !isAuthenticated
                    ? 'none'
                    : '0 4px 15px rgba(102, 126, 234, 0.4)'
                }}
                onMouseOver={(e) => {
                  if (!processing && isAuthenticated) {
                    e.currentTarget.style.transform = 'translateY(-2px)';
                    e.currentTarget.style.boxShadow = '0 6px 20px rgba(102, 126, 234, 0.5)';
                  }
                }}
                onMouseOut={(e) => {
                  if (!processing && isAuthenticated) {
                    e.currentTarget.style.transform = 'translateY(0)';
                    e.currentTarget.style.boxShadow = '0 4px 15px rgba(102, 126, 234, 0.4)';
                  }
                }}
              >
                {processing === plan.id ? (
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}>
                    <div style={{
                      width: '16px',
                      height: '16px',
                      border: '2px solid rgba(255, 255, 255, 0.3)',
                      borderTop: '2px solid #ffffff',
                      borderRadius: '50%',
                      animation: 'spin 1s linear infinite'
                    }} />
                    Processing...
                  </div>
                ) : !isAuthenticated ? (
                  'Login Required'
                ) : (
                  'Purchase Now'
                )}
              </button>
            </div>
          ))}
        </div>

        {/* Features Overview */}
        <div style={{
          background: 'white',
          borderRadius: '20px',
          padding: '40px',
          boxShadow: '0 10px 40px rgba(0, 0, 0, 0.1)',
          border: '1px solid #e2e8f0'
        }}>
          <h2 style={{
            fontSize: '28px',
            fontWeight: '700',
            color: '#2d3748',
            textAlign: 'center',
            marginBottom: '40px',
            fontFamily: "'Space Grotesk', -apple-system, BlinkMacSystemFont, sans-serif"
          }}>
            Premium Features Overview
          </h2>
          
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
            gap: '32px'
          }}>
            {/* Gap Finder Feature */}
            <div style={{
              padding: '24px',
              background: 'linear-gradient(135deg, #f7fafc, #edf2f7)',
              borderRadius: '16px',
              border: '1px solid #e2e8f0'
            }}>
              <h3 style={{
                fontSize: '20px',
                fontWeight: '600',
                color: '#2d3748',
                marginBottom: '12px',
                fontFamily: "'Space Grotesk', -apple-system, BlinkMacSystemFont, sans-serif"
              }}>
                🔍 Advanced Literature Analysis
              </h3>
              <p style={{
                fontSize: '14px',
                color: '#4a5568',
                lineHeight: 1.6,
                marginBottom: '16px',
                fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
              }}>
                Upload up to 5 research papers and get comprehensive literature gap analysis with professional report generation.
              </p>
              <ul style={{
                listStyle: 'none',
                padding: 0,
                margin: 0,
                fontSize: '13px',
                color: '#4a5568',
                fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
              }}>
                <li style={{ marginBottom: '6px' }}>• Research gap identification</li>
                <li style={{ marginBottom: '6px' }}>• Citation network analysis</li>
                <li style={{ marginBottom: '6px' }}>• Professional PDF reports</li>
                <li>• Methodology recommendations</li>
              </ul>
            </div>

            {/* Deep Eval Feature */}
            <div style={{
              padding: '24px',
              background: 'linear-gradient(135deg, #f7fafc, #edf2f7)',
              borderRadius: '16px',
              border: '1px solid #e2e8f0'
            }}>
              <h3 style={{
                fontSize: '20px',
                fontWeight: '600',
                color: '#2d3748',
                marginBottom: '12px',
                fontFamily: "'Space Grotesk', -apple-system, BlinkMacSystemFont, sans-serif"
              }}>
                📊 Deep Paper Analysis
              </h3>
              <p style={{
                fontSize: '14px',
                color: '#4a5568',
                lineHeight: 1.6,
                marginBottom: '16px',
                fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
              }}>
                70-parameter evaluation engine with professional HTML reports and journal compliance checking.
              </p>
              <ul style={{
                listStyle: 'none',
                padding: 0,
                margin: 0,
                fontSize: '13px',
                color: '#4a5568',
                fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"
              }}>
                <li style={{ marginBottom: '6px' }}>• Comprehensive paper evaluation</li>
                <li style={{ marginBottom: '6px' }}>• Journal compliance checking</li>
                <li style={{ marginBottom: '6px' }}>• Evidence-based analysis</li>
                <li>• Interactive HTML reports</li>
              </ul>
            </div>
          </div>
        </div>
      </div>

      {/* CSS for spinner animation */}
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
};

export default EnhancedPremiumPage;
