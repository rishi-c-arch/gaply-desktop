import React from 'react';
import { Link } from 'react-router-dom';
import type { ScopusJournal } from './scopusGuideHelpers';
import { groupByCategoryOrdered, slugifySegment } from './scopusGuideHelpers';

interface JournalDirectorySectionProps {
  journals: ScopusJournal[];
  sectionIdPrefix: string;
  heading: string;
  intro: React.ReactNode;
}

function countTitlesInCategory(subMap: Map<string, ScopusJournal[]>): number {
  return Array.from(subMap.values()).reduce((n, arr) => n + arr.length, 0);
}

const JournalDirectorySection: React.FC<JournalDirectorySectionProps> = ({
  journals,
  sectionIdPrefix,
  heading,
  intro,
}) => {
  const grouped = groupByCategoryOrdered(journals);
  const hubId = `${sectionIdPrefix}-ref-hub`;

  return (
    <section className="seo-guide-directory" aria-labelledby={`${sectionIdPrefix}-h`}>
      {grouped.length > 0 ? (
        <div className="grg-ref-hub" aria-labelledby={`${hubId}-title`}>
          <header className="grg-ref-hub__head">
            <h2 id={`${hubId}-title`} className="grg-ref-hub__title">
              Reference directory
            </h2>
            <p className="grg-ref-hub__kicker">Categorization of {journals.length} verified titles</p>
          </header>
          <div className="grg-ref-hub__grid">
            {(() => {
              const [featName, featMap] = grouped[0];
              const featN = countTitlesInCategory(featMap);
              const featAnchor = `${sectionIdPrefix}-${slugifySegment(featName)}`;
              const minis = grouped.slice(1, 5);
              return (
                <>
                  <a className="grg-ref-hub__featured" href={`#${featAnchor}`}>
                    <span className="grg-ref-hub__featured-name">{featName}</span>
                    <span className="grg-ref-hub__featured-desc">
                      Verified titles grouped by subcategory below—always confirm APC, indexing, and timelines on each
                      official journal site.
                    </span>
                    <span className="grg-ref-hub__featured-count" aria-hidden>
                      {featN}
                    </span>
                    <span className="grg-ref-hub__featured-cta">Explore category</span>
                  </a>
                  <div className="grg-ref-hub__minis">
                    {minis.map(([cat, sm]) => (
                      <a key={cat} className="grg-ref-hub__mini" href={`#${sectionIdPrefix}-${slugifySegment(cat)}`}>
                        <span className="grg-ref-hub__mini-name">{cat}</span>
                        <span className="grg-ref-hub__mini-n">{countTitlesInCategory(sm)} journals</span>
                      </a>
                    ))}
                  </div>
                </>
              );
            })()}
          </div>
          <p className="grg-ref-hub__footnote">Use the accordions below for the complete list and outbound links.</p>
        </div>
      ) : null}

      <div className="seo-guide-card">
        <h2 id={`${sectionIdPrefix}-h`}>{heading}</h2>
        <div>{intro}</div>
      </div>

      {grouped.map(([category, subMap]) => {
        const catId = `${sectionIdPrefix}-${slugifySegment(category)}`;
        return (
          <details key={category} className="seo-guide-directory__category">
            <summary id={catId}>
              {category}
              <span style={{ fontWeight: 400, color: 'var(--muted-text)' }}>
                {' '}
                ({Array.from(subMap.values()).reduce((n, arr) => n + arr.length, 0)} titles)
              </span>
            </summary>
            <div className="seo-guide-directory__body">
              {Array.from(subMap.entries()).map(([subcategory, list]) => {
                const subId = `${catId}-${slugifySegment(subcategory)}`;
                return (
                  <div key={subcategory} className="seo-guide-subblock">
                    <h3 id={subId}>{subcategory}</h3>
                    <ul className="seo-guide-jlist" aria-label={`Journals: ${subcategory}`}>
                      {list.map((j) => (
                        <li key={`${j.title}-${j.website}`} className="seo-guide-jitem">
                          <strong>
                            <a href={j.website} rel="noopener noreferrer">
                              {j.title}
                            </a>
                          </strong>
                          <span className="seo-guide-jmeta">
                            {j.issnLine}
                            {j.publisher ? ` · Publisher: ${j.publisher}` : ''}
                            {j.reviewTime ? ` · Stated review window: ${j.reviewTime}` : ''}
                          </span>
                          {j.scope ? (
                            <span className="seo-guide-jmeta">Scope: {j.scope}</span>
                          ) : null}
                        </li>
                      ))}
                    </ul>
                  </div>
                );
              })}
            </div>
          </details>
        );
      })}

      <p
        style={{
          textAlign: 'center',
          marginTop: 28,
          fontSize: '0.95rem',
          color: 'var(--muted-text)',
        }}
      >
        Need a journal that fits your manuscript? Try{' '}
        <Link to="/journal-matching">Gaply journal matching</Link>.
      </p>
    </section>
  );
};

export default JournalDirectorySection;
