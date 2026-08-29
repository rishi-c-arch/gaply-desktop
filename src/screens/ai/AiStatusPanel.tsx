// Gaply — the AI status + install surface (Phase 9 item 1).
//
// Two rules shape this panel:
//
//   * **Never invent a number.** RAM comes from `generativeRam`, computed by the
//     backend from GGUF metadata — never process RSS, never a frontend guess.
//   * **The device is a FACT, not a warning.** `activeDevice: 'cpu'` is correct
//     on any machine below macOS 15, because the §11 D36 gate closes there.
//     Presenting it as a fault would be telling the user something is wrong
//     when nothing is.
import './ai.css';
import React, { useCallback, useEffect, useState } from 'react';
import { Badge, Button, Card } from '../../design-system/primitives';
import { aiBridge, AiModelStatus, InstallEvent } from './aiBridge';

/** Is a model actually usable? The engines report a tagged state. */
export function isReady(state: { kind: string } | undefined | null): boolean {
  return state?.kind === 'ready' || state?.kind === 'loaded' || state?.kind === 'notLoaded';
}

export function isNotInstalled(status: AiModelStatus | null): boolean {
  if (!status) return true;
  return !isReady(status.embedding) || !isReady(status.generative);
}

function formatBytes(n: number): string {
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(1)} GB`;
  if (n >= 1024 ** 2) return `${Math.round(n / 1024 ** 2)} MB`;
  return `${n} B`;
}

interface Progress {
  file: string;
  bytes: number;
  total: number;
  resumed: boolean;
}

export interface AiStatusPanelProps {
  /** Test seam. */
  bridge?: Pick<
    typeof aiBridge,
    'modelStatus' | 'installEmbedding' | 'installGenerative' | 'cancelInstall'
  >;
}

export const AiStatusPanel: React.FC<AiStatusPanelProps> = ({ bridge = aiBridge }) => {
  const [status, setStatus] = useState<AiModelStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [installing, setInstalling] = useState(false);

  const refresh = useCallback(() => {
    bridge
      .modelStatus()
      .then(setStatus)
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
  }, [bridge]);

  useEffect(refresh, [refresh]);

  const onEvent = useCallback((ev: InstallEvent) => {
    switch (ev.kind) {
      case 'downloading':
        setProgress({
          file: ev.file,
          bytes: ev.fromBytes ?? 0,
          total: ev.totalBytes ?? 0,
          // fromBytes > 0 means the installer picked up a partial download
          // rather than starting again — worth saying, since it is the whole
          // reason a large install survives a dropped connection.
          resumed: (ev.fromBytes ?? 0) > 0,
        });
        break;
      case 'progress':
        setProgress((p) => ({
          file: ev.file,
          bytes: ev.bytes,
          total: ev.totalBytes,
          resumed: p?.resumed ?? false,
        }));
        break;
      case 'cancelled':
      case 'done':
        setProgress(null);
        setInstalling(false);
        break;
      default:
        break;
    }
  }, []);

  const install = useCallback(async () => {
    setInstalling(true);
    setError(null);
    try {
      await bridge.installEmbedding(onEvent);
      await bridge.installGenerative('qwen2.5-1.5b-instruct-q4km', onEvent);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setInstalling(false);
      setProgress(null);
      refresh();
    }
  }, [bridge, onEvent, refresh]);

  const pct = progress && progress.total > 0 ? Math.round((progress.bytes / progress.total) * 100) : 0;

  return (
    <Card title="Local AI" data-testid="ai-status-panel">
      {error && (
        <p className="gds-ai__hint" data-testid="ai-status-error" style={{ color: 'var(--g-flagged)' }}>
          {error}
        </p>
      )}

      {isNotInstalled(status) ? (
        <>
          <p className="gds-ai__hint" data-testid="ai-not-installed">
            Gaply’s AI features run entirely on this machine. Everything else in Gaply —
            citations, formatting, plagiarism and statistics — works without them.
          </p>
          {installing ? (
            <>
              <div className="gds-ai__progress" data-testid="ai-install-progress">
                <div style={{ width: `${pct}%` }} />
              </div>
              <p className="gds-ai__hint">
                {progress
                  ? `${progress.resumed ? 'Resuming' : 'Downloading'} ${progress.file} — ${pct}%`
                  : 'Preparing…'}
              </p>
              <Button variant="secondary" onClick={() => bridge.cancelInstall()} data-testid="ai-install-cancel">
                Cancel
              </Button>
            </>
          ) : (
            <Button variant="primary" onClick={install} data-testid="ai-install">
              Install Gaply AI
            </Button>
          )}
        </>
      ) : (
        <>
          <div className="gds-ai__row">
            <span className="gds-ai__label">Generative model</span>
            <span className="gds-ai__value" data-testid="ai-model-id">
              {status?.generativeModelId}
            </span>
          </div>
          <div className="gds-ai__row">
            <span className="gds-ai__label">Processing</span>
            <span className="gds-ai__value" data-testid="ai-device">
              {status?.activeDevice === 'metal' ? 'GPU (Metal)' : 'CPU'}
            </span>
          </div>
          <div className="gds-ai__row">
            <span className="gds-ai__label">Estimated memory</span>
            <span className="gds-ai__value" data-testid="ai-ram">
              {status?.generativeRam
                ? formatBytes(status.generativeRam.totalBytes)
                : 'not reported'}
            </span>
          </div>
          <div className="gds-ai__row">
            <span className="gds-ai__label">In flight</span>
            <span className="gds-ai__value">{status?.inFlight ?? 0}</span>
          </div>
          <p className="gds-ai__hint">
            Runs on this machine. Nothing is sent anywhere.
          </p>
        </>
      )}
    </Card>
  );
};

/**
 * The single NotInstalled affordance, reused at every surface where an AI
 * feature would otherwise show a button that cannot work.
 *
 * A disabled button with no explanation reads as a bug; this says what is
 * missing and where to fix it.
 */
export const AiUnavailable: React.FC<{ feature: string; onOpenSettings?: () => void }> = ({
  feature,
  onOpenSettings,
}) => (
  <div className="gds-evidence gds-evidence--ungrounded" data-testid="ai-unavailable">
    <Badge status="neutral">Local AI</Badge>
    <p data-testid="ai-unavailable-text">
      {feature} needs Gaply’s local AI, which isn’t installed yet.
    </p>
    {onOpenSettings && (
      <Button variant="secondary" onClick={onOpenSettings} data-testid="ai-unavailable-install">
        Install Gaply AI
      </Button>
    )}
  </div>
);

export default AiStatusPanel;
