import React from 'react';
import { useNavigate } from 'react-router-dom';

const TermsOfServicePage: React.FC = () => {
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
        <div style={{ maxWidth: 980, margin: '0 auto' }}>
          <div style={{
            textAlign: 'center',
            marginBottom: 'clamp(28px, 6vw, 48px)'
          }}>
            <h2 style={{
              fontSize: 'clamp(2.4rem, 6vw, 4rem)',
              fontWeight: 700,
              marginBottom: '12px',
              letterSpacing: '-0.02em'
            }}>
              Terms of Service
            </h2>
            <p style={{
              fontSize: 'clamp(1rem, 2.5vw, 1.2rem)',
              color: 'var(--muted-text)',
              margin: 0
            }}>
              Effective date: [Insert effective date]
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
              Welcome to Gaply.in (“Gaply”, “we”, “us”, or “our”). These Terms of Service (“Terms”) govern your access
              to and use of Gaply’s websites, applications, APIs and services (collectively, the “Services”). By creating
              an account, accessing, or using the Services, you agree to be bound by these Terms and our Privacy Policy.
              If you do not agree, do not use the Services.
            </p>
          </section>

          <section style={{
            background: 'var(--card-bg)',
            border: '1px solid var(--card-border)',
            borderRadius: 20,
            padding: 'clamp(20px, 4vw, 32px)',
            boxShadow: 'var(--card-shadow)'
          }}>
            <div style={{ display: 'grid', gap: '16px' }}>
              {[
                {
                  title: '1. Definitions',
                  text: [
                    '“User”, “you”, or “your” means any person or entity that accesses or uses the Services.',
                    '“Content” means any text, files, data, information, submissions, images, manuscripts, datasets, questionnaires, or other materials you upload to or otherwise provide through the Services.',
                    '“Account” means an account you create to use the Services.',
                    '“Privacy Policy” means our privacy policy, available at /privacy, which explains how we collect and use personal data.'
                  ]
                },
                {
                  title: '2. Acceptance of Terms',
                  text: [
                    'By using Gaply.in you represent and warrant that you have read, understood and agree to these Terms and the Privacy Policy.',
                    'If you are accepting these Terms on behalf of an organization, you represent that you have authority to bind that organization.'
                  ]
                },
                {
                  title: '3. Changes to Terms',
                  text: [
                    'We may modify these Terms from time to time.',
                    'If we make material changes, we will notify account holders via email or a notice on the website.',
                    'Continued use of the Services after changes indicates your acceptance of the revised Terms.',
                    'The “Effective date” above indicates when the current Terms took effect.'
                  ]
                },
                {
                  title: '4. The Services',
                  text: [
                    'Gaply provides AI-powered research tools including but not limited to PublishReady and DataMaestro for manuscript and data analysis, report generation, and interactive Q&A.',
                    'Features may include manuscript evaluation, statistical orchestration, plagiarism/similarity checking (via third-party services where enabled), questionnaire processing, report downloads, and chat with Gaply.AI.',
                    'The Services may be updated, modified, or discontinued at any time without liability.'
                  ]
                },
                {
                  title: '5. Accounts & Registration',
                  text: [
                    'Eligibility: You must be at least 18 years old and have capacity to form a binding contract.',
                    'Registration: To use certain Services you must create an Account and provide accurate information. You agree to maintain the accuracy of your account information.',
                    'Credentials: You are responsible for maintaining the confidentiality of your account credentials and for all activity under your account. Notify helloresearcher@gaply.in immediately of any unauthorized use.',
                    'Termination: Gaply may suspend or terminate your account for violations of these Terms or for any conduct Gaply deems harmful to the Service or other users.'
                  ]
                },
                {
                  title: '6. User Content; License to Gaply',
                  text: [
                    'Ownership: You retain all rights, title, and interest in and to the Content you upload.',
                    'Processing License: By uploading Content, you grant Gaply a limited, non-exclusive, royalty-free, worldwide license to use, copy, store, transmit and display the Content as reasonably necessary to provide the Services (including ephemeral processing and temporary caching). This license ends when the Content is deleted, except where you expressly opt in to persistent storage or where required for troubleshooting with your consent.',
                    'Responsibility for Content: You represent and warrant that you have all rights necessary to upload the Content and to grant the license above. You are solely responsible for your Content and for compliance with applicable laws, including privacy and copyright laws. Gaply is not responsible for user Content.'
                  ]
                },
                {
                  title: '7. File Handling, Retention & Deletion',
                  text: [
                    'Default behavior: As described in our Privacy Policy, Gaply does not permanently retain uploaded manuscripts, datasets, or text by default. Files are used temporarily to produce results and are deleted after an automated retention window unless you opt to save them.',
                    'Saved content: If you choose “Save for later” or enable workspace features, you authorize Gaply to store your Content until you delete it. You can delete saved Content from your account at any time.',
                    'Support retention: With your consent, Gaply may retain a copy temporarily to perform troubleshooting; Gaply will delete such copies once troubleshooting is complete unless you ask otherwise.',
                    'Backups: Gaply may maintain backups for operational reasons. We will take reasonable steps to delete backup copies consistent with industry practice following deletion requests.'
                  ]
                },
                {
                  title: '8. Acceptable Use',
                  text: [
                    'You agree not to use the Services to upload or share Content that violates law, third-party rights, or is defamatory, obscene, pornographic, abusive, or harassing.',
                    'You agree not to upload content you do not have the right to share (e.g., copyrighted material without permission).',
                    'You agree not to transmit malware, malicious code, or anything that interferes with the Services.',
                    'You agree not to attempt to reverse-engineer, exploit, or otherwise interrupt the Services.',
                    'You agree not to use the Services to engage in illegal activities.',
                    'We may remove or refuse content and suspend accounts that violate these rules.'
                  ]
                },
                {
                  title: '9. Paid Subscriptions, Billing & Refunds',
                  text: [
                    'Pricing: Certain features are available only via paid plans (Premium, Enterprise). Pricing and payment terms are provided on the site and at purchase.',
                    'Billing: By subscribing, you authorize Gaply (or our payment processor) to charge the payment method you provide. You are responsible for all applicable taxes.',
                    'Trials & Promotions: Trial periods or promotional offers may be offered; terms are provided at sign-up.',
                    'Refunds: [Insert your refund policy here — e.g., “We offer a 7-day refund for annual plans if no more than X credits used” or “All sales are final except where required by law.”] Tailor this to your business policy.',
                    'Suspension for non-payment: Gaply may suspend access for overdue amounts.'
                  ]
                },
                {
                  title: '10. Third-Party Services',
                  text: [
                    'Integrations: The Services may integrate or rely on third-party providers (e.g., OpenAI, Supabase, CrossRef, Turnitin/Copyleaks). Use of these features may require that you accept additional terms. Gaply is not responsible for third-party services or their privacy practices.',
                    'Optional services: Some features (e.g., Turnitin checks) are optional and may incur additional fees. You must explicitly enable and authorize these services when used.'
                  ]
                },
                {
                  title: '11. Intellectual Property',
                  text: [
                    'Gaply IP: Gaply owns all rights, title, and interest in the Services and related software, content, and documentation, excluding User Content. You may not copy, modify, create derivative works, or distribute Gaply’s proprietary content or software except as permitted in writing.',
                    'Feedback: If you provide feedback or suggestions, you grant Gaply a perpetual, worldwide, royalty-free license to use and implement that feedback.'
                  ]
                },
                {
                  title: '12. Accuracy, No Guarantees & Use of Outputs',
                  text: [
                    'Guidance only: The Services generate analyses, scores, and recommendations intended for research assistance. They are guidance only and do not guarantee publication, plagiarism outcomes, or legal, medical, or professional results.',
                    'No professional advice: The Services do not replace consultation with domain experts, supervisors, or legal counsel. You are responsible for verifying results before relying on them.'
                  ]
                },
                {
                  title: '13. Warranties & Disclaimers',
                  text: [
                    'No warranty: TO THE MAXIMUM EXTENT PERMITTED BY LAW, THE SERVICES ARE PROVIDED “AS IS” AND “AS AVAILABLE” WITHOUT WARRANTIES OF ANY KIND. GAPLY DISCLAIMS ALL WARRANTIES, EXPRESS OR IMPLIED, INCLUDING MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, AND NON-INFRINGEMENT.',
                    'Third-party info: Gaply does not guarantee the accuracy or availability of third-party data or services.'
                  ]
                },
                {
                  title: '14. Limitation of Liability',
                  text: [
                    'No indirect damages: TO THE MAXIMUM EXTENT PERMITTED BY LAW, GAPLY WILL NOT BE LIABLE FOR INDIRECT, INCIDENTAL, SPECIAL, CONSEQUENTIAL, OR PUNITIVE DAMAGES, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGES.',
                    'Cap on liability: GAPLY’S AGGREGATE LIABILITY FOR ANY CLAIM ARISING OUT OF OR RELATING TO THESE TERMS OR THE SERVICES WILL NOT EXCEED THE AMOUNTS YOU HAVE PAID GAPLY IN THE TWELVE (12) MONTHS PRECEDING THE CLAIM, OR INR [Insert nominal amount] IF YOU HAVE NOT PAID FOR THE SERVICES. Some jurisdictions do not allow limitation of liability; where prohibited, liability is limited to the fullest extent permitted by law.'
                  ]
                },
                {
                  title: '15. Indemnification',
                  text: [
                    'You agree to indemnify, defend and hold harmless Gaply, its affiliates, and their officers, employees, and agents from any claims, liabilities, losses, damages, and costs (including reasonable attorneys’ fees) arising from your breach of these Terms, your Content, your violation of law, or your misuse of the Services.'
                  ]
                },
                {
                  title: '16. Termination & Suspension',
                  text: [
                    'By Gaply: We may suspend or terminate your access for violations, misuse, non-payment, or for any reason on notice.',
                    'By you: You may delete your account via account settings; deletion removes saved content as described in the Privacy Policy.',
                    'Effect of termination: Termination does not relieve you of obligations incurred prior to termination. Clauses that by their nature survive termination (e.g., Indemnification, Limitation of Liability, Governing Law) will survive.'
                  ]
                },
                {
                  title: '17. Dispute Resolution; Governing Law',
                  text: [
                    'Governing law: These Terms are governed by the laws of India.',
                    'Dispute resolution: In the event of dispute, parties agree to attempt to resolve the dispute through good-faith negotiations. If unresolved within 30 days, disputes will be subject to the exclusive jurisdiction of the courts located in [Insert City, e.g., New Delhi], India. You may propose arbitration if preferred; otherwise court proceedings will apply.'
                  ]
                },
                {
                  title: '18. Export Controls & Sanctions',
                  text: [
                    'You agree not to use the Services in ways that violate applicable export laws or sanctions.',
                    'You warrant that you are not located in, under control of, or a national of any country subject to applicable export embargoes.'
                  ]
                },
                {
                  title: '19. Miscellaneous',
                  text: [
                    'Entire agreement: These Terms and the Privacy Policy constitute the entire agreement between you and Gaply relating to the Services.',
                    'Severability: If any provision is found unenforceable, the remainder will remain in effect.',
                    'Waiver: A failure to enforce any term is not a waiver of that term.',
                    'Assignment: You may not assign these Terms without Gaply’s prior written consent. Gaply may assign its rights in connection with a merger or sale.'
                  ]
                },
                {
                  title: '20. Contact',
                  text: [
                    'If you have questions or need to contact Gaply about these Terms, please email: helloresearcher@gaply.in',
                    'Mailing address: [Insert physical address if applicable]'
                  ]
                },
                {
                  title: 'Final notes & next steps',
                  text: [
                    'Fill in placeholders (effective date, refund policy, liability cap amount, jurisdiction city, physical address).',
                    'Add any enterprise-specific clauses if you plan to offer institutional deployments, including SLAs, data residency, and on-premises deployment terms.',
                    'Have your legal counsel review and adapt the Terms to local laws and your business needs (especially the refund policy, data residency promises, and liability clauses).',
                    'If you’d like, I can also draft a Privacy Policy companion page with matching legal language, convert these Terms into an HTML page ready to paste into your website CMS, or produce a short end-user summary (1–2 paragraphs) to display as a popup when users sign up.'
                  ]
                }
              ].map((section) => (
                <div key={section.title} style={{
                  padding: '16px 18px',
                  borderRadius: 16,
                  border: '1px solid var(--divider)',
                  background: 'var(--content-bg)'
                }}>
                  <div style={{
                    fontWeight: 700,
                    marginBottom: 8
                  }}>
                    {section.title}
                  </div>
                  <ul style={{
                    margin: 0,
                    paddingLeft: 18,
                    color: 'var(--muted-text)',
                    lineHeight: 1.7
                  }}>
                    {section.text.map((line) => (
                      <li key={line} style={{ marginBottom: 6 }}>
                        {line}
                      </li>
                    ))}
                  </ul>
                </div>
              ))}
            </div>
          </section>
        </div>
      </main>
    </div>
  );
};

export default TermsOfServicePage;
