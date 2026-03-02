import React from 'react';
import { useNavigate } from 'react-router-dom';

interface FreeFeatureCardProps {
  title: string;
  description: string;
  icon: React.ReactNode;
  path: string;
  index: number;
  inView: boolean;
}

export default function FreeFeatureCard({
  title,
  description,
  icon,
  path,
  index,
  inView,
}: FreeFeatureCardProps) {
  const navigate = useNavigate();

  return (
    <article
      className={`free-feature-card section-card-entrance ${inView ? 'in-view' : ''}`}
      style={{ transitionDelay: `${index * 100}ms` }}
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
      <div className="free-feature-card__icon">{icon}</div>
      <h3 className="free-feature-card__title">{title}</h3>
      <p className="free-feature-card__desc">{description}</p>
      <span className="free-feature-card__link">Explore →</span>
    </article>
  );
}
