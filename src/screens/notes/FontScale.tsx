// Gaply — Note Creator reading-size control ("Aa"). Three classy presets that
// scale the SERIF CONTENT type only (the scholarly reading/writing surfaces) —
// interface chrome keeps the design's fixed Hanken Grotesk sizes. Pure CSS:
// the choice sets data-anscale on every .an-root and persists locally. No
// state plumbing, no re-render, works from the dashboard and both editors.
import React, { useEffect, useState } from 'react';

export type AnScale = 'compact' | 'comfortable' | 'large';
const KEY = 'gaply-notes-fontscale';

const read = (): AnScale => {
  try {
    const v = localStorage.getItem(KEY);
    return v === 'compact' || v === 'large' ? v : 'comfortable';
  } catch {
    return 'comfortable';
  }
};

export const applyScale = (v: AnScale): void => {
  document.querySelectorAll('.an-root').forEach((el) => el.setAttribute('data-anscale', v));
};

const FontScale: React.FC = () => {
  const [scale, setScale] = useState<AnScale>(read);

  useEffect(() => {
    applyScale(scale);
    try { localStorage.setItem(KEY, scale); } catch { /* private mode — session only */ }
  }, [scale]);

  return (
    <div className="an-fontscale" data-testid="font-scale" title="Reading size">
      {(['compact', 'comfortable', 'large'] as AnScale[]).map((v, i) => (
        <button
          key={v}
          className={`an-fontscale-btn${scale === v ? ' an-fontscale-btn--on' : ''}`}
          style={{ fontSize: [11, 13, 15][i] }}
          data-testid={`font-scale-${v}`}
          onClick={() => setScale(v)}
          aria-label={`${v} reading size`}
        >
          Aa
        </button>
      ))}
    </div>
  );
};

export default FontScale;
