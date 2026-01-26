import React from 'react';

const HeroToSecondTransition: React.FC = () => {
  return (
    <div style={{
      position: 'relative',
      height: '200px',
      background: 'var(--transition-bg)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      overflow: 'hidden'
    }}>
      {/* Blue Line */}
      <div style={{
        position: 'relative',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        width: '100%',
        height: '100%'
      }}>
        {/* Blue Line */}
        <div style={{
          position: 'absolute',
          top: '50%',
          left: '50%',
          transform: 'translate(-50%, -50%)',
          width: '80%',
          height: '2px',
          background: 'linear-gradient(90deg, transparent 0%, #007AFF 20%, #007AFF 80%, transparent 100%)',
          zIndex: 1
        }} />
      </div>
    </div>
  );
};

export default HeroToSecondTransition;
