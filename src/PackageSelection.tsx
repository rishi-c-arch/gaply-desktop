import React, { useState } from 'react';
import './PackageSelection.css';

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

const PackageSelection: React.FC<PackageSelectionProps> = ({ onClose }) => {
  const [currentView, setCurrentView] = useState<'plans' | 'account'>('plans');
  // Removed unused showAccountPanel state
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
      price: '₹471.00',
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
      price: '₹1061.00',
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
      price: '₹2241.00',
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
    console.log('Selected package:', packageId);
    
    const selectedPackage = packages.find(pkg => pkg.id === packageId);
    if (!selectedPackage) return;

    // Initialize Razorpay
    const script = document.createElement('script');
    script.src = 'https://checkout.razorpay.com/v1/checkout.js';
    script.async = true;
    document.body.appendChild(script);

    script.onload = () => {
      const options = {
        key: 'rzp_live_REMOVED',
        amount: parseInt(selectedPackage.price.replace(/[₹,]/g, '')) * 100, // Convert to paise
        currency: 'INR',
        name: 'GAPLY Research Platform',
        description: selectedPackage.name,
        image: 'https://gaply.in/logo.png',
        order_id: '', // You would generate this from your backend
        handler: function (response: any) {
          console.log('Payment successful:', response);
          // Update user account with selected package
          setUserAccount(prev => ({
            ...prev,
            package: selectedPackage.name,
            gapFinderUses: selectedPackage.id === 'basic' ? 1 : selectedPackage.id === 'plus' ? 2 : 5,
            deepAnalysisUses: selectedPackage.id === 'pro' ? 5 : selectedPackage.id === 'plus' ? 1 : 0,
            teamSupportSessions: selectedPackage.id === 'pro' ? 1 : 0,
            whatsappSupport: selectedPackage.id === 'pro',
            validUntil: new Date(Date.now() + 365 * 24 * 60 * 60 * 1000).toISOString().split('T')[0]
          }));
          
          // Redirect to premium page based on selected plan
          window.location.href = `#premium-${packageId}`;
        },
        prefill: {
          name: 'User Name',
          email: 'user@example.com',
          contact: '+91XXXXXXXXXX'
        },
        theme: {
          color: '#ff6b35'
        }
      };

      const razorpay = new (window as any).Razorpay(options);
      razorpay.open();
    };
  };


  return (
    <div className="package-selection-overlay">
      {/* Background with sci-fi image */}
      <div className="package-background"></div>
      
      {/* Header */}
      <div className="package-header">
        <button className="back-button" onClick={onClose}>
          ← Back
        </button>
        
        {/* View Toggle */}
        <div className="view-toggle">
          <button 
            className={`toggle-btn ${currentView === 'plans' ? 'active' : ''}`}
            onClick={() => setCurrentView('plans')}
          >
            Select Your Research Plan
          </button>
          <button 
            className={`toggle-btn ${currentView === 'account' ? 'active' : ''}`}
            onClick={() => setCurrentView('account')}
          >
            My Account
          </button>
        </div>

        <h1 className="package-title">
          {currentView === 'plans' ? 'Select Your Research Plan' : 'My Account'}
        </h1>
        <p className="package-subtitle">
          {currentView === 'plans' 
            ? 'Choose the plan that best fits your research needs' 
            : 'Manage your subscription and usage'}
        </p>
      </div>

      {/* Main Content */}
      <div className="main-content">
        {currentView === 'plans' ? (
          /* Package Cards */
          <div className="package-grid">
            {packages.map((pkg) => (
              <div 
                key={pkg.id} 
                className={`package-card ${pkg.popular ? 'popular' : ''}`}
                onClick={() => handlePackageSelect(pkg.id)}
              >
                {pkg.popular && (
                  <div className="popular-badge">Most Popular</div>
                )}
                
                <div className="package-header">
                  <h3 className="package-name">{pkg.name}</h3>
                  <div className="package-price">{pkg.price}</div>
                </div>
                
                <p className="package-description">{pkg.description}</p>
                
                <ul className="package-features">
                  {pkg.features.map((feature, index) => (
                    <li key={index} className="feature-item">
                      <span className="checkmark">✓</span>
                      {feature}
                    </li>
                  ))}
                </ul>
                
                <button className="choose-package-btn">
                  Choose Package
                </button>
              </div>
            ))}
          </div>
        ) : (
          /* Account View */
          <div className="account-view">
            <div className="account-content-main">
              <div className="current-package">
                <h4>Current Package</h4>
                <div className="package-info">
                  <span className="package-name">{userAccount.package}</span>
                  <span className="valid-until">Valid until: {userAccount.validUntil}</span>
                </div>
              </div>

              <div className="usage-stats">
                <h4>Usage Statistics</h4>
                <div className="stat-item">
                  <span className="stat-label">Research Gap Finder</span>
                  <div className="stat-bar">
                    <div 
                      className="stat-fill" 
                      style={{ width: `${(userAccount.gapFinderUses / 5) * 100}%` }}
                    ></div>
                  </div>
                  <span className="stat-value">{userAccount.gapFinderUses}/5 uses left</span>
                </div>
                
                {userAccount.deepAnalysisUses > 0 && (
                  <div className="stat-item">
                    <span className="stat-label">Deep Paper Analysis</span>
                    <div className="stat-bar">
                      <div 
                        className="stat-fill" 
                        style={{ width: `${(userAccount.deepAnalysisUses / 5) * 100}%` }}
                      ></div>
                    </div>
                    <span className="stat-value">{userAccount.deepAnalysisUses}/5 uses left</span>
                  </div>
                )}

                {userAccount.teamSupportSessions > 0 && (
                  <div className="stat-item">
                    <span className="stat-label">Team Support</span>
                    <div className="stat-bar">
                      <div 
                        className="stat-fill" 
                        style={{ width: `${(userAccount.teamSupportSessions / 1) * 100}%` }}
                      ></div>
                    </div>
                    <span className="stat-value">{userAccount.teamSupportSessions}/1 sessions left</span>
                  </div>
                )}
              </div>

              <div className="account-actions">
                <button className="upgrade-btn" onClick={() => setCurrentView('plans')}>
                  Upgrade Package
                </button>
                <button className="support-btn">
                  Contact Support
                </button>
              </div>
            </div>
          </div>
        )}
      </div>

    </div>
  );
};

export default PackageSelection;
