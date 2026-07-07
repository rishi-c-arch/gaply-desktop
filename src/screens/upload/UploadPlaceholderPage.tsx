// Gaply — upload flow LANDING (placeholder; F5 builds the real upload +
// analysis pipeline here). Exists now so the F4 dashboard's entry points have
// a graceful destination: "Start a new analysis", the drag-and-drop target
// (arrives with the dropped file's name in route state), and onboarding's
// "Scan a sample manuscript" (?sample=1 → bundled sample path).
import React from 'react';
import { Link, useLocation, useSearchParams } from 'react-router-dom';
import { Badge, Card, Panel } from '../../design-system';
import '../auth/auth.css';

const UploadPlaceholderPage: React.FC = () => {
  const [searchParams] = useSearchParams();
  const location = useLocation() as { state?: { fileName?: string } };
  const sample = searchParams.get('sample') === '1';
  const fileName = location.state?.fileName;

  return (
    <div className="gds-root gds-onboarding" data-testid="upload-page">
      <Panel title="New analysis" className="gds-onboarding__card">
        <Card glass>
          {sample ? (
            <p data-testid="upload-sample">
              <Badge status="neutral">sample</Badge> Loading the bundled sample manuscript — the
              full upload &amp; analysis flow lands in the next build step (F5).
            </p>
          ) : fileName ? (
            <p data-testid="upload-file">
              <Badge status="neutral">queued</Badge> <span className="gds-mono">{fileName}</span>{' '}
              received — parsing runs fully on your device. The analysis flow lands in F5.
            </p>
          ) : (
            <p data-testid="upload-fresh">
              Upload a manuscript (PDF, DOCX, TXT). The full flow lands in F5.
            </p>
          )}
          <Link to="/app">← Back to Home</Link>
        </Card>
      </Panel>
    </div>
  );
};

export default UploadPlaceholderPage;
