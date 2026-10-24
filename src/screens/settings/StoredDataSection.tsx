// Gaply — the Stored data section of Settings (Research OS, Phase 5D-1a;
// decisions 206 and 207): what Gaply's older features keep on this computer,
// one line per category, with its name, its sentence, how many items and how
// much space. Read only: there is no delete control until 5D-1b.
//
// Shown only when storedDataShown() says so; otherwise it draws nothing and
// calls nothing. It shows the two counts readStoredData keeps and nothing
// else of the answer, and one fixed sentence when the summary fails.
import React, { useEffect, useState } from 'react';
import { Card } from '../../design-system';
import { mb } from '../statistics/projects/projectWording';
import { readStoredData, StoredCount, StoredDataBridge, tauriStoredDataBridge } from './storedDataBridge';
import { storedDataShown } from './storedDataGate';
import { countWords, STORED_CATEGORIES, STORED_DATA_TEXT, StoredCategoryId } from './storedDataWording';

export interface StoredDataSectionProps {
  /** Test seams; the app passes nothing. */
  bridge?: StoredDataBridge;
  desktop?: boolean;
}

type View = { kind: 'reading' } | { kind: 'failed' } | { kind: 'read'; counts: Map<StoredCategoryId, StoredCount> };

export const StoredDataSection: React.FC<StoredDataSectionProps> = ({ bridge = tauriStoredDataBridge, desktop }) => {
  const shown = storedDataShown(desktop);
  const [view, setView] = useState<View>({ kind: 'reading' });

  useEffect(() => {
    if (!shown) return undefined;
    let alive = true;
    bridge.summary().then(
      (answer) => {
        if (!alive) return;
        const counts = readStoredData(answer);
        setView(counts ? { kind: 'read', counts } : { kind: 'failed' });
      },
      () => {
        if (alive) setView({ kind: 'failed' });
      }
    );
    return () => {
      alive = false;
    };
  }, [shown, bridge]);

  if (!shown) return null;

  return (
    <Card title={STORED_DATA_TEXT.heading}>
      <div data-testid="stored-data">
        {view.kind === 'reading' && <p className="gds-set__muted">{STORED_DATA_TEXT.reading}</p>}
        {view.kind === 'failed' && (
          <p className="gds-set__muted" data-testid="stored-data-failed">
            {STORED_DATA_TEXT.failed}
          </p>
        )}
        {view.kind === 'read' && (
          <ul className="gds-set__stored">
            {STORED_CATEGORIES.map((c) => {
              const n = view.counts.get(c.id) ?? { items: 0, bytes: 0 };
              return (
                <li key={c.id} className="gds-set__stored-line" data-testid={`stored-line-${c.id}`}>
                  <span className="gds-set__stored-name">{c.name}</span>
                  <span className="gds-set__muted">{c.sentence}</span>
                  <span className="gds-set__stored-count" data-testid={`stored-count-${c.id}`}>
                    {`${countWords(n.items, c)}. Size: ${mb(n.bytes)}.`}
                  </span>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </Card>
  );
};
