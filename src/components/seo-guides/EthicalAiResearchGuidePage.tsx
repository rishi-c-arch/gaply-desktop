import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import SeoGuideShell from './SeoGuideShell';
import { ETHICAL_AI_SLIDE_TITLES } from './ethicalAiSlideTitles';
import EthicalAiDeckExperience from './EthicalAiDeckExperience';
import { ETHICAL_AI_GUIDE_TOPICS } from './ethicalAiGuideTopics';
import deck from '../../data/ethicalAiGuideSlides.json';

const GUIDE_PATH = '/guides/ethical-researcher-guide-ai-academic-writing';
/** Office Online requires a public HTTPS URL; use production origin on localhost so the viewer works when the file is deployed. */
const PRODUCTION_ORIGIN = 'https://www.gaply.in';

const PAGE_URL = 'https://www.gaply.in/guides/ethical-researcher-guide-ai-academic-writing';
const LD_ID = 'ld-json-ethical-ai-guide';

interface SlideRow {
  slide: number;
  id: string;
  body: string;
}

function FeatureDotList({ items }: { items: string[] }) {
  return (
    <div className="ethical-ai-feature-dotlist" role="list">
      {items.map((text) => (
        <div key={text} className="ethical-ai-feature-dotlist__row" role="listitem">
          <span className="ethical-ai-feature-dotlist__dot" aria-hidden />
          <span>{text}</span>
        </div>
      ))}
    </div>
  );
}

const HERO_FEATURE_LINES = [
  'Same 28-slide deck for every footer link and topic pill (only the URL topic changes).',
  'Designed slides: flip through the in-page PDF when it is deployed, or download the .pptx.',
  'The “Text version” section at the bottom is for search and screen readers, not slide layout.',
];

const PRESENTATION_FEATURE_LINES = [
  'Use Previous / Next, arrow keys, or swipe when the PDF is on the server.',
  'Best quality: export the deck from PowerPoint as PDF and deploy it with the site.',
  'If the PDF is missing, we load Microsoft or Google’s viewer for the .pptx instead.',
];

