import React from 'react';

/**
 * Decorative SVG DNA-style double helix for section headers.
 * Two sine-wave paths, purple, low opacity. No Three.js.
 */
export default function DNAAccent({
  className = '',
  style = {},
}: {
  className?: string;
  style?: React.CSSProperties;
}) {
  const w = 60;
  const h = 300;
  const freq = 0.03;
  const amp = 12;
  const points1: string[] = [];
  const points2: string[] = [];
  for (let y = 0; y <= h; y += 4) {
    const x1 = w / 2 + Math.sin(y * freq) * amp;
    const x2 = w / 2 + Math.sin(y * freq + Math.PI) * amp;
    points1.push(`${x1},${y}`);
    points2.push(`${x2},${y}`);
  }

  return (
    <svg
      className={className}
      width={w}
      height={h}
      viewBox={`0 0 ${w} ${h}`}
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      style={{ opacity: 0.15, ...style }}
      aria-hidden="true"
    >
      <path
        d={`M ${points1.join(' L ')}`}
        stroke="var(--accent-purple, #8b5cf6)"
        strokeWidth="1.5"
        fill="none"
      />
      <path
        d={`M ${points2.join(' L ')}`}
        stroke="var(--accent-purple, #8b5cf6)"
        strokeWidth="1.5"
        fill="none"
      />
    </svg>
  );
}
