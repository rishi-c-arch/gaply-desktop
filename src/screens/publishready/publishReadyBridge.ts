// Gaply — PublishReady orchestration bridge. Runs ALL SIX agents via the Report
// Compiler + ReConcile debate; the Verification Agent (cloud) does citation-
// hallucination checking through the Railway proxy. The manuscript stays on the
// device — only the structured payload from buildProxyPayload() is sent.
//
// The full local-agents → structured-summary → proxy → compile_report pipeline
// is a backend concern (compile_report / verify aren't Tauri commands yet — same
// deferral as F6/F8). The real TauriPublishReadyBridge is documented; tests use
// makePublishReadyMock.
import { PublishReadyReport } from '../report/reportTypes';
import { buildProxyPayload } from './buildPayload';
import { synthesizeReviewerLetter } from './synthesize';
import { PublishReadyResult, TargetJournal } from './publishReadyTypes';

export interface PublishReadyBridge {
  run(input: { manuscriptPath: string; journal: TargetJournal }): Promise<PublishReadyResult>;
}

/** Production path (documented, not wired): run the local agents over the path,
 *  build the structured payload, POST it to the proxy for cloud Verification +
 *  reviewer synthesis, then compile_report. */
export class TauriPublishReadyBridge implements PublishReadyBridge {
  async run(_input: { manuscriptPath: string; journal: TargetJournal }): Promise<PublishReadyResult> {
    throw new Error(
      'TauriPublishReadyBridge: the compile_report + proxy review pipeline is not wired yet. ' +
        'The structured-payload discipline (buildProxyPayload) and reviewer synthesis are ready.'
    );
  }
}

/** Test/dev bridge: given a compiled report, derive the reviewer letter and the
 *  exact proxy payload deterministically. Records the payload so tests can prove
 *  it carries no manuscript text. */
export function makePublishReadyMock(report: PublishReadyReport): PublishReadyBridge & { lastPayload?: unknown } {
  const bridge: PublishReadyBridge & { lastPayload?: unknown } = {
    async run({ journal }) {
      const proxyPayload = buildProxyPayload(report, journal);
      bridge.lastPayload = proxyPayload;
      const reviewerLetter = synthesizeReviewerLetter(report, journal);
      return { report, reviewerLetter, proxyPayload };
    },
  };
  return bridge;
}
