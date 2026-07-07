// Gaply — Statistical Analysis Check (F7). 100% local, deterministic: t-test
// misuse for 3+ groups, p-value overclaiming, missing effect size / CI, small-n
// causal claims, data-vs-paper consistency. Presented as 🟢 mathematically
// certain — these require correction, not interpretation.
import React, { useMemo } from 'react';
import CheckScreen from './CheckScreen';
import { CheckBridge, TauriCheckBridge } from './checkBridge';
import { validationToReport } from './adapters';

export interface StatsCheckPageProps {
  bridge?: CheckBridge;
}

const StatsCheckPage: React.FC<StatsCheckPageProps> = ({ bridge }) => {
  const b = useMemo(() => bridge ?? new TauriCheckBridge(), [bridge]);
  return (
    <CheckScreen
      testid="stats-check"
      title="Statistical Analysis Check"
      subtitle="Deterministic statistical-reporting rules — mathematically certain, runs on device"
      tabs={['Overview', 'Statistics']}
      run={async (path, title) => validationToReport(await b.validation(path, title))}
    />
  );
};

export default StatsCheckPage;
