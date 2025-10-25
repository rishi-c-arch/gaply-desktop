import React, { useState } from 'react';
import ExpertSearch from './ExpertSearch';
import HireRequestDialog from './HireRequestDialog';
import prakharImage from '../assets/images/P.jpeg';
import anuragImage from '../assets/images/anu.png';
import piyushImage from '../assets/images/ch.png';

interface ExpertProfile {
  id: number;
  name: string;
  email: string;
  domains: string[];
  experience: number;
  publications: number;
  patents: number;
  bio: string;
  image_url: string;
  is_available: boolean;
  created_at: string;
}

const HireExpertPage: React.FC = () => {
  const [selectedExpert, setSelectedExpert] = useState<ExpertProfile | null>(null);
  const [isDialogOpen, setIsDialogOpen] = useState(false);

  const handleHireExpert = (expert: ExpertProfile) => {
    setSelectedExpert(expert);
    setIsDialogOpen(true);
  };

  const handleCloseDialog = () => {
    setIsDialogOpen(false);
    setSelectedExpert(null);
  };

  return (
    <div style={{
      minHeight: '100vh',
      background: 'linear-gradient(135deg, #000000 0%, #0A0A0A 50%, #000000 100%)',
      color: '#ffffff',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'flex-start',
      padding: '0 20px',
      position: 'relative',
      overflow: 'auto',
      overflowY: 'scroll',
      height: '100vh'
    }}>
      {/* Background Elements */}
      <div style={{
        position: 'absolute',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'radial-gradient(circle at 30% 20%, rgba(0, 122, 255, 0.1) 0%, transparent 50%), radial-gradient(circle at 70% 80%, rgba(88, 86, 214, 0.1) 0%, transparent 50%)',
        pointerEvents: 'none'
      }} />
      
      {/* Floating Particles */}
      <div style={{
        position: 'absolute',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        pointerEvents: 'none'
      }}>
        {Array.from({ length: 20 }).map((_, i) => (
          <div
            key={i}
            style={{
              position: 'absolute',
              width: '2px',
              height: '2px',
              background: 'rgba(255, 255, 255, 0.3)',
              borderRadius: '50%',
              left: `${Math.random() * 100}%`,
              top: `${Math.random() * 100}%`,
              animation: `float ${3 + Math.random() * 4}s ease-in-out infinite`,
              animationDelay: `${Math.random() * 2}s`
            }}
          />
        ))}
      </div>

             <div style={{
               maxWidth: '900px',
               width: '100%',
               textAlign: 'center',
               position: 'relative',
               zIndex: 2,
               paddingTop: '0px',
               marginTop: '0px'
             }}>
               {/* Main Title */}
               <div style={{ marginBottom: '40px' }}>
                 <h1 style={{
                   fontSize: 'clamp(2rem, 4vw, 2.5rem)',
                   fontWeight: '600',
                   marginBottom: '16px',
                   color: '#ffffff',
                   letterSpacing: '-0.02em',
                   lineHeight: '1.2',
                   margin: '0 0 16px 0'
                 }}>
                   Hire an Expert
                 </h1>
                 <p style={{
                   fontSize: 'clamp(0.9rem, 2vw, 1.1rem)',
                   color: 'rgba(255, 255, 255, 0.6)',
                   lineHeight: '1.4',
                   fontWeight: '400',
                   letterSpacing: '-0.01em',
                   maxWidth: '500px',
                   margin: '0 auto'
                 }}>
                   Find the perfect academic research expert for your project.
                 </p>
               </div>

               {/* Expert Search Component */}
               <div style={{
                 marginTop: '0px',
                 marginBottom: '60px',
                 maxWidth: '1000px',
                 width: '100%',
                 position: 'relative'
               }}>
                 <ExpertSearch 
                   onExpertSelect={(expert) => {
                     // Handle expert selection - could open contact modal or redirect
                     console.log('Selected expert:', expert);
                     // You can implement contact functionality here
                   }}
                   onDomainSelect={(domain) => {
                     // Handle domain selection - could filter experts or show domain info
                     console.log('Selected domain:', domain);
                   }}
                 />
               </div>

               {/* Expert Portfolios Section */}
               <div style={{
                 marginTop: '80px',
                 padding: '0px 20px 60px 20px',
                 color: 'white',
                 fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif',
                 width: '100%',
                 maxWidth: '1200px',
                 boxSizing: 'border-box'
               }}>

                 {/* Cards Container */}
                 <div style={{ 
                   maxWidth: '1200px', 
                   margin: '0 auto',
                   display: 'flex',
                   justifyContent: 'center',
                   alignItems: 'center',
                   gap: '30px',
                   perspective: '1200px',
                   transformStyle: 'preserve-3d'
                 }}>
                   {/* Prakhar Singh - Center */}
                   <div style={{
                     backgroundColor: '#000000',
                     borderRadius: '20px',
                     overflow: 'hidden',
                     border: '1px solid rgba(255, 255, 255, 0.12)',
                     cursor: 'pointer',
                     transition: 'all 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                     transform: 'translateZ(0px) scale(1)',
                     boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)',
                     position: 'relative',
                     width: '300px',
                     maxWidth: '300px',
                     opacity: '1',
                     zIndex: 2
                   }}
                   onMouseEnter={(e) => {
                     e.currentTarget.style.transform = 'translateZ(20px) scale(1.05)';
                     e.currentTarget.style.boxShadow = '0 20px 60px rgba(255, 255, 255, 0.2), 0 0 0 1px rgba(255, 255, 255, 0.3)';
                     e.currentTarget.style.opacity = '1';
                     e.currentTarget.style.zIndex = '10';
                   }}
                   onMouseLeave={(e) => {
                     e.currentTarget.style.transform = 'translateZ(0px) scale(1)';
                     e.currentTarget.style.boxShadow = '0 8px 32px rgba(0, 0, 0, 0.3)';
                     e.currentTarget.style.opacity = '1';
                     e.currentTarget.style.zIndex = '2';
                   }}
                   >
                     {/* Photo Section */}
                     <div style={{ 
                       position: 'relative', 
                       height: '200px',
                       overflow: 'hidden',
                       background: 'linear-gradient(135deg, #1a1a1a 0%, #000000 100%)',
                       display: 'flex',
                       alignItems: 'center',
                       justifyContent: 'center'
                     }}>
                      <img 
                        src={prakharImage} 
                        alt="Prakhar Singh"
                         style={{
                           width: '120px',
                           height: '120px',
                           borderRadius: '50%',
                           objectFit: 'cover',
                           border: '3px solid rgba(255, 255, 255, 0.2)',
                           boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4)'
                         }}
                         onError={(e) => {
                           e.currentTarget.style.display = 'none';
                           const nextElement = e.currentTarget.nextElementSibling as HTMLElement;
                           if (nextElement) {
                             nextElement.style.display = 'flex';
                           }
                         }}
                       />
                       <div style={{
                         width: '120px',
                         height: '120px',
                         borderRadius: '50%',
                         background: 'rgba(255, 255, 255, 0.1)',
                         display: 'none',
                         alignItems: 'center',
                         justifyContent: 'center',
                         fontSize: '28px',
                         fontWeight: '700',
                         color: '#ffffff',
                         border: '3px solid rgba(255, 255, 255, 0.2)',
                         boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4)'
                       }}>
                         PS
                       </div>
                       
                       {/* Expert Badge */}
                       <div style={{
                         position: 'absolute',
                         top: '16px',
                         right: '16px',
                         backgroundColor: 'rgba(255, 255, 255, 0.15)',
                         color: 'white',
                         padding: '6px 12px',
                         borderRadius: '12px',
                         fontSize: '10px',
                         fontWeight: '600',
                         letterSpacing: '0.5px',
                         backdropFilter: 'blur(20px)',
                         border: '1px solid rgba(255, 255, 255, 0.3)',
                         textTransform: 'uppercase'
                       }}>
                         Expert
                       </div>
                     </div>

                     {/* Content Section */}
                     <div style={{ 
                       padding: '24px',
                       background: 'linear-gradient(135deg, #0a0a0a 0%, #000000 100%)',
                       borderTop: '1px solid rgba(255, 255, 255, 0.08)'
                     }}>
                       <h3 style={{ 
                         fontSize: '1.4rem',
                         fontWeight: '600', 
                         marginBottom: '12px',
                         color: '#ffffff',
                         letterSpacing: '-0.01em',
                         lineHeight: '1.3'
                       }}>
                         Prakhar Singh
                       </h3>
                       
                       <p style={{ 
                         color: 'rgba(255, 255, 255, 0.8)', 
                         marginBottom: '20px',
                         fontSize: '0.95rem',
                         lineHeight: '1.5',
                         fontWeight: '300',
                         letterSpacing: '0.01em'
                       }}>
                         Finance & Research Expert. PhD Finance | 5 yrs | 5 papers.
                       </p>

                      <button style={{
                        width: '100%',
                        backgroundColor: 'transparent',
                        color: '#ffffff',
                        border: '1px solid rgba(255, 255, 255, 0.3)',
                        padding: '14px 24px',
                        borderRadius: '12px',
                        fontSize: '12px',
                        fontWeight: '600',
                        letterSpacing: '0.5px',
                        cursor: 'pointer',
                        transition: 'all 0.6s ease',
                        textTransform: 'uppercase',
                        position: 'relative',
                        overflow: 'hidden'
                      }}
                      onClick={() => handleHireExpert({
                        id: 1,
                        name: 'Prakhar Singh',
                        email: 'prakhar@example.com',
                        domains: ['Finance & Research Expert', 'Econometrician'],
                        experience: 5,
                        publications: 5,
                        patents: 0,
                        bio: 'Finance & Research Expert. PhD Finance | 5 yrs | 5 papers.',
                        image_url: prakharImage,
                        is_available: true,
                        created_at: new Date().toISOString()
                      })}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.backgroundColor = 'rgba(255, 255, 255, 0.1)';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.6)';
                        e.currentTarget.style.transform = 'translateY(-2px)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.backgroundColor = 'transparent';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.3)';
                        e.currentTarget.style.transform = 'translateY(0)';
                      }}
                      >
                        <span style={{ position: 'relative', zIndex: 1 }}>Hire Prakhar</span>
                      </button>
                     </div>
            </div>

                   {/* Anurag Singh - Right */}
                   <div style={{
                     backgroundColor: '#000000',
                     borderRadius: '20px',
                     overflow: 'hidden',
                     border: '1px solid rgba(255, 255, 255, 0.12)',
                     cursor: 'pointer',
                     transition: 'all 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                     transform: 'translateZ(0px) scale(1)',
                     boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)',
                     position: 'relative',
                     width: '300px',
                     maxWidth: '300px',
                     opacity: '1',
                     zIndex: 2
                   }}
                   onMouseEnter={(e) => {
                     e.currentTarget.style.transform = 'translateZ(20px) scale(1.05)';
                     e.currentTarget.style.boxShadow = '0 20px 60px rgba(255, 255, 255, 0.2), 0 0 0 1px rgba(255, 255, 255, 0.3)';
                     e.currentTarget.style.opacity = '1';
                     e.currentTarget.style.zIndex = '10';
                   }}
                   onMouseLeave={(e) => {
                     e.currentTarget.style.transform = 'translateZ(0px) scale(1)';
                     e.currentTarget.style.boxShadow = '0 8px 32px rgba(0, 0, 0, 0.3)';
                     e.currentTarget.style.opacity = '1';
                     e.currentTarget.style.zIndex = '2';
                   }}
                   >
                     {/* Photo Section */}
                     <div style={{ 
                       position: 'relative', 
                       height: '200px',
                       overflow: 'hidden',
                       background: 'linear-gradient(135deg, #1a1a1a 0%, #000000 100%)',
                       display: 'flex',
                       alignItems: 'center',
                       justifyContent: 'center'
                     }}>
                      <img 
                        src={anuragImage} 
                        alt="Anurag Singh"
                         style={{
                           width: '120px',
                           height: '120px',
                           borderRadius: '50%',
                           objectFit: 'cover',
                           border: '3px solid rgba(255, 255, 255, 0.2)',
                           boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4)'
                         }}
                         onError={(e) => {
                           e.currentTarget.style.display = 'none';
                           const nextElement = e.currentTarget.nextElementSibling as HTMLElement;
                           if (nextElement) {
                             nextElement.style.display = 'flex';
                           }
                         }}
                       />
                       <div style={{
                         width: '120px',
                         height: '120px',
                         borderRadius: '50%',
                         background: 'rgba(255, 255, 255, 0.1)',
                         display: 'none',
                         alignItems: 'center',
                         justifyContent: 'center',
                         fontSize: '28px',
                         fontWeight: '700',
                         color: '#ffffff',
                         border: '3px solid rgba(255, 255, 255, 0.2)',
                         boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4)'
                       }}>
                         AS
                       </div>
                       
                       {/* Expert Badge */}
                       <div style={{
                         position: 'absolute',
                         top: '16px',
                         right: '16px',
                         backgroundColor: 'rgba(255, 255, 255, 0.15)',
                         color: 'white',
                         padding: '6px 12px',
                         borderRadius: '12px',
                         fontSize: '10px',
                         fontWeight: '600',
                         letterSpacing: '0.5px',
                         backdropFilter: 'blur(20px)',
                         border: '1px solid rgba(255, 255, 255, 0.3)',
                         textTransform: 'uppercase'
                       }}>
                         Expert
                       </div>
                     </div>

                     {/* Content Section */}
                     <div style={{ 
                       padding: '24px',
                       background: 'linear-gradient(135deg, #0a0a0a 0%, #000000 100%)',
                       borderTop: '1px solid rgba(255, 255, 255, 0.08)'
                     }}>
                       <h3 style={{ 
                         fontSize: '1.4rem',
                         fontWeight: '600', 
                         marginBottom: '12px',
                         color: '#ffffff',
                         letterSpacing: '-0.01em',
                         lineHeight: '1.3'
                       }}>
                         Anurag Singh
                       </h3>
                       
                       <p style={{ 
                         color: 'rgba(255, 255, 255, 0.8)', 
                         marginBottom: '20px',
                         fontSize: '0.95rem',
                         lineHeight: '1.5',
                         fontWeight: '300',
                         letterSpacing: '0.01em'
                       }}>
                         Computer Scientist. PhD CS | 10 yrs | 2 papers | 1 patent.
                       </p>

                      <button style={{
                        width: '100%',
                        backgroundColor: 'transparent',
                        color: '#ffffff',
                        border: '1px solid rgba(255, 255, 255, 0.3)',
                        padding: '14px 24px',
                        borderRadius: '12px',
                        fontSize: '12px',
                        fontWeight: '600',
                        letterSpacing: '0.5px',
                        cursor: 'pointer',
                        transition: 'all 0.6s ease',
                        textTransform: 'uppercase',
                        position: 'relative',
                        overflow: 'hidden'
                      }}
                      onClick={() => handleHireExpert({
                        id: 2,
                        name: 'Anurag Singh',
                        email: 'anurag@example.com',
                        domains: ['Computer Scientist', 'Machine Learning Engineer'],
                        experience: 10,
                        publications: 2,
                        patents: 1,
                        bio: 'Computer Scientist. PhD CS | 10 yrs | 2 papers | 1 patent.',
                        image_url: anuragImage,
                        is_available: true,
                        created_at: new Date().toISOString()
                      })}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.backgroundColor = 'rgba(255, 255, 255, 0.1)';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.6)';
                        e.currentTarget.style.transform = 'translateY(-2px)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.backgroundColor = 'transparent';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.3)';
                        e.currentTarget.style.transform = 'translateY(0)';
                      }}
                      >
                        <span style={{ position: 'relative', zIndex: 1 }}>Hire Anurag</span>
                      </button>
                     </div>
            </div>

                   {/* Piyush Anand - Left */}
                   <div style={{
                     backgroundColor: '#000000',
                     borderRadius: '20px',
                     overflow: 'hidden',
                     border: '1px solid rgba(255, 255, 255, 0.12)',
                     cursor: 'pointer',
                     transition: 'all 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                     transform: 'translateZ(0px) scale(1)',
                     boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)',
                     position: 'relative',
                     width: '300px',
                     maxWidth: '300px',
                     opacity: '1',
                     zIndex: 2
                   }}
                   onMouseEnter={(e) => {
                     e.currentTarget.style.transform = 'translateZ(20px) scale(1.05)';
                     e.currentTarget.style.boxShadow = '0 20px 60px rgba(255, 255, 255, 0.2), 0 0 0 1px rgba(255, 255, 255, 0.3)';
                     e.currentTarget.style.opacity = '1';
                     e.currentTarget.style.zIndex = '10';
                   }}
                   onMouseLeave={(e) => {
                     e.currentTarget.style.transform = 'translateZ(0px) scale(1)';
                     e.currentTarget.style.boxShadow = '0 8px 32px rgba(0, 0, 0, 0.3)';
                     e.currentTarget.style.opacity = '1';
                     e.currentTarget.style.zIndex = '2';
                   }}
                   >
                     {/* Photo Section */}
                     <div style={{ 
                       position: 'relative', 
                       height: '200px',
                       overflow: 'hidden',
                       background: 'linear-gradient(135deg, #1a1a1a 0%, #000000 100%)',
                       display: 'flex',
                       alignItems: 'center',
                       justifyContent: 'center'
                     }}>
                      <img 
                        src={piyushImage} 
                        alt="Piyush Anand"
                         style={{
                           width: '120px',
                           height: '120px',
                           borderRadius: '50%',
                           objectFit: 'cover',
                           border: '3px solid rgba(255, 255, 255, 0.2)',
                           boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4)'
                         }}
                         onError={(e) => {
                           e.currentTarget.style.display = 'none';
                           const nextElement = e.currentTarget.nextElementSibling as HTMLElement;
                           if (nextElement) {
                             nextElement.style.display = 'flex';
                           }
                         }}
                       />
                       <div style={{
                         width: '120px',
                         height: '120px',
                         borderRadius: '50%',
                         background: 'rgba(255, 255, 255, 0.1)',
                         display: 'none',
                         alignItems: 'center',
                         justifyContent: 'center',
                         fontSize: '28px',
                         fontWeight: '700',
                         color: '#ffffff',
                         border: '3px solid rgba(255, 255, 255, 0.2)',
                         boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4)'
                       }}>
                         PA
                       </div>
                       
                       {/* Expert Badge */}
                       <div style={{
                         position: 'absolute',
                         top: '16px',
                         right: '16px',
                         backgroundColor: 'rgba(255, 255, 255, 0.15)',
                         color: 'white',
                         padding: '6px 12px',
                         borderRadius: '12px',
                         fontSize: '10px',
                         fontWeight: '600',
                         letterSpacing: '0.5px',
                         backdropFilter: 'blur(20px)',
                         border: '1px solid rgba(255, 255, 255, 0.3)',
                         textTransform: 'uppercase'
                       }}>
                         Expert
                       </div>
                     </div>

                     {/* Content Section */}
                     <div style={{ 
                       padding: '24px',
                       background: 'linear-gradient(135deg, #0a0a0a 0%, #000000 100%)',
                       borderTop: '1px solid rgba(255, 255, 255, 0.08)'
                     }}>
                       <h3 style={{ 
                         fontSize: '1.4rem',
                         fontWeight: '600', 
                         marginBottom: '12px',
                         color: '#ffffff',
                         letterSpacing: '-0.01em',
                         lineHeight: '1.3'
                       }}>
                         Piyush Anand
                       </h3>
                       
                       <p style={{ 
                         color: 'rgba(255, 255, 255, 0.8)', 
                         marginBottom: '20px',
                         fontSize: '0.95rem',
                         lineHeight: '1.5',
                         fontWeight: '300',
                         letterSpacing: '0.01em'
                       }}>
                         ML & XAI Expert. PhD ML | 10 yrs | 6 papers.
                       </p>

                      <button style={{
                        width: '100%',
                        backgroundColor: 'transparent',
                        color: '#ffffff',
                        border: '1px solid rgba(255, 255, 255, 0.3)',
                        padding: '14px 24px',
                        borderRadius: '12px',
                        fontSize: '12px',
                        fontWeight: '600',
                        letterSpacing: '0.5px',
                        cursor: 'pointer',
                        transition: 'all 0.6s ease',
                        textTransform: 'uppercase',
                        position: 'relative',
                        overflow: 'hidden'
                      }}
                      onClick={() => handleHireExpert({
                        id: 3,
                        name: 'Piyush Anand',
                        email: 'piyush@example.com',
                        domains: ['ML & XAI Expert', 'Data Analyst / EDA Specialist'],
                        experience: 10,
                        publications: 6,
                        patents: 0,
                        bio: 'ML & XAI Expert. PhD ML | 10 yrs | 6 papers.',
                        image_url: piyushImage,
                        is_available: true,
                        created_at: new Date().toISOString()
                      })}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.backgroundColor = 'rgba(255, 255, 255, 0.1)';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.6)';
                        e.currentTarget.style.transform = 'translateY(-2px)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.backgroundColor = 'transparent';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.3)';
                        e.currentTarget.style.transform = 'translateY(0)';
                      }}
                      >
                        <span style={{ position: 'relative', zIndex: 1 }}>Hire Piyush</span>
                      </button>
                     </div>
            </div>
          </div>
        </div>

      </div>

      {/* CSS for floating animation */}
      <style>{`
        @keyframes float {
          0%, 100% { transform: translateY(0px) rotate(0deg); }
          50% { transform: translateY(-20px) rotate(180deg); }
        }
      `}</style>

      {/* Hire Request Dialog */}
      {selectedExpert && (
        <HireRequestDialog
          expert={selectedExpert}
          isOpen={isDialogOpen}
          onClose={handleCloseDialog}
        />
      )}
    </div>
  );
};

export default HireExpertPage;
