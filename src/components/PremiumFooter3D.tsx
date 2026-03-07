import React from 'react';
import { Link } from 'react-router-dom';
import './PremiumFooter3D.css';

const PremiumFooter3D: React.FC = () => {
  const columns = [
    {
      title: 'Product',
      links: [
        { label: 'Features', href: '/features' },
        { label: 'Pricing', href: '/pricing' },
        { label: 'Download', href: '/download' },
        { label: 'Journal Matching', href: '/journal-matching' },
        { label: 'Paper Search', href: '/paper-search' },
      ],
    },
    {
      title: 'Company',
      links: [
        { label: 'Careers', href: '/career' },
        { label: 'Contact', href: '/contact' },
      ],
    },
    {
      title: 'Resources',
      links: [
        { label: 'Blog', href: '/blog' },
        { label: 'Support', href: '#support' },
        { label: 'Privacy Policy', href: '/privacy' },
        { label: 'Terms of Service', href: '/terms' },
      ],
    },
  ];

  return (
    <footer className="premium-footer">
      {/* Seam blend: softly mix light page into dark footer (no animation) */}
      <div className="premium-footer__seam" />
      <div className="premium-footer__inner">
        <div className="premium-footer__top">
          <div className="premium-footer__brand">
            <div className="premium-footer__logo">Gaply</div>
            <p className="premium-footer__tagline">Accelerating academic research</p>
            <p className="premium-footer__desc">
              A premium research platform built for clarity, integrity, and better academic outcomes.
            </p>
          </div>

          <div className="premium-footer__columns" aria-label="Footer">
            {columns.map((column) => (
              <div key={column.title} className="premium-footer__column">
                <div className="premium-footer__column-title">{column.title}</div>
                <ul className="premium-footer__links">
                  {column.links.map((link) => (
                    <li key={link.label}>
                      {link.href.startsWith('/') && !link.href.startsWith('//') ? (
                        <Link to={link.href} className="premium-footer__link">
                          {link.label}
                        </Link>
                      ) : (
                        <a href={link.href} className="premium-footer__link">
                          {link.label}
                        </a>
                      )}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>

        <div className="premium-footer__bottom">
          <span>© {new Date().getFullYear()} Gaply. All rights reserved.</span>
          <span>Built with care for researchers worldwide.</span>
        </div>
      </div>
    </footer>
  );
};

export default PremiumFooter3D;


// Force rebuild Wed Oct  8 01:58:34 IST 2025
