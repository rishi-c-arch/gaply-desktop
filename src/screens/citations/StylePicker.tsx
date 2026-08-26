// Gaply — Citation Manager style picker (Set 2c-ii). A searchable combobox over
// ALL ~2,856 bundled CSL styles: the curated 8 pinned when the search is empty,
// the full catalog (manifest, lazy-loaded once + cached by the engine) filtered
// by title/id when searching. Results are CAPPED (never 2,856 DOM nodes). The
// actual .csl load happens on select (the page's prepareStyle) — this component
// only chooses an id.
import React, { useEffect, useMemo, useState } from 'react';
import { CSL_STYLES } from './formatCitation';
import { listStyles, StyleEntry } from './cslEngine';

/** Never render more than this many matches — a search for a common word
 *  ("journal") matches hundreds; we show the top slice, not all of them. */
export const MAX_STYLE_RESULTS = 50;

export interface StylePickerProps {
  value: string;
  onSelect: (id: string) => void;
  testid: string;
}

export const StylePicker: React.FC<StylePickerProps> = ({ value, onSelect, testid }) => {
  const [catalog, setCatalog] = useState<StyleEntry[] | null>(null);
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);

  // Lazy-load the manifest catalog (265 KB) on first open; the engine caches it.
  useEffect(() => {
    if (!open || catalog) return;
    let alive = true;
    listStyles()
      .then((s) => alive && setCatalog(s))
      .catch(() => alive && setCatalog([]));
    return () => {
      alive = false;
    };
  }, [open, catalog]);

  const currentLabel = useMemo(() => {
    const pinned = CSL_STYLES.find((s) => s.id === value);
    if (pinned) return pinned.label;
    return catalog?.find((s) => s.id === value)?.title ?? value;
  }, [value, catalog]);

  const results = useMemo((): StyleEntry[] => {
    const q = query.trim().toLowerCase();
    if (!q) return CSL_STYLES.map((s) => ({ id: s.id, title: s.label })); // the pinned 8
    return (catalog ?? [])
      .filter((s) => s.title.toLowerCase().includes(q) || s.id.toLowerCase().includes(q))
      .slice(0, MAX_STYLE_RESULTS);
  }, [query, catalog]);

  const pick = (id: string) => {
    onSelect(id);
    setOpen(false);
    setQuery('');
  };

  return (
    <div className="gds-style-picker" data-testid={testid}>
      <button
        type="button"
        className="gds-style-picker__current"
        data-testid={`${testid}-current`}
        onClick={() => setOpen((o) => !o)}
      >
        {currentLabel} ▾
      </button>
      {open && (
        <div className="gds-style-picker__pop">
          <input
            className="gds-cite-input"
            autoFocus
            // The placeholder carries the catalog count and is not a label;
            // this gives the input a stable accessible name that survives the
            // count changing. Additive — no behaviour or prop change.
            aria-label="Search citation styles"
            placeholder="Search 2,856 citation styles…"
            value={query}
            data-testid={`${testid}-search`}
            onChange={(e) => setQuery(e.target.value)}
          />
          {query && !catalog ? (
            <p className="gds-style-picker__note">Loading catalog…</p>
          ) : (
            <ul className="gds-style-picker__list" data-testid={`${testid}-results`}>
              {results.map((s) => (
                <li key={s.id}>
                  <button
                    type="button"
                    data-testid={`${testid}-opt-${s.id}`}
                    aria-current={s.id === value}
                    onClick={() => pick(s.id)}
                  >
                    {s.title}
                  </button>
                </li>
              ))}
              {query && results.length === 0 && (
                <li className="gds-style-picker__note" data-testid={`${testid}-empty`}>
                  No styles match “{query}”.
                </li>
              )}
            </ul>
          )}
        </div>
      )}
    </div>
  );
};
