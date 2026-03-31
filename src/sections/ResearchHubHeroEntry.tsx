import React from 'react';
import { Link } from 'react-router-dom';
import './ResearchHubHeroEntry.css';

const ResearchHubHeroEntry: React.FC = () => {
  return (
    <Link className="rh-hero-entry" to="/research-hub">
      <span className="rh-hero-entry__badge" aria-hidden>
        <span className="rh-hero-entry__badge-dot" />
        New
      </span>
      <div className="rh-hero-entry__body">
        <span className="rh-hero-entry__kicker">ResearchHub</span>
        <span className="rh-hero-entry__title">Co-authors &amp; researcher discovery</span>
        <span className="rh-hero-entry__hint">
          Profiles, feed, domain-matched connections, and DMs — built for how research actually works.
        </span>
      </div>
      <span className="rh-hero-entry__chevron" aria-hidden>
        →
      </span>
    </Link>
  );
};

export default ResearchHubHeroEntry;
