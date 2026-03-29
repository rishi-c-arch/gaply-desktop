import React from 'react';
import { useTheme } from '../contexts/ThemeContext';
import { Link } from 'react-router-dom';
import './SupportPage.css';

const WHATSAPP_NUMBER = '6387144799';
const WHATSAPP_LINK = `https://wa.me/91${WHATSAPP_NUMBER}`;
const EMAIL = 'helloresearcher@gaply.in';

const FAQ_ITEMS = [
  {
    q: 'How do I get help with my thesis or research paper?',
    a: 'Reach us on WhatsApp for instant support. Our research experts are available to guide you through thesis writing, journal matching, AI detection, and more.',
  },
  {
    q: 'What are your response times?',
    a: 'We typically respond within a few hours during business hours. For urgent queries, WhatsApp is the fastest way to connect with our team.',
  },
  {
    q: 'Can I get help with PublishReady or DataMaestro?',
    a: 'Yes! Our support team can assist with all premium features including PublishReady, DataMaestro Pro, Journal Verification, and Research Deep Analysis.',
  },
  {
    q: 'Is there a fee for support?',
    a: 'General support is free. For specialized consultation or custom research assistance, we can discuss options when you reach out.',
  },
];

const SupportPage: React.FC = () => {
  const { theme } = useTheme();

  return (
    <div className="support-page" data-theme={theme}>
      <div className="support-page__noise" aria-hidden="true" />
      <div className="support-page__bg" />

      {/* Hero */}
      <section className="support-page__hero">
        <div className="support-page__hero-content">
          <span className="support-page__badge">We&apos;re here to help</span>
          <h1 className="support-page__title">
            Support <span className="support-page__title-accent">Center</span>
          </h1>
          <p className="support-page__subtitle">
            Get instant help from our research experts. Connect via WhatsApp for the fastest response, or reach us by email.
          </p>
        </div>
      </section>

      {/* Primary CTA - WhatsApp */}
      <section className="support-page__content">
        <div className="support-page__primary-cta">
          <div className="support-page__whatsapp-card">
            <div className="support-page__whatsapp-glow" aria-hidden="true" />
            <div className="support-page__whatsapp-content">
              <div className="support-page__whatsapp-icon">
                <svg viewBox="0 0 24 24" fill="currentColor" width={48} height={48}>
                  <path d="M17.472 14.382c-.297-.149-1.758-.867-2.03-.967-.273-.099-.471-.148-.67.15-.197.297-.767.966-.94 1.164-.173.199-.347.223-.644.075-.297-.15-1.255-.463-2.39-1.475-.883-.788-1.48-1.761-1.653-2.059-.173-.297-.018-.458.13-.606.134-.133.298-.347.446-.52.149-.174.198-.298.298-.497.099-.198.05-.371-.025-.52-.075-.149-.669-1.612-.916-2.207-.242-.579-.487-.5-.669-.51-.173-.008-.371-.01-.57-.01-.198 0-.52.074-.792.372-.272.297-1.04 1.016-1.04 2.479 0 1.462 1.065 2.875 1.213 3.074.149.198 2.096 3.2 5.077 4.487.709.306 1.262.489 1.694.625.712.227 1.36.195 1.871.118.571-.085 1.758-.719 2.006-1.413.248-.694.248-1.289.173-1.413-.074-.124-.272-.198-.57-.347m-5.421 7.403h-.004a9.87 9.87 0 01-5.031-1.378l-.361-.214-3.741.982.998-3.648-.235-.374a9.86 9.86 0 01-1.51-5.26c.001-5.45 4.436-9.884 9.888-9.884 2.64 0 5.122 1.03 6.988 2.898a9.825 9.825 0 012.893 6.994c-.003 5.45-4.437 9.884-9.885 9.884m8.413-18.297A11.815 11.815 0 0012.05 0C5.495 0 .16 5.335.157 11.892c0 2.096.547 4.142 1.588 5.945L.057 24l6.305-1.654a11.882 11.882 0 005.683 1.448h.005c6.554 0 11.89-5.335 11.893-11.893a11.821 11.821 0 00-3.48-8.413z" />
                </svg>
              </div>
              <div className="support-page__whatsapp-text">
                <h2>Chat on WhatsApp</h2>
                <p>Fastest way to get help. Our team typically responds within minutes.</p>
                <span className="support-page__whatsapp-number">+91 {WHATSAPP_NUMBER}</span>
              </div>
              <a
                href={WHATSAPP_LINK}
                target="_blank"
                rel="noopener noreferrer"
                className="support-page__whatsapp-btn"
              >
                <svg viewBox="0 0 24 24" fill="currentColor" width={22} height={22}>
                  <path d="M17.472 14.382c-.297-.149-1.758-.867-2.03-.967-.273-.099-.471-.148-.67.15-.197.297-.767.966-.94 1.164-.173.199-.347.223-.644.075-.297-.15-1.255-.463-2.39-1.475-.883-.788-1.48-1.761-1.653-2.059-.173-.297-.018-.458.13-.606.134-.133.298-.347.446-.52.149-.174.198-.298.298-.497.099-.198.05-.371-.025-.52-.075-.149-.669-1.612-.916-2.207-.242-.579-.487-.5-.669-.51-.173-.008-.371-.01-.57-.01-.198 0-.52.074-.792.372-.272.297-1.04 1.016-1.04 2.479 0 1.462 1.065 2.875 1.213 3.074.149.198 2.096 3.2 5.077 4.487.709.306 1.262.489 1.694.625.712.227 1.36.195 1.871.118.571-.085 1.758-.719 2.006-1.413.248-.694.248-1.289.173-1.413-.074-.124-.272-.198-.57-.347m-5.421 7.403h-.004a9.87 9.87 0 01-5.031-1.378l-.361-.214-3.741.982.998-3.648-.235-.374a9.86 9.86 0 01-1.51-5.26c.001-5.45 4.436-9.884 9.888-9.884 2.64 0 5.122 1.03 6.988 2.898a9.825 9.825 0 012.893 6.994c-.003 5.45-4.437 9.884-9.885 9.884m8.413-18.297A11.815 11.815 0 0012.05 0C5.495 0 .16 5.335.157 11.892c0 2.096.547 4.142 1.588 5.945L.057 24l6.305-1.654a11.882 11.882 0 005.683 1.448h.005c6.554 0 11.89-5.335 11.893-11.893a11.821 11.821 0 00-3.48-8.413z" />
                </svg>
                Start conversation
              </a>
            </div>
          </div>
        </div>

        {/* Other channels */}
        <div className="support-page__channels">
          <div className="support-page__channel-card">
            <div className="support-page__channel-icon support-page__channel-icon--email">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width={24} height={24}>
                <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z" />
                <polyline points="22,6 12,13 2,6" />
              </svg>
            </div>
            <h3>Email Support</h3>
            <p>For detailed queries or documentation requests</p>
            <a href={`mailto:${EMAIL}`} className="support-page__channel-link">
              {EMAIL}
            </a>
          </div>
          <div className="support-page__channel-card">
            <div className="support-page__channel-icon support-page__channel-icon--docs">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width={24} height={24}>
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="16" x2="12" y2="12" />
                <line x1="12" y1="8" x2="12.01" y2="8" />
              </svg>
            </div>
            <h3>Help & Tutorials</h3>
            <p>Watch video demos and learn how to use Gaply</p>
            <Link to="/watch-demo" className="support-page__channel-link">
              Watch Demo
            </Link>
          </div>
        </div>

        {/* FAQ */}
        <div className="support-page__faq">
          <h2 className="support-page__faq-title">Frequently asked</h2>
          <div className="support-page__faq-list">
            {FAQ_ITEMS.map((item, i) => (
              <div key={i} className="support-page__faq-item">
                <h4>{item.q}</h4>
                <p>{item.a}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Footer CTA */}
      <section className="support-page__cta">
        <div className="support-page__cta-content">
          <h3>Still need help?</h3>
          <p>Our team is here for you. Reach out on WhatsApp for the fastest response.</p>
          <a href={WHATSAPP_LINK} target="_blank" rel="noopener noreferrer" className="support-page__cta-btn">
            <svg viewBox="0 0 24 24" fill="currentColor" width={20} height={20}>
              <path d="M17.472 14.382c-.297-.149-1.758-.867-2.03-.967-.273-.099-.471-.148-.67.15-.197.297-.767.966-.94 1.164-.173.199-.347.223-.644.075-.297-.15-1.255-.463-2.39-1.475-.883-.788-1.48-1.761-1.653-2.059-.173-.297-.018-.458.13-.606.134-.133.298-.347.446-.52.149-.174.198-.298.298-.497.099-.198.05-.371-.025-.52-.075-.149-.669-1.612-.916-2.207-.242-.579-.487-.5-.669-.51-.173-.008-.371-.01-.57-.01-.198 0-.52.074-.792.372-.272.297-1.04 1.016-1.04 2.479 0 1.462 1.065 2.875 1.213 3.074.149.198 2.096 3.2 5.077 4.487.709.306 1.262.489 1.694.625.712.227 1.36.195 1.871.118.571-.085 1.758-.719 2.006-1.413.248-.694.248-1.289.173-1.413-.074-.124-.272-.198-.57-.347m-5.421 7.403h-.004a9.87 9.87 0 01-5.031-1.378l-.361-.214-3.741.982.998-3.648-.235-.374a9.86 9.86 0 01-1.51-5.26c.001-5.45 4.436-9.884 9.888-9.884 2.64 0 5.122 1.03 6.988 2.898a9.825 9.825 0 012.893 6.994c-.003 5.45-4.437 9.884-9.885 9.884m8.413-18.297A11.815 11.815 0 0012.05 0C5.495 0 .16 5.335.157 11.892c0 2.096.547 4.142 1.588 5.945L.057 24l6.305-1.654a11.882 11.882 0 005.683 1.448h.005c6.554 0 11.89-5.335 11.893-11.893a11.821 11.821 0 00-3.48-8.413z" />
            </svg>
            Chat on WhatsApp
          </a>
        </div>
      </section>
    </div>
  );
};

export default SupportPage;
