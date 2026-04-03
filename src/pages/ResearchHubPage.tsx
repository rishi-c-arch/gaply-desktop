import React from 'react';
import { Link } from 'react-router-dom';
import { useTheme } from '../contexts/ThemeContext';
import './ResearchHubPage.css';

const ResearchHubPage: React.FC = () => {
  const { theme } = useTheme();

  return (
    <div className="rh-landing" data-theme={theme}>
      <div className="rh-landing__noise" aria-hidden />
      <div className="rh-landing__inner">
        <p className="rh-landing__soon">Coming soon</p>

        <div className="rh-landing__copy">
          <p className="rh-landing__lead">
            A dedicated social layer for researchers. Share your work, invite critique, and keep
            discussions sharp and on topic. Meet peers worldwide, send connection requests, and
            message in private when you are ready to go deeper.
          </p>
          <p className="rh-landing__lead">
            Team up on the same projects, line up corporate and government funding together, and
            grow your network as you ship real science.
          </p>
          <ul className="rh-landing__list">
            <li>Profiles and posts built for papers, methods, and open questions</li>
            <li>Feeds that surface collaborators and timely calls for input</li>
            <li>Connection requests and DMs between researchers</li>
            <li>Shared projects with room for joint grants and partnerships</li>
          </ul>
        </div>

        <div className="rh-landing__cta-row">
          <Link className="rh-landing__cta" to="/">
            Back to home
          </Link>
          <Link className="rh-landing__cta-secondary" to="/login">
            Log in
          </Link>
        </div>
      </div>
    </div>
  );
};

export default ResearchHubPage;
