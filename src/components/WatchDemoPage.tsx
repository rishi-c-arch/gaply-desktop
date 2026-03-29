import React from 'react';
import { useTheme } from '../contexts/ThemeContext';
import './WatchDemoPage.css';

const YOUTUBE_CHANNEL = 'https://youtube.com/@gaplyresearch?si=XRdEht3eCYN4mHG1';

interface DemoItem {
  id: string;
  title: string;
  description: string;
  type: 'playlist' | 'video';
  embedUrl: string;
  playlistUrl?: string;
  videoUrl?: string;
  accent: string;
  comingSoon?: boolean;
}

const DEMO_ITEMS: DemoItem[] = [
  {
    id: 'intro',
    title: 'Introduction to Gaply',
    description: 'Get started with Gaply — your AI-powered academic research platform. Learn the basics and explore the interface.',
    type: 'playlist',
    embedUrl: 'https://www.youtube.com/embed/videoseries?list=PLGqR-7KjVXdHHqT1LCMHcKKremolpwzV-',
    playlistUrl: 'https://youtube.com/playlist?list=PLGqR-7KjVXdHHqT1LCMHcKKremolpwzV-',
    accent: 'var(--accent-purple)',
  },
  {
    id: 'free',
    title: 'Free Features',
    description: 'Paper Search, Journal Matching, AI Content Detection — explore everything you can do for free on Gaply.',
    type: 'playlist',
    embedUrl: 'https://www.youtube.com/embed/videoseries?list=PLGqR-7KjVXdHni2fCYsdw3ADouQwUoT0p',
    playlistUrl: 'https://youtube.com/playlist?list=PLGqR-7KjVXdHni2fCYsdw3ADouQwUoT0p',
    accent: '#22c55e',
  },
  {
    id: 'publishready',
    title: 'PublishReady',
    description: 'Journal-fit evaluation in minutes. See how PublishReady compares your manuscript to journal rules and returns a publication-likelihood score.',
    type: 'video',
    embedUrl: 'https://www.youtube.com/embed/AebVhNpumAM',
    videoUrl: 'https://youtu.be/AebVhNpumAM',
    accent: '#0ea5e9',
  },
  {
    id: 'datamaestro',
    title: 'DataMaestro Pro',
    description: 'Academic-grade statistical analysis. Upload data, get publication-ready tables, figures, and plain-English interpretations.',
    type: 'video',
    embedUrl: '',
    accent: '#f43f5e',
    comingSoon: true,
  },
  {
    id: 'journal-verify',
    title: 'Journal Verification',
    description: 'Verify journal authenticity and quartile rankings. Ensure you submit to legitimate, indexed journals.',
    type: 'video',
    embedUrl: 'https://www.youtube.com/embed/I84f_9IDJAY',
    videoUrl: 'https://youtu.be/I84f_9IDJAY?si=qqm9mf56c0F_1LLE',
    accent: '#f59e0b',
  },
  {
    id: 'research-deep',
    title: 'Research Deep Analysis',
    description: 'Deep-dive analysis for your research. Get comprehensive insights and publication readiness assessment.',
    type: 'video',
    embedUrl: '',
    accent: '#8b5cf6',
    comingSoon: true,
  },
];

const WatchDemoPage: React.FC = () => {
  const { theme } = useTheme();

  return (
    <div className="watch-demo" data-theme={theme}>
      <div className="watch-demo__noise" aria-hidden="true" />
      <div className="watch-demo__bg" />

      {/* Hero */}
      <section className="watch-demo__hero">
        <div className="watch-demo__hero-content">
          <span className="watch-demo__badge">Video tutorials</span>
          <h1 className="watch-demo__title">
            Watch <span className="watch-demo__title-accent">Demo</span>
          </h1>
          <p className="watch-demo__subtitle">
            Learn how to use Gaply with step-by-step video guides. From getting started to mastering premium features.
          </p>
          <a
            href={YOUTUBE_CHANNEL}
            target="_blank"
            rel="noopener noreferrer"
            className="watch-demo__channel-link"
          >
            <svg viewBox="0 0 24 24" fill="currentColor" width={20} height={20}>
              <path d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z" />
            </svg>
            Subscribe to Gaply Research on YouTube
          </a>
        </div>
      </section>

      {/* Demo grid */}
      <section className="watch-demo__content">
        <div className="watch-demo__grid">
          {DEMO_ITEMS.map((item, index) => (
            <article
              key={item.id}
              className="watch-demo__card"
              style={{ '--card-accent': item.accent } as React.CSSProperties}
            >
              <div className="watch-demo__card-header">
                <span className="watch-demo__card-number">{String(index + 1).padStart(2, '0')}</span>
                <h2 className="watch-demo__card-title">{item.title}</h2>
                <p className="watch-demo__card-desc">{item.description}</p>
              </div>

              <div className="watch-demo__card-media">
                {item.comingSoon ? (
                  <div className="watch-demo__coming-soon">
                    <div className="watch-demo__coming-soon-icon">
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                        <path d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z" />
                      </svg>
                    </div>
                    <p>Demo coming soon</p>
                    <span>We&apos;re preparing this tutorial</span>
                  </div>
                ) : (
                  <div className="watch-demo__embed-wrap">
                    <iframe
                      src={item.embedUrl}
                      title={item.title}
                      allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
                      allowFullScreen
                      className="watch-demo__embed"
                    />
                  </div>
                )}
              </div>

              {!item.comingSoon && (item.playlistUrl || item.videoUrl) && (
                <a
                  href={item.playlistUrl || item.videoUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="watch-demo__card-link"
                >
                  Watch on YouTube
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width={14} height={14}>
                    <path d="M18 13v6a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2h6M15 3h6v6M10 14L21 3" />
                  </svg>
                </a>
              )}
            </article>
          ))}
        </div>
      </section>

      {/* Footer CTA */}
      <section className="watch-demo__cta">
        <div className="watch-demo__cta-content">
          <h3>Ready to get started?</h3>
          <p>Try Gaply free — no credit card required.</p>
          <a href="/" className="watch-demo__cta-btn">
            Explore Gaply
          </a>
        </div>
      </section>
    </div>
  );
};

export default WatchDemoPage;
