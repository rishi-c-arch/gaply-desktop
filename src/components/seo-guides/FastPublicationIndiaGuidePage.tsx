import React, { useEffect, useMemo } from 'react';
import { Link } from 'react-router-dom';
import SeoGuideShell from './SeoGuideShell';
import JournalDirectorySection from './JournalDirectorySection';
import { groupByCategoryOrdered, isRelativelyFastReview, slugifySegment, type ScopusJournal } from './scopusGuideHelpers';
import scopusDirectory from '../../data/scopusDirectory.json';

const PAGE_URL = 'https://www.gaply.in/guides/fast-publication-scopus-journals-india';
const LD_ID = 'ld-json-fast-pub-india-guide';

const FastPublicationIndiaGuidePage: React.FC = () => {
  const allJournals = scopusDirectory.journals as ScopusJournal[];
  const fastJournals = useMemo(
    () => allJournals.filter((j) => isRelativelyFastReview(j.reviewTime)),
    [allJournals]
  );

  const tocLinks = useMemo(() => {
    const grouped = groupByCategoryOrdered(fastJournals);
    return grouped.map(([cat]) => ({
      label: cat,
      href: `#fast-india-dir-${slugifySegment(cat)}`,
    }));
  }, [fastJournals]);

  useEffect(() => {
    const faqLd = {
      '@context': 'https://schema.org',
      '@graph': [
        {
          '@type': 'WebPage',
          '@id': `${PAGE_URL}#webpage`,
          url: PAGE_URL,
          name: 'Fast publication Scopus-indexed journals for researchers in India (verify timelines)',
          description:
            'Educational guide for Indian PhD and postgraduate researchers: realistic peer-review timelines, thesis planning, predatory journal avoidance, and a reference list of titles with shorter stated review windows.',
          isPartOf: { '@type': 'WebSite', name: 'Gaply', url: 'https://www.gaply.in' },
        },
        {
          '@type': 'FAQPage',
          '@id': `${PAGE_URL}#faq`,
          mainEntity: [
            {
              '@type': 'Question',
              name: 'Are there special “fast publication” Scopus journals only for India?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Indexing and peer review are governed by each journal worldwide. Indian researchers often search for faster routes because of university thesis deadlines; the same journals apply to authors globally. Always verify current review and production timelines on the official journal website.',
              },
            },
            {
              '@type': 'Question',
              name: 'Why do Indian PhD students search for fast publication journals?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Coursework, synopsis, and viva schedules can create pressure to show accepted or published work. Planning backward from your official deadline—and choosing realistic journals—reduces risk compared to chasing unrealistic “guaranteed fast” offers.',
              },
            },
            {
              '@type': 'Question',
              name: 'How should I use the directory on this page?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'This page lists reference titles where the source directory recorded relatively short stated peer-review windows. Scopus status, exact timelines, and APCs change. Confirm everything on Scopus and the publisher before submitting.',
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
    return () => {
      document.getElementById(LD_ID)?.remove();
    };
  }, []);

  return (
    <SeoGuideShell
      headline="Fast publication Scopus-indexed journals for researchers in India"
      subhead="Plan thesis and graduation timelines with realistic peer-review expectations—then verify every detail on the official journal site."
    >
      <article>
        <section className="seo-guide-card">
          <h2>Context: why “fast publication” shows up in Indian searches</h2>
          <p>
            Queries like <strong>fast publication journals for engineering in India</strong> or similar subject
            variants usually reflect <strong>university deadlines</strong>, credit requirements, or viva timing—not
            a separate class of “India-only” Scopus journals. The same international journals accept authors from
            India; what matters is whether the journal’s scope fits your work and whether its{' '}
            <strong>real</strong> workflow matches your calendar.
          </p>
          <p>
            Avoid anyone who promises guaranteed acceptance or implausibly short peer review. Cross-check indexing
            in Scopus (and your institution’s approved list, if applicable) and read recent issues for turnaround
            patterns.
          </p>
        </section>

        <section className="seo-guide-card">
          <h2>How we built the list below</h2>
          <p style={{ margin: 0 }}>
            We filtered the same static reference directory used across Gaply guides to titles whose{' '}
            <strong>stated</strong> peer-review window in the source text looked relatively short (for example, rapid
            review labels or upper bounds of roughly eight weeks in parsed ranges). That is a{' '}
            <strong>heuristic only</strong>—production, revisions, and backlog can add months.{' '}
            {fastJournals.length} titles match this filter out of {allJournals.length} in the reference set.
          </p>
        </section>

        <p className="seo-guide-card" style={{ marginBottom: 16, textAlign: 'center', padding: '14px 20px' }}>
          Browse by field: jump to Health Sciences, Life Sciences, Physical Sciences, Engineering &amp; Technology,
          Social Sciences, Arts &amp; Humanities, Environmental Sciences, or Business &amp; Economics—then open the
          subcategory that matches your topic (for example oncology, computer science, economics).
        </p>

        <nav className="seo-guide-toc" aria-label="Browse by field">
          {tocLinks.map(({ label, href }) => (
            <a key={label} href={href}>
              {label}
            </a>
          ))}
        </nav>

        <JournalDirectorySection
          sectionIdPrefix="fast-india-dir"
          journals={fastJournals}
          heading="Reference titles with shorter stated peer-review windows (verify on the journal site)"
          intro={
            <p style={{ margin: 0 }}>
              Subjects are grouped by broad field and specialty labels from the source directory. For APC planning
              and a full title list, see{' '}
              <Link to="/guides/scopus-indexed-journals-low-apc">Scopus journals and APC verification</Link>.
            </p>
          }
        />
      </article>
    </SeoGuideShell>
  );
};

export default FastPublicationIndiaGuidePage;
