import { useState, useEffect, RefObject } from 'react';

/**
 * Intersection Observer hook - use to pause 3D/animation when section is not visible.
 */
export function useInView(ref: RefObject<HTMLElement | null>, options?: { threshold?: number }): boolean {
  const [inView, setInView] = useState(false);
  const threshold = options?.threshold ?? 0.1;

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => setInView(entry.isIntersecting),
      { threshold }
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref, threshold]);

  return inView;
}
