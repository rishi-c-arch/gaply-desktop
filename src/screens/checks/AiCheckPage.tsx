// Gaply — AI Check (F7). Local, free: perplexity + burstiness, per-section risk.
// The MANDATORY uncertainty disclaimer rides on every result (surfaced verbatim
// from the Rust agent). The PerplexityModel seam means the future fine-tuned SLM
// replaces the interim HeuristicModel with NO UI change.
import React, { useMemo } from 'react';
import CheckScreen from './CheckScreen';
import { CheckBridge, TauriCheckBridge } from './checkBridge';
import { aiToReport } from './adapters';

export interface AiCheckPageProps {
  bridge?: CheckBridge;
}

const AiCheckPage: React.FC<AiCheckPageProps> = ({ bridge }) => {
  const b = useMemo(() => bridge ?? new TauriCheckBridge(), [bridge]);
  return (
    <CheckScreen
      testid="ai-check"
      title="AI Check"
      subtitle="Per-section AI-writing signal (perplexity + burstiness) — statistical, not proof"
      tabs={['Overview', 'AI Risk']}
      run={async (path) => aiToReport(await b.ai(path))}
    />
  );
};

export default AiCheckPage;
