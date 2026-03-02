import { useState, useEffect } from 'react';

/**
 * Responsive breakpoint hook. Use for reducing particles / layout on small screens.
 */
export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() => {
    if (typeof window === 'undefined') return false;
    return window.matchMedia(query).matches;
  });

  useEffect(() => {
    const mql = window.matchMedia(query);
    const handler = (e: MediaQueryListEvent) => setMatches(e.matches);
    setMatches(mql.matches);
    mql.addEventListener('change', handler);
    return () => mql.removeEventListener('change', handler);
  }, [query]);

  return matches;
}

/** Particle count by viewport (for performance). */
export function getParticleCount(): number {
  if (typeof window === 'undefined') return 50;
  if (window.innerWidth < 768) return 30;
  if (window.innerWidth < 1024) return 50;
  return 100;
}
