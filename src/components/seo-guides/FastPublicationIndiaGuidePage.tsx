import React, { useEffect, useMemo } from 'react';
import { Link } from 'react-router-dom';
import './SeoGuidePages.css';
import GaplyResearchGuideShell from './GaplyResearchGuideShell';
import JournalDirectorySection from './JournalDirectorySection';
import { groupByCategoryOrdered, isRelativelyFastReview, slugifySegment, type ScopusJournal } from './scopusGuideHelpers';
import scopusDirectory from '../../data/scopusDirectory.json';
import { RESEARCH_GUIDE_SEO_PHRASES, researchGuideKeywordsJoined } from '../../seo/researchGuideSeoPhrases';

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
    const keywords = researchGuideKeywordsJoined();
    const faqLd = {
      '@context': 'https://schema.org',
      '@graph': [
        {
          '@type': 'BreadcrumbList',
          '@id': `${PAGE_URL}#breadcrumb`,
          itemListElement: [
            { '@type': 'ListItem', position: 1, name: 'Home', item: 'https://www.gaply.in/' },
            {
              '@type': 'ListItem',
              position: 2,
              name: 'Fast publication Scopus journals India',
              item: PAGE_URL,
            },
          ],
        },
        {
          '@type': 'WebPage',
          '@id': `${PAGE_URL}#webpage`,
          url: PAGE_URL,
          name: 'Fast publication Scopus-indexed journals for researchers in India (verify timelines)',
          description:
            'Educational guide for Indian PhD and postgraduate researchers: realistic peer-review timelines, thesis planning, predatory journal avoidance, and a reference list of titles with shorter stated review windows.',
          keywords,
          inLanguage: 'en',
          isPartOf: { '@type': 'WebSite', name: 'Gaply', url: 'https://www.gaply.in' },
        },
        {
          '@type': 'FAQPage',
          '@id': `${PAGE_URL}#faq`,
          url: PAGE_URL,
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
            {
              '@type': 'Question',
              name: 'Where can I find the latest UGC CARE List Group I and II PDF for 2026?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'The University Grants Commission (India) publishes official CARE lists and updates on its website. Download the current PDF from the UGC directly and verify each journal against the live list. This Gaply guide does not host government PDFs.',
              },
            },
            {
              '@type': 'Question',
              name: 'How does journal quartile analysis (Q1 vs Q2) relate to PhD thesis submission?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Quartiles summarize citation-based groupings and depend on the database and subject category. Universities set their own acceptable journal tiers for theses. Follow your institution’s doctoral regulations and verify outlets in official databases rather than informal lists alone.',
              },
            },
            {
              '@type': 'Question',
              name: 'How can UGC guidelines help identify predatory journals?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'UGC and university notices warn against deceptive publishers. Treat unrealistic fast-publication guarantees, opaque peer review, and unverifiable indexing claims as red flags. Always confirm Scopus status and publisher legitimacy on official sites before you submit or pay fees.',
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

  const sidebarNav = [
    { href: '#fast-india-context', label: 'Context' },
    { href: '#fast-india-built', label: 'How the list is built' },
    { href: '#fast-india-browse', label: 'Browse by field' },
    { href: '#fast-india-dir-h', label: 'Directory' },
  ];

  return (
    <GaplyResearchGuideShell
      heroBadge="India timelines · thesis planning"
      headline="Fast publication Scopus-indexed journals for researchers in India"
      subhead="Plan thesis and graduation timelines with realistic peer-review expectations—then verify every detail on the official journal site."
      sidebarTitle="Fast publication (India)"
      sidebarMeta="Stated review windows · verify live"
      sidebarNav={sidebarNav}
      seoSearchPhrases={RESEARCH_GUIDE_SEO_PHRASES}
      siblingGuide={{
        to: '/guides/scopus-indexed-journals-low-apc',
        label: 'Scopus & APC guide',
      }}
    >
      <article>
        <section id="fast-india-context" className="seo-guide-card">
          <h2>Context: why “fast publication” shows up in Indian searches</h2>
          <p>
            Queries like <strong>fast publication journals for engineering in India</strong> or similar subject variants
            usually reflect <strong>university deadlines</strong>, credit requirements, or viva timing—not a separate class
            of “India-only” Scopus journals. The same international journals accept authors from India; what matters is
            whether the journal’s scope fits your work and whether its <strong>real</strong> workflow matches your
            calendar.
          </p>
          <p>
            Avoid anyone who promises guaranteed acceptance or implausibly short peer review. Cross-check indexing in
            Scopus (and your institution’s approved list, if applicable) and read recent issues for turnaround patterns.
          </p>
        </section>

        <section id="fast-india-built" className="seo-guide-card">
          <h2>How we built the list below</h2>
          <p style={{ margin: 0 }}>
            We filtered the same static reference directory used across Gaply guides to titles whose{' '}
            <strong>stated</strong> peer-review window in the source text looked relatively short (for example, rapid review
            labels or upper bounds of roughly eight weeks in parsed ranges). That is a <strong>heuristic only</strong>
            —production, revisions, and backlog can add months. {fastJournals.length} titles match this filter out of{' '}
            {allJournals.length} in the reference set.
          </p>
        </section>

        <section id="fast-india-browse" className="seo-guide-card">
          <p style={{ margin: 0, textAlign: 'center' }}>
            Browse by field: jump to Health Sciences, Life Sciences, Physical Sciences, Engineering &amp; Technology,
            Social Sciences, Arts &amp; Humanities, Environmental Sciences, or Business &amp; Economics—then open the
            subcategory that matches your topic (for example oncology, computer science, economics).
          </p>
        </section>

        <nav id="fast-india-toc" className="seo-guide-toc" aria-label="Browse by field">
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
              Subjects are grouped by broad field and specialty labels from the source directory. For APC planning and a
              full title list, see{' '}
              <Link to="/guides/scopus-indexed-journals-low-apc">Scopus journals and APC verification</Link>.
            </p>
          }
        />
      </article>
    </GaplyResearchGuideShell>
  );
};

export default FastPublicationIndiaGuidePage;
