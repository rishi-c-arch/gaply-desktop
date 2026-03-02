import { useState, useEffect, RefObject } from 'react';

/**
 * Returns true when the element is in view. Used for scroll-triggered animations.
 * Add class 'in-view' to the observed element when threshold is met.
 */
export function useIntersectionObserver(
  ref: RefObject<HTMLElement | null>,
  options?: { threshold?: number; rootMargin?: string }
): boolean {
  const [inView, setInView] = useState(false);
  const threshold = options?.threshold ?? 0.15;
  const rootMargin = options?.rootMargin ?? '0px';

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        const isIn = entry.isIntersecting;
        setInView(isIn);
        if (isIn) el.classList.add('in-view');
        else el.classList.remove('in-view');
      },
      { threshold, rootMargin }
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref, threshold, rootMargin]);

  return inView;
}
