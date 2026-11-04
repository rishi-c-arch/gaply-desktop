// Gaply — the timing counter's line in the window's footer (Research OS, Phase
// 8Q, decision 462). Shown only while statsTiming and statsLayout are both on.
import React, { useEffect, useState } from 'react';
import type { GridTiming } from './gridTiming';
import { timingSentence } from './windowWording';

export const TimingLine: React.FC<{ timing: GridTiming }> = ({ timing }) => {
  const [figures, setFigures] = useState(() => timing.read());
  useEffect(() => {
    const t = setInterval(() => setFigures(timing.read()), 500);
    return () => clearInterval(t);
  }, [timing]);
  return <span data-testid="sw-timing">{timingSentence(figures)}</span>;
};
