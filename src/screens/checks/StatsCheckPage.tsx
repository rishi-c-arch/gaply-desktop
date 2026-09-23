// Gaply — Statistical Analysis Check (F7). 100% local, deterministic: t-test
// misuse for 3+ groups, p-value overclaiming, missing effect size / CI, small-n
// causal claims, data-vs-paper consistency. The DETECTION is deterministic and
// keeps the 🟢 tier for ordering; each RULE's label says only what its check
// did (§11 D217, D219). The "all rules passed" row still says "mathematically
// certain": what a clean pass claims is a separate question (D218, not started).
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
      subtitle="Deterministic pattern checks of statistical reporting — runs on device"
      tabs={['Overview', 'Statistics']}
      run={async (path, title) => validationToReport(await b.validation(path, title))}
    />
  );
};

export default StatsCheckPage;
