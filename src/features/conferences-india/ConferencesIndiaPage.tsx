import React, { useMemo, useState } from 'react';
import { useTheme } from '../../contexts/ThemeContext';
import { useConferencesIndia } from './useConferencesIndia';
import './ConferencesIndiaPage.css';

const ConferencesIndiaPage: React.FC = () => {
  const { theme } = useTheme();
  const { data, loading, error, reload } = useConferencesIndia();
  const [q, setQ] = useState('');

  const filtered = useMemo(() => {
    if (!data?.conferences) return [];
    const s = q.trim().toLowerCase();
    if (!s) return data.conferences;
    return data.conferences.filter(
      (c) =>
        c.title.toLowerCase().includes(s) ||
        c.venue.toLowerCase().includes(s) ||
        c.date_text.toLowerCase().includes(s) ||
        c.description.toLowerCase().includes(s) ||
        String(c.year).includes(s),
    );
  }, [data, q]);

  return (
    <div className="ci-page" data-theme={theme}>
      <div className="ci-page__noise" aria-hidden="true" />
      <div className="ci-page__bg" />

      <section className="ci-page__hero">
        <span className="ci-page__badge">Free resource</span>
        <h1 className="ci-page__title">Tech conferences in India</h1>
        <p className="ci-page__subtitle">
          Community-curated list of tech conferences held in or relevant to India. Sourced from open data;
          always verify dates and links before booking travel.
        </p>
        {data?.attribution && <p className="ci-page__attribution">{data.attribution}</p>}
      </section>

      {loading && (
        <div className="ci-page__state" role="status">
          Loading conferences…
        </div>
      )}

      {error && !loading && (
        <div className="ci-page__state">
          <p>Could not load the list right now.</p>
          <p style={{ fontSize: '0.9rem', opacity: 0.8 }}>{error}</p>
          <button type="button" className="ci-page__retry" onClick={() => reload()}>
            Try again
          </button>
        </div>
      )}

      {!loading && !error && data && (
        <>
          <div className="ci-page__toolbar">
            <input
              className="ci-page__search"
              type="search"
              placeholder="Search by name, city, year, date…"
              value={q}
              onChange={(e) => setQ(e.target.value)}
              aria-label="Filter conferences"
            />
            <span style={{ fontSize: '0.85rem', color: 'var(--text-secondary)' }}>
              {filtered.length} of {data.count} shown
              {data.cache_hit ? ' · cached' : ''}
            </span>
          </div>

          <div className="ci-page__list">
            {filtered.map((c) => (
              <article key={c.id} className="ci-card">
                <div className="ci-card__meta">
                  {c.year}
                  {c.scholarship && c.scholarship !== 'No' ? ' · Scholarship' : ''}
                </div>
                <h2 className="ci-card__title">
                  <a href={c.url} target="_blank" rel="noopener noreferrer">
                    {c.title}
                  </a>
                </h2>
                <div className="ci-card__row">
                  <strong>When:</strong> {c.date_text || '—'}
                </div>
                <div className="ci-card__row">
                  <strong>Where:</strong> {c.venue || '—'}
                </div>
                {c.description ? <p className="ci-card__desc">{c.description}</p> : null}
              </article>
            ))}
          </div>
        </>
      )}
    </div>
  );
};

export default ConferencesIndiaPage;