const EthicalAiResearchGuidePage: React.FC = () => {
  const slides = deck.slides as SlideRow[];
  const total = slides.length;
  const [searchParams] = useSearchParams();
  const topicSlug = searchParams.get('topic') || '';
  const [activeSlide, setActiveSlide] = useState(1);
  const sectionRefs = useRef<(HTMLElement | null)[]>([]);

  useEffect(() => {
    if (!topicSlug) return;
    const deckEl = document.getElementById('ethical-ai-deck-anchor');
    requestAnimationFrame(() =>
      deckEl?.scrollIntoView({ behavior: 'smooth', block: 'start' })
    );
  }, [topicSlug]);

  useEffect(() => {
    if (!topicSlug) return;
    const topicEl = document.getElementById(`ethical-ai-topic-${topicSlug}`) as HTMLDetailsElement | null;
    requestAnimationFrame(() => {
      if (topicEl) topicEl.open = true;
    });
  }, [topicSlug]);

  const titleFor = useCallback(
    (index: number) => ETHICAL_AI_SLIDE_TITLES[index] ?? `Slide ${index + 1}`,
    []
  );

  const scrollToSlide = useCallback(
    (n: number) => {
      const clamped = Math.min(Math.max(1, n), total);
      const el = document.getElementById(`ethical-ai-slide-${clamped}`);
      el?.scrollIntoView({ behavior: 'smooth', block: 'start' });
      setActiveSlide(clamped);
    },
    [total]
  );

  useEffect(() => {
    const els = sectionRefs.current.filter(Boolean) as HTMLElement[];
    if (els.length === 0) return;

    const io = new IntersectionObserver(
      (entries) => {
        let bestIdx = 0;
        let bestRatio = 0;
        for (const en of entries) {
          if (!en.isIntersecting) continue;
          const i = els.indexOf(en.target as HTMLElement);
          if (i < 0) continue;
          if (en.intersectionRatio > bestRatio) {
            bestRatio = en.intersectionRatio;
            bestIdx = i;
          }
        }
        if (bestRatio > 0.08) setActiveSlide(bestIdx + 1);
      },
      { rootMargin: '-10% 0px -35% 0px', threshold: [0, 0.1, 0.25, 0.5, 0.75, 1] }
    );

    els.forEach((el) => io.observe(el));
    return () => io.disconnect();
  }, [total]);

  useEffect(() => {
    const faqLd = {
      '@context': 'https://schema.org',
      '@graph': [
        {
          '@type': 'WebPage',
          '@id': `${PAGE_URL}#webpage`,
          url: PAGE_URL,
          name: 'Ethical research guide: AI detection, Turnitin, humanizers & literature reviews (2026)',
          description:
            'Embedded presentation plus ethical guidance on AI detection limits, Turnitin in India, humanizer myths, and literature reviews without misconduct.',
          isPartOf: { '@type': 'WebSite', name: 'Gaply', url: 'https://www.gaply.in' },
        },
        {
          '@type': 'FAQPage',
          '@id': `${PAGE_URL}#faq`,
          mainEntity: [
            {
              '@type': 'Question',
              name: 'How to remove AI detection from a research paper in 2026?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Trying to hide or remove AI detection signals is the wrong goal for scholarly work. Ethical practice is to write your own analysis, use AI transparently where your institution allows it, disclose assistance, and verify every claim. Detection tools are imperfect but evasion can constitute academic dishonesty.',
              },
            },
            {
              '@type': 'Question',
              name: 'Is there a free AI rewriter for academic papers to bypass Turnitin?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Using rewriters or humanizers to bypass Turnitin or similar checks risks academic misconduct. Turnitin and university policies treat undisclosed AI-generated work as a integrity issue. Ethical alternatives include drafting your own text, using AI only for permitted feedback, and following your course policy on disclosure.',
              },
            },
            {
              '@type': 'Question',
              name: 'What are the best humanizer tools for research writing?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Tools marketed as AI humanizers or detector bypasses are unreliable and often violate academic integrity rules. Better approaches: write first, use AI as a coach for structure or clarity suggestions you apply yourself, vary your own sentence style, and disclose AI use when required.',
              },
            },
            {
              '@type': 'Question',
              name: 'Does ChatGPT text pass university plagiarism checks in India?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Many Indian universities use Turnitin and similar systems with AI detection. ChatGPT-style text can be flagged, and detectors have false positives and limits. The safer question is whether your use follows your institution policy and whether you can disclose and defend your authorship—not whether text passes a detector.',
              },
            },
            {
              '@type': 'Question',
              name: 'How to use AI for a literature review without being flagged?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Use AI to discover or organize papers, then read primary sources yourself, verify every citation in a database, synthesize in your own words, and disclose AI assistance if required. AI cannot replace critical reading; relying on summaries alone produces shallow reviews and higher risk.',
              },
            },
          ],
        },
      ],
    };

    document.getElementById(LD_ID)?.remove();
    const script = document.createElement('script');
    script.id = LD_ID;
    script.type = 'application/ld+json';
    script.textContent = JSON.stringify(faqLd);
    document.head.appendChild(script);
    return () => document.getElementById(LD_ID)?.remove();
  }, []);

  const tocItems = useMemo(
    () =>
      slides.map((s, i) => ({
        n: s.slide,
        href: `#ethical-ai-slide-${s.slide}`,
        label: titleFor(i),
      })),
    [slides, titleFor]
  );

  return (
    <SeoGuideShell headline={deck.title} subhead={deck.subtitle} variant="feature">
      <div className="ethical-ai-deck-main">
        <nav className="ethical-ai-sticky-jump" aria-label="On this page">
          <a href="#ethical-ai-deck-anchor">Slides</a>
          <a href="#ethical-ai-topics-anchor">Topics</a>
          <a href="#ethical-ai-transcript-anchor">Text version</a>
        </nav>

        <section className="seo-guide-card ethical-ai-hero-compact ethical-ai-feature-panel">
          <h2 className="ethical-ai-feature-panel__title">One deck for every search topic</h2>
          <p className="ethical-ai-feature-panel__sub">{deck.sourceNote}</p>
          <p className="ethical-ai-feature-panel__desc">
            Footer links and pills match how people search. The slide presentation is the same each time; only the
            highlighted topic changes.
          </p>
          <FeatureDotList items={HERO_FEATURE_LINES} />
          <p className="ethical-ai-feature-panel__links">
            <Link to="/academic-ai-remover">Academic AI review</Link>
            <span aria-hidden> · </span>
            <Link to="/journal-matching">Journal matching</Link>
          </p>
        </section>

        <section
          id="ethical-ai-deck-anchor"
          className="seo-guide-card ethical-ai-slide-focus ethical-ai-deck-card"
          aria-labelledby="ethical-ai-embed-heading"
        >
          <div className="ethical-ai-deck-card__head">
            <h2 id="ethical-ai-embed-heading" className="ethical-ai-feature-panel__title">
              Presentation
            </h2>
            <p className="ethical-ai-feature-panel__sub">View slides on this page</p>
            <p className="ethical-ai-feature-panel__desc">
              When the PDF is live, move through slides with the controls or keyboard. If it is not deployed yet, we
              load a viewer for the PowerPoint file instead.
            </p>
            <FeatureDotList items={PRESENTATION_FEATURE_LINES} />
          </div>
          <EthicalAiDeckExperience />
          <details className="ethical-ai-copy-hint">
            <summary>Hosting the files (Terminal)</summary>
            <p style={{ margin: '12px 0 0', fontSize: '0.9rem', lineHeight: 1.65, color: 'var(--section-text)' }}>
              <strong>Best on-page experience:</strong> in PowerPoint use <strong>File → Export → PDF</strong>, then from{' '}
              <code>~/Desktop/GAPLY/gaply-react-frontend</code> run <code>npm run copy-ethical-pdf</code> (or copy to{' '}
              <code>public/guides/ethical-researcher-guide-ai-2026.pdf</code>).
            </p>
            <p style={{ margin: '10px 0 0', fontSize: '0.9rem', lineHeight: 1.65, color: 'var(--section-text)' }}>
              <strong>.pptx</strong> for download / Microsoft viewer: <code>npm run copy-ethical-pptx</code> or:
            </p>
            <pre className="ethical-ai-copy-hint__cmd">
              cd ~/Desktop/GAPLY/gaply-react-frontend{'\n'}
              npm run copy-ethical-pptx
            </pre>
            <pre className="ethical-ai-copy-hint__cmd">
              {`node ~/Desktop/GAPLY/gaply-react-frontend/scripts/copy-ethical-ppt-to-public.js "$HOME/Desktop/The Ethical Researcher's Guide to AI (2).pptx"`}
            </pre>
            <p style={{ margin: '8px 0 0', fontSize: '0.9rem', lineHeight: 1.65, color: 'var(--section-text)' }}>
              Optional env: <code>REACT_APP_ETHICAL_AI_PDF_URL</code>, <code>REACT_APP_ETHICAL_AI_PPTX_URL</code> in{' '}
              <code>.env.local</code>. On localhost, embeds resolve against {PRODUCTION_ORIGIN} so files must be
              deployed there or set via env.
            </p>
          </details>
        </section>

        <nav className="ethical-ai-topic-pills" aria-label="Search topics" id="ethical-ai-topics-anchor">
          {ETHICAL_AI_GUIDE_TOPICS.map((t) => {
            const active = topicSlug === t.slug;
            return (
              <Link
                key={t.slug}
                to={`${GUIDE_PATH}?topic=${encodeURIComponent(t.slug)}`}
                className={`ethical-ai-topic-pill${active ? ' ethical-ai-topic-pill--active' : ''}`}
              >
                {t.label}
              </Link>
            );
          })}
          {topicSlug ? (
            <Link to={GUIDE_PATH} className="ethical-ai-topic-pill ethical-ai-topic-pill--clear">
              Clear topic
            </Link>
          ) : null}
        </nav>

        <div className="ethical-ai-topic-accordion" role="region" aria-label="Topic summaries">
          {ETHICAL_AI_GUIDE_TOPICS.map((t) => (
            <details
              key={t.slug}
              name="ethical-ai-topic"
              id={`ethical-ai-topic-${t.slug}`}
              className="ethical-ai-topic-acc"
            >
              <summary className="ethical-ai-topic-acc__summary">
                <h2 className="ethical-ai-topic-acc__h" id={`ethical-ai-seo-h-${t.slug}`}>
                  {t.label}
                </h2>
              </summary>
              <div className="ethical-ai-topic-acc__body">
                <p>{t.intro}</p>
                <a href="#ethical-ai-deck-anchor" className="ethical-ai-topic-acc__to-slides">
                  View slides above
                </a>
              </div>
            </details>
          ))}
        </div>

        <details className="ethical-ai-transcript" id="ethical-ai-transcript-anchor">
          <summary>Full text transcript (all slides, for search and accessibility)</summary>

          <div className="ethical-ai-deck-toolbar" role="toolbar" aria-label="Transcript slide navigation">
            <button
              type="button"
              disabled={activeSlide <= 1}
              onClick={() => scrollToSlide(activeSlide - 1)}
            >
              Previous slide
            </button>
            <span className="ethical-ai-deck-counter" aria-live="polite">
              Transcript {activeSlide} / {total}
            </span>
            <button
              type="button"
              disabled={activeSlide >= total}
              onClick={() => scrollToSlide(activeSlide + 1)}
            >
              Next slide
            </button>
          </div>

          <nav className="ethical-ai-toc" aria-label="Transcript slide list">
            <details>
              <summary>Jump to slide in transcript</summary>
              <ol style={{ margin: '12px 0 0', paddingLeft: '1.25rem', lineHeight: 1.8 }}>
                {tocItems.map((item) => (
                  <li key={item.n}>
                    <a href={item.href}>{item.label}</a>
                  </li>
                ))}
              </ol>
            </details>
          </nav>

          <article>
            {slides.map((s, i) => {
              const heading = titleFor(i);
              const body =
                s.body.trim() ||
                'This slide in the original presentation is primarily visual. See surrounding transcript or the embedded deck above.';

              return (
                <section
                  key={s.id}
                  id={`ethical-ai-slide-${s.slide}`}
                  className="seo-guide-card ethical-ai-transcript-slide"
                  aria-labelledby={`ethical-ai-h-${s.slide}`}
                  ref={(el) => {
                    sectionRefs.current[i] = el;
                  }}
                >
                  <p style={{ fontSize: '0.85rem', color: 'var(--muted-text)', marginBottom: 8 }}>
                    Slide {s.slide} of {total}
                  </p>
                  <h3 id={`ethical-ai-h-${s.slide}`} style={{ fontSize: 'clamp(1.05rem, 2.5vw, 1.35rem)' }}>
                    {heading}
                  </h3>
                  <div className="ethical-ai-slide-body">{body}</div>
                </section>
              );
            })}
          </article>
        </details>
      </div>
    </SeoGuideShell>
  );
};

export default EthicalAiResearchGuidePage;
