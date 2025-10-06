import React from 'react';
import globeImage from '../assets/dreamstime_xxl_126606510.jpg';

const ThreeJSGlobe: React.FC = () => {
  return (
    <div style={{ 
      position: 'absolute',
      right: '5%',
      top: '50%',
      transform: 'translateY(-50%)',
      width: '500px',
      height: '500px',
      zIndex: 10,
      borderRadius: '50%',
      overflow: 'hidden',
      boxShadow: '0 25px 60px rgba(0, 0, 0, 0.8), 0 10px 30px rgba(255, 122, 26, 0.3)',
      background: `url(${globeImage})`,
      backgroundSize: 'cover',
      backgroundPosition: 'center',
      backgroundRepeat: 'no-repeat',
      border: '2px solid rgba(255, 122, 26, 0.3)',
      animation: 'globeRotate 20s linear infinite'
    }}>
      {/* Simple rotating globe using CSS */}
    </div>
  );
};

export default ThreeJSGlobe;
