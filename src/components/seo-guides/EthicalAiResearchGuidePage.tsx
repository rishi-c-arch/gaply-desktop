import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import SeoGuideShell from './SeoGuideShell';
import { ETHICAL_AI_SLIDE_TITLES } from './ethicalAiSlideTitles';
import { ETHICAL_AI_GUIDE_TOPICS, ETHICAL_AI_PPTX_PATH } from './ethicalAiGuideTopics';
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

type DeckViewer = 'office' | 'google';

function PptxDeckEmbed({ pptxPath }: { pptxPath: string }) {
  const [viewer, setViewer] = useState<DeckViewer>('office');
  const [absoluteUrl, setAbsoluteUrl] = useState('');

  useEffect(() => {
    const fromEnv = process.env.REACT_APP_ETHICAL_AI_PPTX_URL?.trim();
    if (fromEnv) {
      setAbsoluteUrl(fromEnv);
      return;
    }
    const host = window.location.hostname;
    const useProd =
      host === 'localhost' || host === '127.0.0.1' || host.endsWith('.local');
    const origin = useProd ? PRODUCTION_ORIGIN : window.location.origin;
    setAbsoluteUrl(`${origin}${pptxPath}`);
  }, [pptxPath]);

  const embedSrc = absoluteUrl
    ? viewer === 'office'
      ? `https://view.officeapps.live.com/op/embed.aspx?src=${encodeURIComponent(absoluteUrl)}`
      : `https://docs.google.com/viewer?url=${encodeURIComponent(absoluteUrl)}&embedded=true`
    : '';

  const downloadLocal =
    typeof window !== 'undefined' ? `${window.location.origin}${pptxPath}` : pptxPath;
  const isLocalhost =
    typeof window !== 'undefined' &&
    (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1');

  return (
    <div className="ethical-ai-pptx-wrap">
      <div className="ethical-ai-viewer-tabs" role="tablist" aria-label="Presentation viewer">
        <button
          type="button"
          role="tab"
          aria-selected={viewer === 'office'}
          className={`ethical-ai-viewer-tab${viewer === 'office' ? ' ethical-ai-viewer-tab--active' : ''}`}
          onClick={() => setViewer('office')}
        >
          Microsoft viewer
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={viewer === 'google'}
          className={`ethical-ai-viewer-tab${viewer === 'google' ? ' ethical-ai-viewer-tab--active' : ''}`}
          onClick={() => setViewer('google')}
        >
          Google viewer
        </button>
        {embedSrc ? (
          <a
            className="ethical-ai-viewer-open-tab"
            href={embedSrc}
            target="_blank"
            rel="noopener noreferrer"
          >
            Open full presentation (new tab)
          </a>
        ) : null}
      </div>
      <div className="ethical-ai-pptx-frame">
        {embedSrc ? (
          <iframe
            key={viewer}
            title="The Ethical Researcher’s Guide to AI — presentation"
            src={embedSrc}
            loading="lazy"
            referrerPolicy="no-referrer-when-downgrade"
            allowFullScreen
          />
        ) : (
          <div className="ethical-ai-pptx-placeholder">Loading presentation…</div>
        )}
      </div>
      <p className="ethical-ai-pptx-meta">
        <a href={downloadLocal} download className="ethical-ai-pptx-download">
          Download .pptx
        </a>
        {isLocalhost ? (
          <a href={`${PRODUCTION_ORIGIN}${pptxPath}`} className="ethical-ai-pptx-download" rel="noreferrer">
            Download from production
          </a>
        ) : null}
        <span className="ethical-ai-pptx-hint">
          Blank frame? Add the file to <code>public/guides/ethical-researcher-guide-ai-2026.pptx</code> on your machine,
          or set <code>REACT_APP_ETHICAL_AI_PPTX_URL</code> to a public HTTPS link. Try both viewers above.
        </span>
      </p>
    </div>
  );
}

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
    <SeoGuideShell
      headline={deck.title}
      subhead={deck.subtitle}
    >
      <div className="ethical-ai-deck-main">
        <section className="seo-guide-card ethical-ai-hero-compact">
          <p style={{ margin: 0, lineHeight: 1.65 }}>
            <cite>The Ethical Researcher&apos;s Guide to AI</cite> (2026)—same slides for every topic below.
            Footer links and pills match common searches; the deck is identical each time. {deck.sourceNote}{' '}
            <Link to="/academic-ai-remover">Academic AI review</Link> ·{' '}
            <Link to="/journal-matching">Journal matching</Link>.
          </p>
        </section>

        <section
          id="ethical-ai-deck-anchor"
          className="seo-guide-card ethical-ai-slide-focus"
          aria-labelledby="ethical-ai-embed-heading"
        >
          <h2 id="ethical-ai-embed-heading">Slide presentation</h2>
          <p style={{ marginTop: 0, color: 'var(--muted-text)', fontSize: '0.9rem' }}>
            Your <strong>designed slides</strong> (layout, theme, images) show only in the embedded viewer above—or use
            &quot;Open full presentation&quot;. The collapsible section at the bottom is plain text for search and
            accessibility, not a pixel-perfect copy of PowerPoint. On localhost the embed uses {PRODUCTION_ORIGIN}.
          </p>
          <PptxDeckEmbed pptxPath={ETHICAL_AI_PPTX_PATH} />
          <details className="ethical-ai-copy-hint">
            <summary>Slides look blank? Add the .pptx once (use your Terminal)</summary>
            <p style={{ margin: '12px 0 0', fontSize: '0.9rem', lineHeight: 1.65, color: 'var(--section-text)' }}>
              Run these from <strong>Terminal</strong>. Your project lives under <strong>Desktop/GAPLY</strong> — not in
              your home folder — so <code>cd gaply-react-frontend</code> alone will fail until you are in the right place.
            </p>
            <pre className="ethical-ai-copy-hint__cmd">
              cd ~/Desktop/GAPLY/gaply-react-frontend{'\n'}
              npm run copy-ethical-pptx
            </pre>
            <p style={{ margin: '10px 0 0', fontSize: '0.9rem', lineHeight: 1.65, color: 'var(--section-text)' }}>
              Or run this <strong>from any directory</strong> (uses full paths):
            </p>
            <pre className="ethical-ai-copy-hint__cmd">
              {`node ~/Desktop/GAPLY/gaply-react-frontend/scripts/copy-ethical-ppt-to-public.js "$HOME/Desktop/The Ethical Researcher's Guide to AI (2).pptx"`}
            </pre>
            <p style={{ margin: '8px 0 0', fontSize: '0.9rem', lineHeight: 1.65, color: 'var(--section-text)' }}>
              If GAPLY is not on your Desktop, replace <code>~/Desktop/GAPLY</code> with your real folder. That copies the
              deck to <code>public/guides/ethical-researcher-guide-ai-2026.pptx</code>. Or drag the file there manually.
              Optional: <code>REACT_APP_ETHICAL_AI_PPTX_URL</code> in <code>.env.local</code> for a hosted URL.
            </p>
          </details>
        </section>

        <nav className="ethical-ai-topic-pills" aria-label="Search topics">
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

        {ETHICAL_AI_GUIDE_TOPICS.map((t) => (
          <section
            key={t.slug}
            id={`ethical-ai-topic-${t.slug}`}
            className="seo-guide-card"
            aria-labelledby={`ethical-ai-seo-h-${t.slug}`}
          >
            <h2 id={`ethical-ai-seo-h-${t.slug}`}>{t.label}</h2>
            <p style={{ marginBottom: 0 }}>{t.intro}</p>
          </section>
        ))}

        <details className="ethical-ai-transcript">
          <summary>Full text transcript (all slides — for search &amp; accessibility)</summary>

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
