// Gaply DS — the 3D globe at three scales, REUSING the existing
// src/components/ThreeJSGlobe.tsx untouched (R3F Canvas fills its container,
// so scale is purely a container concern). Fully offline: the globe already
// uses the bundled HDR (public/hdr/studio_small_03_1k.hdr) and a local
// texture — no CDN anywhere.
//
// Lazy-loaded so three.js never lands in a bundle chunk for screens that
// don't render a globe (and never loads in jsdom tests).
import React, { Suspense } from 'react';
import './globe.css';

const ThreeJSGlobe = React.lazy(() => import('../components/ThreeJSGlobe'));

export type GlobeScale = 'hero' | 'mark' | 'micro';

const SCALE_PX: Record<GlobeScale, { width: string; height: string }> = {
  hero: { width: '100%', height: '480px' }, // full hero section
  mark: { width: '40px', height: '40px' }, // rail brand mark
  micro: { width: '20px', height: '20px' }, // inline loader
};

// A globe that fails (a texture that will not load, a renderer error, a lazy
// chunk that does not arrive) leaves its box empty and the screen in place.
// Without it, drei's useTexture rethrows through R3F's Canvas to the app's
// root boundary, which replaces the whole screen (Phase 5B-4a, finding h).
// It shows nothing and logs nothing.
class GlobeBoundary extends React.Component<{ children: React.ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    return this.state.failed ? null : this.props.children;
  }
}

export interface GaplyGlobeProps {
  scale: GlobeScale;
  className?: string;
}

export const GaplyGlobe: React.FC<GaplyGlobeProps> = ({ scale, className = '' }) => (
  <div
    className={`gds-globe gds-globe--${scale} ${className}`}
    data-testid={`gds-globe-${scale}`}
    style={{
      ...SCALE_PX[scale],
      // mark/micro are decorative: no orbit interaction, no focus target
      pointerEvents: scale === 'hero' ? 'auto' : 'none',
      flex: 'none',
    }}
    aria-hidden={scale !== 'hero'}
  >
    <GlobeBoundary>
      <Suspense fallback={null}>
        <ThreeJSGlobe />
      </Suspense>
    </GlobeBoundary>
  </div>
);
