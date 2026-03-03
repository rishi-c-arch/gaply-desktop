import React, { useState, useEffect } from 'react';
import { buildApiUrl } from '../api/config';
import PaymentSuccessOverlay from './PaymentSuccessOverlay';

interface PlanConfig {
  id: string;
  name: string;
  price: number;
  gapFinderUses: number;
  deepEvalUses: number;
  hasSupport: boolean;
  features: string[];
}

interface UserPlan {
  planId: string;
  planName: string;
  gapFinderUsesRemaining: number;
  deepEvalUsesRemaining: number;
  hasSupport: boolean;
  expiresAt: string;
}

const PremiumPage: React.FC = () => {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [userPlan, setUserPlan] = useState<UserPlan | null>(null);
  const [loading, setLoading] = useState(true);
  const [processing, setProcessing] = useState<string | null>(null);
  const [paymentSuccess, setPaymentSuccess] = useState(false);

  const plans: PlanConfig[] = [
    {
      id: "PLAN-471",
      name: "Gaply Basic",
      price: 471,
      gapFinderUses: 1,
      deepEvalUses: 0,
      hasSupport: false,
      features: [
        "Advanced Literature Analysis",
        "Research Gap Identification", 
        "Professional Report Generation",
        "1-time access to Gap Finder"
      ]
    },
    {
      id: "PLAN-1061", 
      name: "Gaply Plus",
      price: 1061,
      gapFinderUses: 2,
      deepEvalUses: 1,
      hasSupport: false,
      features: [
        "Advanced Literature Analysis",
        "Deep Paper Analysis",
        "Journal Compliance Checking",
        "2x Gap Finder + 1x Deep Evaluation",
        "Professional HTML Reports"
      ]
    },
    {
      id: "PLAN-2241",
      name: "Gaply Pro", 
      price: 2241,
      gapFinderUses: 5,
      deepEvalUses: 5,
      hasSupport: true,
      features: [
        "Advanced Literature Analysis",
        "Deep Paper Analysis", 
        "Journal Compliance Checking (Q1-Q4)",
        "5x Gap Finder + 5x Deep Evaluation",
        "Team Support Included",
        "Priority Processing"
      ]
    }
  ];

  const getToken = () => localStorage.getItem('authToken') || localStorage.getItem('token') || localStorage.getItem('gaply_token');

  useEffect(() => {
    checkAuthentication();
  }, []);

  const checkAuthentication = async () => {
    const token = getToken();
    const userEmail = localStorage.getItem('user_email');
    
    // Special handling for test account
    if (userEmail === 'testadmin@gaply.com') {
      setIsAuthenticated(true);
      setUserPlan({
        planId: 'TEST-ADMIN',
        planName: 'Test Admin Plan',
        gapFinderUsesRemaining: 999,
        deepEvalUsesRemaining: 999,
        hasSupport: true,
        expiresAt: 'Never expires'
      });
      setLoading(false);
      return;
    }
    
    if (!token) {
      setIsAuthenticated(false);
      setLoading(false);
      return;
    }

    try {
      const response = await fetch(buildApiUrl('/api/v1/user/profile'), {
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json'
        }
      });

      if (response.ok) {
        setIsAuthenticated(true);
        await checkUserPlan();
      } else {
        setIsAuthenticated(false);
        localStorage.removeItem('gaply_token');
        localStorage.removeItem('token');
      }
    } catch (error) {
      console.error('Auth check failed:', error);
      setIsAuthenticated(false);
    } finally {
      setLoading(false);
    }
  };

  const checkUserPlan = async () => {
    const token = getToken();
    if (!token) return;

    try {
      const response = await fetch(buildApiUrl('/api/v1/user/account'), {
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json'
        }
      });

      if (response.ok) {
        const data = await response.json();
        setUserPlan(data);
      }
    } catch (error) {
      console.error('Failed to fetch user plan:', error);
    }
  };

  const initiatePayment = async (planId: string) => {
    const token = getToken();
    if (!token) {
      alert('Please login first');
      return;
    }

    // Check if user is test admin account
    const userEmail = localStorage.getItem('user_email') || '';
    if (userEmail === 'testadmin@gaply.com') {
      // Bypass payment for test account
      alert('Test Account: Payment bypassed! You now have unlimited access to all premium features.');
      window.location.href = '/dashboard';
      return;
    }

    setProcessing(planId);

    try {
      const response = await fetch(buildApiUrl('/api/v1/payment/create-order'), {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({ package_id: planId })
      });

      const data = await response.json();
      
      if (data.success) {
        const options = {
          key: 'rzp_live_REMOVED',
          amount: data.data.amount,
          currency: 'INR',
          name: 'Gaply',
          description: `Premium Plan - ${data.data.package_name}`,
          order_id: data.data.razorpay_order_id,
          handler: async (response: any) => {
            await verifyPayment(response, planId);
          },
          prefill: {
            name: data.data.user_name,
            email: data.data.user_email,
          },
          theme: {
            color: '#ff7a1a'
          }
        };

        const razorpay = new (window as any).Razorpay(options);
        razorpay.open();
      } else {
        alert('Failed to create payment order: ' + data.error);
      }
    } catch (error) {
      console.error('Payment initiation failed:', error);
      alert('Payment failed. Please try again.');
    } finally {
      setProcessing(null);
    }
  };

  const verifyPayment = async (paymentData: any, planId: string) => {
    try {
      const response = await fetch(buildApiUrl('/api/v1/payment/verify'), {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${getToken()}`,
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          razorpay_order_id: paymentData.razorpay_order_id,
          razorpay_payment_id: paymentData.razorpay_payment_id,
          razorpay_signature: paymentData.razorpay_signature
        })
      });

      const data = await response.json();
      
      if (data.success) {
        setPaymentSuccess(true);
        await checkUserPlan();
      } else {
        alert('Payment verification failed: ' + data.error);
      }
    } catch (error) {
      console.error('Payment verification failed:', error);
      alert('Payment verification failed. Please contact support.');
    }
  };

  const handleLogin = () => {
    window.location.href = '/login';
  };

  if (loading) {
    return (
      <div style={{ 
        display: 'flex', 
        justifyContent: 'center', 
        alignItems: 'center', 
        height: '100vh',
        background: 'var(--app-bg)',
        color: 'var(--app-text)'
      }}>
        <div>Loading...</div>
      </div>
    );
  }

  if (!isAuthenticated) {
    return (
      <div style={{
        minHeight: '100vh',
        background: 'linear-gradient(135deg, var(--app-bg) 0%, var(--section-bg) 100%)',
        color: 'var(--app-text)',
        padding: '80px 20px',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center'
      }}>
        <div style={{ textAlign: 'center', maxWidth: '600px' }}>
          <h1 style={{ 
            fontSize: '3rem', 
            fontWeight: 'bold', 
            marginBottom: '20px',
            background: 'linear-gradient(45deg, #ff7a1a, var(--app-text))',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent'
          }}>
            Premium Access Required
          </h1>
          <p style={{ fontSize: '1.2rem', marginBottom: '40px', opacity: 0.8, color: 'var(--muted-text)' }}>
            Please login to access premium features and pricing
          </p>
          <button 
            onClick={handleLogin}
            style={{
              background: 'linear-gradient(45deg, #ff7a1a, #ff9500)',
              color: 'white',
              border: 'none',
              padding: '15px 30px',
              borderRadius: '25px',
              fontSize: '1.1rem',
              fontWeight: '600',
              cursor: 'pointer',
              transition: 'all 0.3s ease'
            }}
            onMouseEnter={(e) => e.currentTarget.style.transform = 'translateY(-2px)'}
            onMouseLeave={(e) => e.currentTarget.style.transform = 'translateY(0)'}
          >
            Login to Continue
          </button>
        </div>
      </div>
    );
  }

  return (
    <div style={{
      minHeight: '100vh',
      background: 'linear-gradient(135deg, var(--app-bg) 0%, var(--section-bg) 100%)',
      color: 'var(--app-text)',
      padding: '80px 20px'
    }}>
      {/* Debug button for test account */}
      <button 
        onClick={() => {
          console.log('Current user email:', localStorage.getItem('user_email'));
          console.log('Current authToken:', localStorage.getItem('authToken'));
          console.log('Current user data:', localStorage.getItem('user'));
          console.log('Is authenticated:', isAuthenticated);
          console.log('User plan:', userPlan);
        }}
        style={{
          position: 'fixed',
          top: '10px',
          right: '10px',
          zIndex: 9999,
          background: 'red',
          color: 'white',
          padding: '5px 10px',
          border: 'none',
          borderRadius: '5px'
        }}
      >
        Debug Premium
      </button>
      
      <div style={{ maxWidth: '1200px', margin: '0 auto' }}>
        {/* Header */}
        <div style={{ textAlign: 'center', marginBottom: '60px' }}>
          <h1 style={{ 
            fontSize: '3rem', 
            fontWeight: 'bold', 
            marginBottom: '20px',
            background: 'linear-gradient(45deg, #ff7a1a, var(--app-text))',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent'
          }}>
            Choose Your Plan
          </h1>
          <p style={{ fontSize: '1.2rem', opacity: 0.8, color: 'var(--muted-text)' }}>
            Unlock advanced research tools and accelerate your academic journey
          </p>
        </div>

        {/* Current Plan Status */}
        {userPlan && (
          <div style={{
            background: 'rgba(255, 122, 26, 0.1)',
            border: '1px solid rgba(255, 122, 26, 0.3)',
            borderRadius: '15px',
            padding: '20px',
            marginBottom: '40px',
            textAlign: 'center'
          }}>
            <h3 style={{ color: '#ff7a1a', marginBottom: '10px' }}>
              Current Plan: {userPlan.planName}
            </h3>
            <div style={{ display: 'flex', justifyContent: 'center', gap: '30px', flexWrap: 'wrap' }}>
              <div>
                <strong>Gap Finder:</strong> {userPlan.gapFinderUsesRemaining} uses remaining
              </div>
              <div>
                <strong>Deep Evaluation:</strong> {userPlan.deepEvalUsesRemaining} uses remaining
              </div>
              {userPlan.hasSupport && (
                <div>
                  <strong>Team Support:</strong> ✅ Included
                </div>
              )}
            </div>
          </div>
        )}

        {/* Pricing Plans */}
        <div style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(350px, 1fr))',
          gap: '30px',
          marginBottom: '60px'
        }}>
          {plans.map((plan) => (
            <div key={plan.id} style={{
              background: 'var(--card-bg)',
              border: '1px solid var(--card-border)',
              borderRadius: '20px',
              padding: '30px',
              textAlign: 'center',
              position: 'relative',
              transition: 'all 0.3s ease'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.transform = 'translateY(-10px)';
              e.currentTarget.style.borderColor = '#ff7a1a';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.transform = 'translateY(0)';
              e.currentTarget.style.borderColor = 'var(--card-border)';
            }}
            >
              {plan.name === 'Gaply Pro' && (
                <div style={{
                  position: 'absolute',
                  top: '-15px',
                  left: '50%',
                  transform: 'translateX(-50%)',
                  background: 'linear-gradient(45deg, #ff7a1a, #ff9500)',
                  color: 'white',
                  padding: '8px 20px',
                  borderRadius: '20px',
                  fontSize: '0.9rem',
                  fontWeight: '600'
                }}>
                  Most Popular
                </div>
              )}

              <h3 style={{ 
                fontSize: '1.8rem', 
                fontWeight: 'bold', 
                marginBottom: '10px',
                color: 'var(--app-text)'
              }}>
                {plan.name}
              </h3>

              <div style={{ marginBottom: '20px' }}>
                <span style={{ fontSize: '3rem', fontWeight: 'bold', color: '#ff7a1a' }}>
                  ₹{plan.price}
                </span>
                <span style={{ fontSize: '1rem', opacity: 0.7, marginLeft: '5px' }}>
                  (incl. GST)
                </span>
              </div>

              <ul style={{ 
                listStyle: 'none', 
                padding: 0, 
                marginBottom: '30px',
                textAlign: 'left'
              }}>
                {plan.features.map((feature, index) => (
                  <li key={index} style={{ 
                    marginBottom: '10px',
                    display: 'flex',
                    alignItems: 'center'
                  }}>
                    <span style={{ color: '#ff7a1a', marginRight: '10px' }}>✓</span>
                    <span style={{ color: 'var(--app-text)' }}>{feature}</span>
                  </li>
                ))}
              </ul>

              <button
                onClick={() => initiatePayment(plan.id)}
                disabled={processing === plan.id}
                style={{
                  width: '100%',
                  background: processing === plan.id 
                    ? 'rgba(255, 122, 26, 0.5)' 
                    : 'linear-gradient(45deg, #ff7a1a, #ff9500)',
                  color: 'white',
                  border: 'none',
                  padding: '15px',
                  borderRadius: '25px',
                  fontSize: '1.1rem',
                  fontWeight: '600',
                  cursor: processing === plan.id ? 'not-allowed' : 'pointer',
                  transition: 'all 0.3s ease'
                }}
              >
                {processing === plan.id ? 'Processing...' : `Get ${plan.name}`}
              </button>
            </div>
          ))}
        </div>

        {/* Features Comparison */}
        <div style={{
          background: 'var(--card-bg)',
          borderRadius: '20px',
          padding: '40px',
          marginBottom: '40px'
        }}>
          <h3 style={{ 
            textAlign: 'center', 
            fontSize: '2rem', 
            marginBottom: '30px',
            color: 'var(--app-text)'
          }}>
            Premium Features
          </h3>
          
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
            gap: '30px'
          }}>
            <div>
              <h4 style={{ color: '#ff7a1a', marginBottom: '15px' }}>
                🔍 Advanced Literature Analysis
              </h4>
              <p style={{ opacity: 0.8, color: 'var(--muted-text)' }}>
                Upload up to 5 research papers and get AI-powered gap identification with professional report generation.
              </p>
            </div>
            
            <div>
              <h4 style={{ color: '#ff7a1a', marginBottom: '15px' }}>
                📊 Deep Paper Analysis
              </h4>
              <p style={{ opacity: 0.8, color: 'var(--muted-text)' }}>
                70-parameter evaluation engine with journal compliance checking (Q1-Q4) and evidence-based analysis.
              </p>
            </div>
            
            <div>
              <h4 style={{ color: '#ff7a1a', marginBottom: '15px' }}>
                📄 Professional Reports
              </h4>
              <p style={{ opacity: 0.8, color: 'var(--muted-text)' }}>
                Generate comprehensive HTML reports with detailed insights and recommendations for your research.
              </p>
            </div>
          </div>
        </div>

        {/* Support Info */}
        <div style={{ textAlign: 'center', opacity: 0.7, color: 'var(--muted-text)' }}>
          <p>Need help choosing a plan? Contact our support team.</p>
          <p>All plans include 30-day money-back guarantee.</p>
        </div>
      </div>

      <PaymentSuccessOverlay
        show={paymentSuccess}
        title="Payment Successful!"
        message="Enjoy your premium features."
        onAutoDismiss={() => setPaymentSuccess(false)}
        autoDismissMs={4500}
      />
    </div>
  );
};

export default PremiumPage;
