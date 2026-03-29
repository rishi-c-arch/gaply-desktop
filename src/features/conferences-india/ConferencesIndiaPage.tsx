import React, { useMemo, useState } from 'react';
import { useTheme } from '../../contexts/ThemeContext';
import { useConferencesIndia } from './useConferencesIndia';
import './ConferencesIndiaPage.css';

const ConferencesIndiaPage: React.FC = () => {
  const { theme } = useTheme();
  const { data, loading, error, reload } = useConferencesIndia();
  const [q, setQ] = useState('');
  const [discipline, setDiscipline] = useState('');

  const disciplineOptions = useMemo(() => {
    if (!data?.conferences?.length) return [];
    const set = new Set<string>();
    for (const c of data.conferences) {
      if (c.discipline?.trim()) set.add(c.discipline.trim());
    }
    return Array.from(set).sort((a, b) => a.localeCompare(b));
  }, [data]);

  const filtered = useMemo(() => {
    if (!data?.conferences) return [];
    let rows = data.conferences;
    if (discipline) {
      rows = rows.filter((c) => (c.discipline || '').trim() === discipline);
    }
    const s = q.trim().toLowerCase();
    if (!s) return rows;
    return rows.filter(
      (c) =>
        c.title.toLowerCase().includes(s) ||
        c.venue.toLowerCase().includes(s) ||
        c.date_text.toLowerCase().includes(s) ||
        c.description.toLowerCase().includes(s) ||
        (c.discipline || '').toLowerCase().includes(s) ||
        String(c.year).includes(s),
    );
  }, [data, q, discipline]);

  return (
    <div className="ci-page" data-theme={theme}>
      <div className="ci-page__noise" aria-hidden="true" />
      <div className="ci-page__bg" />

      <section className="ci-page__hero">
        <span className="ci-page__badge">Free resource</span>
        <h1 className="ci-page__title">Research conferences in India</h1>
        <p className="ci-page__subtitle">
          Upcoming and recent academic research events across major disciplines (STEM, medicine, social sciences, humanities,
          law, and multidisciplinary national programmes). We show roughly the last two calendar years through future editions—
          always confirm dates on each organizer’s site.
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
            <label className="ci-page__field-label" htmlFor="ci-discipline">
              Field
            </label>
            <select
              id="ci-discipline"
              className="ci-page__select"
              value={discipline}
              onChange={(e) => setDiscipline(e.target.value)}
              aria-label="Filter by research field"
            >
              <option value="">All fields</option>
              {disciplineOptions.map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </select>
            <input
              className="ci-page__search"
              type="search"
              placeholder="Search by name, city, field, year…"
              value={q}
              onChange={(e) => setQ(e.target.value)}
              aria-label="Filter conferences"
            />
            <span className="ci-page__count">
              {filtered.length} of {data.count} shown
              {data.cache_hit ? ' · cached' : ''}
            </span>
          </div>

          <div className="ci-page__list">
            {filtered.map((c) => (
              <article key={c.id} className="ci-card">
                <div className="ci-card__meta">
                  {c.year}
                  {c.is_upcoming ? ' · Upcoming / current year' : ' · Recent'}
                  {c.scholarship && c.scholarship !== 'No' && c.scholarship !== '—' ? ' · Funding' : ''}
                </div>
                <h2 className="ci-card__title">
                  <a href={c.url} target="_blank" rel="noopener noreferrer">
                    {c.title}
                  </a>
                </h2>
                {c.discipline ? <span className="ci-card__discipline">{c.discipline}</span> : null}
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
