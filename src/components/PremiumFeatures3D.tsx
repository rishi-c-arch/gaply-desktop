import React from 'react';
import './PremiumFeatures3D.css';

const PremiumFeatures3D: React.FC = () => {
  const premiumFeatures = [
    {
      id: 'publishready',
      title: 'PublishReady',
      tagline: '“Is your paper ready for publication? Instant, actionable feedback.”',
      reason: 'Why: clear, trustable, instantly understandable.',
      imageSrc: '/images/i.jpg',
      imageAlt: 'Researcher reviewing manuscript for publication readiness',
      imagePosition: '50% 75%'
    },
    {
      id: 'datamaestro',
      title: 'DataMaestro',
      tagline: '“Conduct any statistical test, hypothesis or model — orchestrated smartly.”',
      reason: 'Why: suggests mastery and orchestration of analyses.',
      imageSrc: '/images/j.jpg',
      imageAlt: 'Analyst reviewing data and statistical models',
      imagePosition: '50% 35%'
    }
  ];

  return (
    <section className="premium-features">
      <div className="premium-features__header">
        <h2>Premium Features</h2>
        <div className="premium-features__divider" />
      </div>

      <div className="premium-features__grid">
        {premiumFeatures.map((item) => (
          <article key={item.id} className="premium-feature-card">
            <div className="premium-feature-card__media">
              <div
                className="premium-feature-card__image"
                style={{
                  backgroundImage: `url(${item.imageSrc})`,
                  backgroundPosition: item.imagePosition
                }}
                role="img"
                aria-label={item.imageAlt}
              />
            </div>
            <div className="premium-feature-card__content">
              <h3>{item.title}</h3>
              <p className="premium-feature-card__tagline">{item.tagline}</p>
              <p className="premium-feature-card__reason">{item.reason}</p>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
};

export default PremiumFeatures3D;

