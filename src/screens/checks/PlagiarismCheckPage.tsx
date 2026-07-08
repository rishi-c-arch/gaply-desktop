// Gaply — Plagiarism Check (F7). Local, free: self-plagiarism, internal
// duplication, paraphrase, verbatim — via the per-session isolated vector
// store, computed on-device.
//
// The "deep" comprehensive check is NOT wired and shows an honest coming-soon
// state (gated by the `deepPlagiarism` flag, off by default). It will be
// powered by Gaply's own trained model — NO third-party service, and never a
// fabricated score or source.
import React, { useMemo } from 'react';
import { Badge, Button, Card } from '../../design-system';
import CheckScreen from './CheckScreen';
import { CheckBridge, TauriCheckBridge } from './checkBridge';
import { plagiarismToReport } from './adapters';
import { useFeatureFlag } from '../../config/Feature';

export interface PlagiarismCheckPageProps {
  bridge?: CheckBridge;
}

const PlagiarismCheckPage: React.FC<PlagiarismCheckPageProps> = ({ bridge }) => {
  const b = useMemo(() => bridge ?? new TauriCheckBridge(), [bridge]);
  const deepEnabled = useFeatureFlag('deepPlagiarism');

  // Honest state: the deep check is not built. No vendor name, no number, no
  // placeholder source. When the flag is on later, this is where the real
  // (Gaply-model-powered) result will render.
  const deepPanel = (
    <Card title="Deep plagiarism analysis" glass data-testid="deep-check">
      <p style={{ margin: '0 0 8px', color: 'var(--g-text-2)', fontSize: 13 }}>
        Broader coverage beyond your own documents and the shared corpus.{' '}
        <Badge status="neutral">coming soon</Badge>
      </p>
      <p style={{ margin: 0, color: 'var(--g-text-3)', fontSize: 12 }} data-testid="deep-note">
        Deep plagiarism analysis is coming soon. Local, on-device checks below are available now.
      </p>
      {deepEnabled && (
        // Flag on but still no real backend — keep it honest, do not fabricate.
        <Button variant="secondary" disabled style={{ marginTop: 8 }} data-testid="run-deep">
          Run deep analysis (coming soon)
        </Button>
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
