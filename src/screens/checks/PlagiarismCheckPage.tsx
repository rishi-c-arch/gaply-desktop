// Gaply — Plagiarism Check (F7). Local, free: self-plagiarism, internal
// duplication, paraphrase, verbatim — via the per-session isolated vector store.
// A clearly-labeled deep web/database check (Copyleaks-class) is teased behind
// the swappable seam: paid + online, routed via the proxy (no client-side key,
// no manuscript bytes on the client).
import React, { useMemo, useState } from 'react';
import { Badge, Button, Card } from '../../design-system';
import CheckScreen from './CheckScreen';
import { CheckBridge, TauriCheckBridge } from './checkBridge';
import { plagiarismToReport } from './adapters';
import { CopyleaksClient, DeepCheckResult, MockCopyleaksClient } from './copyleaks';
import { useGaplySession } from '../session/SessionProvider';

export interface PlagiarismCheckPageProps {
  bridge?: CheckBridge;
  copyleaks?: CopyleaksClient;
  /** Server-side reference for a deep check (never the manuscript text). */
  manuscriptRef?: string;
}

const PlagiarismCheckPage: React.FC<PlagiarismCheckPageProps> = ({
  bridge,
  copyleaks,
  manuscriptRef = 'session:current',
}) => {
  const { session } = useGaplySession();
  const b = useMemo(() => bridge ?? new TauriCheckBridge(), [bridge]);
  const deep = useMemo(() => copyleaks ?? new MockCopyleaksClient(), [copyleaks]);
  const [deepResult, setDeepResult] = useState<DeepCheckResult | null>(null);
  const [deepErr, setDeepErr] = useState<string | null>(null);

  const runDeep = async () => {
    setDeepErr(null);
    try {
      // The request carries a REFERENCE + options + consent — NEVER text.
      const res = await deep.checkDeep({
        manuscriptRef,
        scope: ['internet', 'scholar', 'repositories'],
        consent: true,
      });
      setDeepResult(res);
    } catch (e) {
      setDeepErr(e instanceof Error ? e.message : 'deep check failed');
    }
  };

  const deepPanel = (
    <Card title="Deep web/database check (Turnitin-class)" glass data-testid="deep-check">
      <p style={{ margin: '0 0 8px', color: 'var(--g-text-2)', fontSize: 13 }}>
        Compares against billions of web pages and published papers — coverage local checking can’t
        replicate. <Badge status="assessed">paid · online</Badge>{' '}
        Routed through the Gaply proxy: your manuscript never touches a client-side key.
      </p>
      {session ? (
        <Button variant="secondary" onClick={runDeep} data-testid="run-deep">
          Run deep check (Copyleaks)
        </Button>
      ) : (
        <Badge status="neutral">Sign in to enable the deep check</Badge>
      )}
      {deepErr && <p style={{ color: 'var(--g-text-3)', fontSize: 12, marginTop: 8 }} data-testid="deep-note">{deepErr}</p>}
      {deepResult && (
        <div style={{ marginTop: 8 }} data-testid="deep-result">
          <div className="gds-mono">
            {deepResult.provider}: {(deepResult.aggregateScore * 100).toFixed(0)}% aggregate ·{' '}
            {deepResult.matches.length} match(es)
          </div>
        </div>
      )}
    </Card>
  );

  return (
    <CheckScreen
      testid="plagiarism-check"
      title="Plagiarism Check"
      subtitle="Self-plagiarism, internal duplication, paraphrase & verbatim — on your device"
      tabs={['Overview', 'Plagiarism']}
      run={async (path) => plagiarismToReport(await b.plagiarism(path))}
      extra={deepPanel}
    />
  );
};

export default PlagiarismCheckPage;
