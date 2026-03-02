import React from 'react';
import { useNavigate } from 'react-router-dom';

interface PremiumFeatureCardProps {
  title: string;
  description: string;
  icon: React.ReactNode;
  path: string;
}

export default function PremiumFeatureCard({
  title,
  description,
  icon,
  path,
}: PremiumFeatureCardProps) {
  const navigate = useNavigate();

  return (
    <article
      className="premium-feature-card-v2"
      tabIndex={0}
      aria-label={`${title} feature`}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          navigate(path);
        }
      }}
      onClick={() => navigate(path)}
      role="button"
    >
      <span className="premium-feature-card-v2__badge">PREMIUM</span>
      <div className="premium-feature-card-v2__icon-wrap">{icon}</div>
      <h3 className="premium-feature-card-v2__title">{title}</h3>
      <p className="premium-feature-card-v2__desc">{description}</p>
    </article>
  );
}
