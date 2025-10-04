import React from 'react';
import './LegalPopup.css';

interface LegalPopupProps {
  type: 'terms' | 'privacy';
  onClose: () => void;
}

const LegalPopup: React.FC<LegalPopupProps> = ({ type, onClose }) => {
  const isTerms = type === 'terms';
  const title = isTerms ? 'Terms of Service' : 'Privacy Policy';
  
  const termsContent = `
    Last updated: October 2024

    Welcome to GAPLY

    These Terms of Service ("Terms") govern your use of our website located at gaply.in (the "Service") operated by GAPLY ("us", "we", or "our").

    By accessing or using our Service, you agree to be bound by these Terms. If you disagree with any part of these terms, then you may not access the Service.

    Accounts

    When you create an account with us, you must provide information that is accurate, complete, and current at all times. You are responsible for safeguarding the password and for maintaining the confidentiality of your account.

    Intellectual Property

    The Service and its original content, features, and functionality are and will remain the exclusive property of GAPLY and its licensors. The Service is protected by copyright, trademark, and other laws.

    Prohibited Uses

    You may not use our Service:
    • For any unlawful purpose or to solicit others to perform unlawful acts
    • To violate any international, federal, provincial, or state regulations, rules, laws, or local ordinances
    • To infringe upon or violate our intellectual property rights or the intellectual property rights of others
    • To harass, abuse, insult, harm, defame, slander, disparage, intimidate, or discriminate

    Termination

    We may terminate or suspend your account immediately, without prior notice or liability, for any reason whatsoever, including without limitation if you breach the Terms.

    Contact Information

    If you have any questions about these Terms of Service, please contact us at helloresearcher@gaply.in
  `;

  const privacyContent = `
    Last updated: October 2024

    Privacy Policy for GAPLY

    At GAPLY, accessible from gaply.in, one of our main priorities is the privacy of our visitors. This Privacy Policy document contains types of information that is collected and recorded by GAPLY and how we use it.

    Information We Collect

    We collect information you provide directly to us, such as when you:
    • Create an account
    • Use our services
    • Contact us for support
    • Subscribe to our newsletter

    The types of information we may collect include:
    • Name and email address
    • Institution or organization
    • Research interests and preferences
    • Usage data and analytics

    How We Use Your Information

    We use the information we collect to:
    • Provide, maintain, and improve our services
    • Process transactions and send related information
    • Send technical notices, updates, and support messages
    • Respond to your comments and questions
    • Personalize your experience

    Information Sharing

    We do not sell, trade, or otherwise transfer your personal information to third parties without your consent, except as described in this policy. We may share your information in the following circumstances:
    • With your consent
    • To comply with legal obligations
    • To protect our rights and safety
    • In connection with a business transfer

    Data Security

    We implement appropriate technical and organizational measures to protect your personal information against unauthorized access, alteration, disclosure, or destruction.

    Cookies

    We use cookies and similar tracking technologies to track activity on our Service and hold certain information. You can instruct your browser to refuse all cookies or to indicate when a cookie is being sent.

    Your Rights

    You have the right to:
    • Access your personal information
    • Correct inaccurate information
    • Delete your information
    • Object to processing
    • Data portability

    Contact Us

    If you have any questions about this Privacy Policy, please contact us at helloresearcher@gaply.in
  `;

  const content = isTerms ? termsContent : privacyContent;

  return (
    <div className="legal-overlay">
      <div className="legal-container">
        <div className="legal-header">
          <h2>{title}</h2>
          <button className="legal-close-button" onClick={onClose}>
            ×
          </button>
        </div>
        
        <div className="legal-content">
          <pre className="legal-text">{content}</pre>
        </div>
        
        <div className="legal-footer">
          <button className="legal-close-btn" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
};

export default LegalPopup;
