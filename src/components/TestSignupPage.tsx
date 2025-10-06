import React from 'react';

const TestSignupPage: React.FC = () => {
  return (
    <div style={{
      position: 'relative',
      width: '100%',
      height: '100vh',
      overflow: 'hidden',
      background: 'linear-gradient(135deg, #0A0A0A 0%, #1a1a1a 30%, #2a2a2a 70%, #0A0A0A 100%)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      minHeight: '100vh',
      color: '#ffffff',
      fontSize: '2rem',
      fontWeight: 'bold'
    }}>
      <div style={{
        textAlign: 'center',
        background: 'rgba(255, 255, 255, 0.1)',
        padding: '50px',
        borderRadius: '20px',
        border: '2px solid #ff7a1a'
      }}>
        <h1 style={{
          color: '#ff7a1a',
          fontSize: '4rem',
          marginBottom: '20px'
        }}>
          TEST SIGNUP PAGE
        </h1>
        <p style={{
          fontSize: '1.5rem',
          color: '#ffffff'
        }}>
          If you can see this, the routing is working!
        </p>
        <p style={{
          fontSize: '1rem',
          color: '#cecece',
          marginTop: '20px'
        }}>
          This is a test to verify the signup page loads correctly.
        </p>
      </div>
    </div>
  );
};

export default TestSignupPage;
