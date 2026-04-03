import React, { useEffect, useMemo } from 'react';
import { Link } from 'react-router-dom';
import SeoGuideShell from './SeoGuideShell';
import JournalDirectorySection from './JournalDirectorySection';
import { publisherOftenEmphasizesOA, type ScopusJournal } from './scopusGuideHelpers';
import scopusDirectory from '../../data/scopusDirectory.json';

const PAGE_URL = 'https://www.gaply.in/guides/scopus-indexed-journals-low-apc';
const LD_ID = 'ld-json-scopus-low-apc-guide';

const ScopusLowApcGuidePage: React.FC = () => {
  const journals = scopusDirectory.journals as ScopusJournal[];

  const oaEmphasisList = useMemo(
    () => journals.filter((j) => publisherOftenEmphasizesOA(j.publisher)),
    [journals]
  );

  useEffect(() => {
    const faqLd = {
      '@context': 'https://schema.org',
      '@graph': [
        {
          '@type': 'WebPage',
          '@id': `${PAGE_URL}#webpage`,
          url: PAGE_URL,
          name: 'Scopus-indexed journals and Article Processing Charges (APC): how to find lower-fee options',
          description:
            'Educational guide for researchers: Scopus indexing, hybrid vs open access, APC verification, and a reference directory of journal titles with official links.',
          isPartOf: { '@type': 'WebSite', name: 'Gaply', url: 'https://www.gaply.in' },
        },
        {
          '@type': 'FAQPage',
          '@id': `${PAGE_URL}#faq`,
          mainEntity: [
            {
              '@type': 'Question',
              name: 'Does Scopus list Article Processing Charges (APC)?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Scopus is an abstract and citation database. APCs, hybrid options, and waivers are set by each journal and publisher and can change. You should always confirm the current fee schedule on the official journal or publisher website before submitting.',
              },
            },
            {
              '@type': 'Question',
              name: 'How can I find Scopus-indexed journals with lower APC?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Compare fully open access journals, society journals with subsidized OA routes, hybrid journals with optional OA, and regional agreements if your institution has one. Use the official author instructions and APC pages—not third-party estimates alone.',
              },
            },
            {
              '@type': 'Question',
              name: 'Is the journal directory on this page an official Scopus export?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'No. It is a static reference list compiled for educational navigation. Indexing status, timelines, and fees must be verified with Scopus and the publisher at the time you submit.',
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
      headline="Scopus-indexed journals and lower Article Processing Charges (APC)"
      subhead="A practical, verification-first guide for researchers—plus a reference directory with official journal links."
    >
      <article>
        <section className="seo-guide-card">
          <h2>Why “low APC” needs a careful definition</h2>
          <p>
            Researchers often search for <strong>Scopus indexed journals with low APC</strong> when planning
            open-access publication. Scopus itself does not set fees; publishers and individual journals do,
            and those numbers change with policies, waivers, and hybrid (subscription + optional OA) models.
          </p>
          <p>
            This page does <strong>not</strong> claim specific dollar amounts for any title. Use the official
            journal site for APC, hybrid options, and any institutional discounts. Treat any static list—including
            ours—as a <strong>navigation aid</strong>, not a fee quote.
          </p>
        </section>

        <section className="seo-guide-card">
          <h2>Smarter ways to narrow lower-cost or transparent APC routes</h2>
          <ul>
            <li>
              <strong>Confirm OA model:</strong> fully OA, hybrid, or subscription-only—each affects whether you
              pay an APC at all.
            </li>
            <li>
              <strong>Check waivers:</strong> many publishers list waivers or discounts for authors in certain
              countries or with documented need.
            </li>
            <li>
              <strong>Compare society vs commercial brands:</strong> society journals sometimes offer different
              pricing or bundled membership benefits.
            </li>
            <li>
              <strong>Institutional agreements:</strong> your library may have transformative or OA agreements
              that reduce net author cost.
            </li>
            <li>
              <strong>Stay alert for predatory offers:</strong> unrealistically fast guarantees or unclear indexing
              claims are red flags—always cross-check Scopus and the publisher.
            </li>
          </ul>
          <p style={{ marginBottom: 0 }}>
            Use <Link to="/journal-matching">Gaply journal matching</Link> to align scope and audience with your
            manuscript, then verify APC on the official journal page.
          </p>
        </section>

        <section className="seo-guide-card">
          <h2>Reference titles: publishers that often publish APC details prominently</h2>
          <p>
            The subset below is drawn from the same reference directory and only flags{' '}
            <strong>publishers that commonly operate fully OA journals or clearly documented APC pages</strong>. It
            is <strong>not</strong> a ranking by fee and <strong>not</strong> financial advice—always confirm on the
            official site.
          </p>
          <ul className="seo-guide-oalist">
            {oaEmphasisList.map((j) => (
              <li key={`${j.title}-${j.website}-oa`}>
                <strong>
                  <a href={j.website} rel="noopener noreferrer">
                    {j.title}
                  </a>
                </strong>
                <span className="seo-guide-jmeta" style={{ display: 'block', marginTop: 6 }}>
                  {j.category} · {j.subcategory} · {j.issnLine}
                </span>
              </li>
            ))}
          </ul>
        </section>

        <JournalDirectorySection
          sectionIdPrefix="full-apc-dir"
          journals={journals}
          heading="Full reference directory (Scopus-style titles, verify before you submit)"
          intro={
            <p style={{ margin: 0 }}>
              {scopusDirectory.generatedNote} Listed titles: {scopusDirectory.journalCount}. For timelines oriented
              toward degree completion pressures in India, see{' '}
              <Link to="/guides/fast-publication-scopus-journals-india">
                fast publication Scopus journals for researchers in India
              </Link>
              .
            </p>
          }
        />
      </article>
    </SeoGuideShell>
  );
};

export default ScopusLowApcGuidePage;
