import React from 'react';

// Fallback CSS-only version
const CSSPremiumText: React.FC = () => {
  return (
    <div className="css-premium-text">
      <div className="main-title">
        <span className="title-line line1">AI AGENT WHICH ACCELERATE</span>
        <span className="title-line line2">YOUR ACADEMIC JOURNEY</span>
      </div>
      <div className="subtitle">
        This is the platform that makes genius look effortless
      </div>
    </div>
  );
};

const PremiumHeroText: React.FC = () => {
  // Always use CSS fallback for clean, reliable styling
  return <CSSPremiumText />;
};

export default PremiumHeroText;




