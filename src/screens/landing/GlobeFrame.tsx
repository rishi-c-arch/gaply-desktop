// Gaply — robust mount for the landing globe. The landing is the FIRST screen a
// user sees, so a WebGL failure (no GPU, driver issue, remote/VM) must NEVER
// crash it. We (1) check for WebGL up front and (2) wrap the three.js canvas in
// an error boundary; either way we fall back to a static styled orb inside the
// same frame. Keeps Globe.tsx itself clean/uncluttered for restyling.
import React, { Suspense } from 'react';
import Globe from './Globe';

function webglAvailable(): boolean {
  try {
    const canvas = document.createElement('canvas');
    return Boolean(
      window.WebGLRenderingContext &&
        (canvas.getContext('webgl') || canvas.getContext('experimental-webgl'))
    );
  } catch {
    return false;
  }
}

/** Static fallback that keeps the frame looking intentional (not empty/broken)
 *  when the 3D globe can't render. */
const GlobeFallback: React.FC = () => (
  <div className="gpl-landing__globe-fallback" data-testid="landing-globe-fallback" aria-hidden />
);

class GlobeBoundary extends React.Component<
  { children: React.ReactNode; fallback: React.ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch(err: unknown) {
    // Don't let a WebGL/renderer error bubble up and blank the launch screen.
    // eslint-disable-next-line no-console
    console.warn('[landing] globe failed to render; showing static fallback:', err);
  }
  render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}

const GlobeFrame: React.FC<{ light?: boolean }> = ({ light = false }) => {
  if (typeof window !== 'undefined' && !webglAvailable()) return <GlobeFallback />;
  return (
    <GlobeBoundary fallback={<GlobeFallback />}>
      <Suspense fallback={<GlobeFallback />}>
        <Globe light={light} />
      </Suspense>
    </GlobeBoundary>
  );
};

export default GlobeFrame;
