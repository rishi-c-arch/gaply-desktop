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
        { label: 'Watch Demo', href: '/watch-demo' },
        { label: 'Support', href: '/support' },
        { label: 'Privacy Policy', href: '/privacy' },
        { label: 'Terms of Service', href: '/terms' },
        { label: 'Scopus journals & APC guide', href: '/guides/scopus-indexed-journals-low-apc' },
        { label: 'Fast publication (India)', href: '/guides/fast-publication-scopus-journals-india' },
      ],
    },
    {
      title: 'Ethical AI guide',
      links: [
        {
          label: 'How to remove AI detection from research paper 2026',
          href: '/guides/ethical-researcher-guide-ai-academic-writing?topic=ai-detection-2026',
        },
        {
          label: 'Free AI rewriter for academic papers to bypass Turnitin',
          href: '/guides/ethical-researcher-guide-ai-academic-writing?topic=rewriter-bypass-turnitin',
        },
        {
          label: 'Best humanizer tools for research writing',
          href: '/guides/ethical-researcher-guide-ai-academic-writing?topic=humanizer-tools',
        },
        {
          label: 'Does ChatGPT text pass university plagiarism checks in India?',
          href: '/guides/ethical-researcher-guide-ai-academic-writing?topic=chatgpt-plagiarism-india',
        },
        {
          label: 'How to use AI for literature review without being flagged',
          href: '/guides/ethical-researcher-guide-ai-academic-writing?topic=literature-review-flagged',
        },
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
