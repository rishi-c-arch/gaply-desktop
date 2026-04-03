import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import SeoGuideShell from './SeoGuideShell';
import { ETHICAL_AI_SLIDE_TITLES } from './ethicalAiSlideTitles';
import deck from '../../data/ethicalAiGuideSlides.json';

const PAGE_URL = 'https://www.gaply.in/guides/ethical-researcher-guide-ai-academic-writing';
const LD_ID = 'ld-json-ethical-ai-guide';

interface SlideRow {
  slide: number;
  id: string;
  body: string;
}

const EthicalAiResearchGuidePage: React.FC = () => {
  const slides = deck.slides as SlideRow[];
  const total = slides.length;
  const [activeSlide, setActiveSlide] = useState(1);
  const sectionRefs = useRef<(HTMLElement | null)[]>([]);

  const titleFor = useCallback(
    (index: number) => ETHICAL_AI_SLIDE_TITLES[index] ?? `Slide ${index + 1}`,
    []
  );

  const scrollToSlide = useCallback((n: number) => {
    const clamped = Math.min(Math.max(1, n), total);
    const el = document.getElementById(`ethical-ai-slide-${clamped}`);
    el?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    setActiveSlide(clamped);
  }, [total]);

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
            'Full slide-based guide for researchers on ethical AI use in academic writing, AI detection limits, Turnitin in India, humanizer myths, and literature reviews without misconduct.',
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
        <section className="seo-guide-card">
          <h2>Keywords researchers search for—and an ethical answer</h2>
          <p>
            Many people search for{' '}
            <strong>how to remove AI detection from a research paper in 2026</strong>,{' '}
            <strong>free AI rewriters to bypass Turnitin</strong>,{' '}
            <strong>best humanizer tools</strong>, whether{' '}
            <strong>ChatGPT text passes plagiarism checks in India</strong>, and{' '}
            <strong>how to use AI for a literature review without being flagged</strong>. This page
            addresses those queries directly: the responsible path is transparency, your own
            intellectual contribution, and institutional policy—not evasion.
          </p>
          <p>
            Below is the full text of the slide deck <cite>The Ethical Researcher&apos;s Guide to AI</cite>{' '}
            (Botanical Research Series, 2026 edition), adapted for the web so search engines and readers
            can access every slide. {deck.sourceNote}
          </p>
          <p style={{ marginBottom: 0 }}>
            For tools that help you review drafting integrity on your own work, see{' '}
            <Link to="/academic-ai-remover">Academic AI review</Link> and{' '}
            <Link to="/journal-matching">journal matching</Link>—always alongside your university&apos;s
            rules.
          </p>
        </section>

        <nav className="ethical-ai-toc" aria-label="Slide list">
          <details open>
            <summary>All slides (jump to any section)</summary>
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
              'This slide in the original presentation is primarily visual. Use the previous and next slides for the same chapter—the written guidance is covered in surrounding sections.';

            return (
              <section
                key={s.id}
                id={`ethical-ai-slide-${s.slide}`}
                className="seo-guide-card"
                aria-labelledby={`ethical-ai-h-${s.slide}`}
                ref={(el) => {
                  sectionRefs.current[i] = el;
                }}
              >
                <p style={{ fontSize: '0.85rem', color: 'var(--muted-text)', marginBottom: 8 }}>
                  Slide {s.slide} of {total}
                </p>
                <h2 id={`ethical-ai-h-${s.slide}`} style={{ fontSize: 'clamp(1.2rem, 3vw, 1.5rem)' }}>
                  {heading}
                </h2>
                <div className="ethical-ai-slide-body">{body}</div>
              </section>
            );
          })}
        </article>
      </div>

      <div className="ethical-ai-deck-toolbar" role="toolbar" aria-label="Slide navigation">
        <button
          type="button"
          disabled={activeSlide <= 1}
          onClick={() => scrollToSlide(activeSlide - 1)}
        >
          Previous slide
        </button>
        <span className="ethical-ai-deck-counter" aria-live="polite">
          Slide {activeSlide} / {total}
        </span>
        <button
          type="button"
          disabled={activeSlide >= total}
          onClick={() => scrollToSlide(activeSlide + 1)}
        >
          Next slide
        </button>
      </div>
    </SeoGuideShell>
  );
};

export default EthicalAiResearchGuidePage;
