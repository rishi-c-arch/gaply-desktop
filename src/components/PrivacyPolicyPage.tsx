import React from 'react';
import { useNavigate } from 'react-router-dom';

const PrivacyPolicyPage: React.FC = () => {
  const navigate = useNavigate();

  return (
    <div style={{
      minHeight: '100vh',
      background: 'var(--app-bg)',
      color: 'var(--app-text)',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif'
    }}>
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        zIndex: 1000,
        background: 'var(--header-bg)',
        backdropFilter: 'blur(20px)',
        WebkitBackdropFilter: 'blur(20px)',
        borderBottom: '1px solid var(--header-border)',
        padding: '12px 0'
      }}>
        <div style={{
          maxWidth: 1200,
          margin: '0 auto',
          padding: '0 clamp(16px, 4vw, 40px)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between'
        }}>
          <div style={{ fontWeight: 800, letterSpacing: '.02em' }}>
            <h1 style={{
              margin: 0,
              fontSize: 18,
              color: 'var(--header-text)',
              fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif'
            }}>
              Gaply
            </h1>
          </div>
          <button
            onClick={() => navigate('/')}
            style={{
              background: 'var(--toggle-bg)',
              color: 'var(--app-text)',
              padding: '10px 20px',
              borderRadius: '12px',
              border: '1px solid var(--toggle-border)',
              cursor: 'pointer',
              fontSize: '1rem',
              fontWeight: '500',
              transition: 'all 0.3s ease',
              backdropFilter: 'blur(10px)',
              WebkitBackdropFilter: 'blur(10px)'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'var(--button-bg-hover)';
              e.currentTarget.style.borderColor = 'var(--button-border-hover)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'var(--toggle-bg)';
              e.currentTarget.style.borderColor = 'var(--toggle-border)';
            }}
          >
            Back to Home
          </button>
        </div>
      </div>

      <main style={{
        paddingTop: 'clamp(110px, 12vw, 140px)',
        paddingBottom: 'clamp(56px, 8vw, 96px)',
        paddingLeft: 'clamp(16px, 4vw, 40px)',
        paddingRight: 'clamp(16px, 4vw, 40px)'
      }}>
        <div style={{ maxWidth: 960, margin: '0 auto' }}>
          <div style={{
            textAlign: 'center',
            marginBottom: 'clamp(28px, 6vw, 48px)'
          }}>
            <h2 style={{
              fontSize: 'clamp(2.4rem, 6vw, 4rem)',
              fontWeight: 700,
              marginBottom: '16px',
              letterSpacing: '-0.02em'
            }}>
              Privacy Policy
            </h2>
            <p style={{
              fontSize: 'clamp(1.05rem, 2.5vw, 1.35rem)',
              color: 'var(--muted-text)',
              lineHeight: 1.6,
              margin: 0
            }}>
              We respect your privacy and keep your data in your control.
            </p>
          </div>

          <section style={{
            background: 'var(--card-bg)',
            border: '1px solid var(--card-border)',
            borderRadius: 20,
            padding: 'clamp(20px, 4vw, 32px)',
            boxShadow: 'var(--card-shadow)',
            marginBottom: 'clamp(20px, 4vw, 32px)'
          }}>
            <p style={{
              fontSize: 'clamp(1rem, 2.4vw, 1.15rem)',
              color: 'var(--section-text)',
              lineHeight: 1.7,
              margin: 0
            }}>
              We respect your privacy. Gaply.in processes your files to deliver analysis — but we do not keep
              uploaded papers or text by default. Files are used temporarily for processing and deleted
              automatically unless you explicitly choose to save them.
            </p>
          </section>

          <section style={{
            background: 'var(--card-bg)',
            border: '1px solid var(--card-border)',
            borderRadius: 20,
            padding: 'clamp(20px, 4vw, 32px)',
            boxShadow: 'var(--card-shadow)'
          }}>
            <h3 style={{
              fontSize: 'clamp(1.4rem, 3vw, 1.8rem)',
              marginBottom: '18px'
            }}>
              Key points
            </h3>
            <ul style={{
              listStyle: 'none',
              padding: 0,
              margin: 0,
              display: 'grid',
              gap: '12px'
            }}>
              {[
                {
                  title: 'No permanent storage by default',
                  text: 'Uploaded manuscripts, datasets, and text are processed and removed after results are delivered (short automated retention window).'
                },
                {
                  title: 'Optional saving',
                  text: 'If you opt into “Save for later” or a workspace, your files are stored until you delete them.'
                },
                {
                  title: 'Temporary troubleshooting',
                  text: 'With your permission we may keep a copy briefly to fix issues.'
                },
                {
                  title: 'Third-party services',
                  text: 'We may call external services (AI providers, DOI/metadata lookups, optional plagiarism services) - only the data required for those services is shared.'
                },
                {
                  title: 'Security',
                  text: 'Data is transmitted over HTTPS and stored with industry-standard protections when saved. Enterprise options (India-region or on-premise) are available.'
                },
                {
                  title: 'Your rights',
                  text: 'You can delete saved files, export your reports, opt out of analytics, and request data deletion.'
                }
              ].map((item) => (
                <li key={item.title} style={{
                  padding: '14px 16px',
                  borderRadius: 14,
                  border: '1px solid var(--divider)',
                  background: 'var(--content-bg)'
                }}>
                  <div style={{
                    fontWeight: 600,
                    marginBottom: 6
                  }}>
                    {item.title}
                  </div>
                  <div style={{
                    color: 'var(--muted-text)',
                    lineHeight: 1.6
                  }}>
                    {item.text}
                  </div>
                </li>
              ))}
            </ul>
          </section>

          <section style={{
            marginTop: 'clamp(24px, 6vw, 40px)',
            textAlign: 'center'
          }}>
            <p style={{
              fontSize: 'clamp(1rem, 2.4vw, 1.15rem)',
              color: 'var(--muted-text)',
              marginBottom: 10
            }}>
              Contact: <strong>helloresearcher@gaply.in</strong>
            </p>
          </section>
        </div>
      </main>
    </div>
  );
};

export default PrivacyPolicyPage;
