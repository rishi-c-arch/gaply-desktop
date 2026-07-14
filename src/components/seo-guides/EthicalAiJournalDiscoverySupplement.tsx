import React, { useMemo } from 'react';
import { Link } from 'react-router-dom';
import JournalDirectorySection from './JournalDirectorySection';
import { publisherOftenEmphasizesOA, type ScopusJournal } from './scopusGuideHelpers';
import scopusDirectory from '../../data/scopusDirectory.json';
import { RESEARCH_GUIDE_SEO_PHRASES } from '../../seo/researchGuideSeoPhrases';

/**
 * Journal discovery / APC / UGC-adjacent SEO block for the ethical AI guide.
 * Phrases match researchGuideSeoPhrases and the standalone Scopus APC guide.
 */
const EthicalAiJournalDiscoverySupplement: React.FC = () => {
  const journals = scopusDirectory.journals as ScopusJournal[];

  const oaEmphasisList = useMemo(
    () => journals.filter((j) => publisherOftenEmphasizesOA(j.publisher)),
    [journals]
  );

  return (
    <section
      id="ethical-ai-journal-discovery-supplement"
      className="ethical-ai-journal-supplement"
      aria-labelledby="ethical-ai-related-searches-h"
    >
      <div className="seo-guide-card">
        <h2 id="ethical-ai-related-searches-h">Related searches this guide supports</h2>
        <p className="ethical-ai-journal-supplement__lede">
          Ethical writing and publication choices overlap with where you submit, what you might pay in open access, and how
          you avoid predatory traps. The phrases below mirror common researcher searches; use them as navigation, then
          verify every fee and index status on official sites.
        </p>
        <ul className="ethical-ai-journal-supplement__phrase-list">
          {RESEARCH_GUIDE_SEO_PHRASES.map((phrase) => (
            <li key={phrase}>{phrase}</li>
          ))}
        </ul>
        <p className="ethical-ai-journal-supplement__more">
          For full verification-first walkthroughs, see{' '}
          <Link to="/guides/scopus-indexed-journals-low-apc">Scopus indexed journals with low APC (Article Processing Charge)</Link>
          {' and '}
          <Link to="/guides/fast-publication-scopus-journals-india">
            fast publication journals for researchers in India
          </Link>
          .
        </p>
      </div>

      <section id="ethical-ai-apc-why" className="seo-guide-card">
        <h2>Why “low APC” needs a careful definition</h2>
        <p>
          Researchers often search for <strong>Scopus indexed journals with low APC</strong> when planning open-access
          publication. Scopus itself does not set fees; publishers and individual journals do, and those numbers change
          with policies, waivers, and hybrid (subscription + optional OA) models.
        </p>
        <p>
          This page does <strong>not</strong> claim specific dollar amounts for any title. Use the official journal site
          for APC, hybrid options, and any institutional discounts. Treat any static list—including ours—as a{' '}
          <strong>navigation aid</strong>, not a fee quote.
        </p>
      </section>

      <section id="ethical-ai-apc-smarter" className="seo-guide-card">
        <h2>Smarter ways to narrow lower-cost or transparent APC routes</h2>
        <ul>
          <li>
            <strong>Confirm OA model:</strong> fully OA, hybrid, or subscription-only—each affects whether you pay an APC
            at all.
          </li>
          <li>
            <strong>Check waivers:</strong> many publishers list waivers or discounts for authors in certain countries or
            with documented need.
          </li>
          <li>
            <strong>Compare society vs commercial brands:</strong> society journals sometimes offer different pricing or
            bundled membership benefits.
          </li>
          <li>
            <strong>Institutional agreements:</strong> your library may have transformative or OA agreements that reduce
            net author cost.
          </li>
          <li>
            <strong>Stay alert for predatory offers:</strong> unrealistically fast guarantees or unclear indexing claims
            are red flags—always cross-check Scopus and the publisher.
          </li>
        </ul>
        <p style={{ marginBottom: 0 }}>
          Align scope and audience with your manuscript, then verify APC on the
          official journal page.
        </p>
      </section>

      <section id="ethical-ai-apc-oa-list" className="seo-guide-card">
        <h2>Reference titles: publishers that often publish APC details prominently</h2>
        <p>
          The subset below is drawn from the same reference directory and only flags{' '}
          <strong>publishers that commonly operate fully OA journals or clearly documented APC pages</strong>. It is{' '}
          <strong>not</strong> a ranking by fee and <strong>not</strong> financial advice—always confirm on the official
          site.
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
        sectionIdPrefix="ethical-ai-full-dir"
        journals={journals}
        heading="Full reference directory (Scopus-style titles, verify before you submit)"
        intro={
          <p style={{ margin: 0 }}>
            {scopusDirectory.generatedNote} Listed titles: {scopusDirectory.journalCount}. For timelines oriented toward
            degree completion pressures in India, see{' '}
            <Link to="/guides/fast-publication-scopus-journals-india">
              fast publication Scopus journals for researchers in India
            </Link>
            .
          </p>
        }
      />
    </section>
  );
};

export default EthicalAiJournalDiscoverySupplement;
