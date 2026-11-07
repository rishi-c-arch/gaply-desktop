// Gaply — the project report's and the provenance file's export dialog
// (Research OS, Phase 5B-3b, decisions 150 and 151; moved here, not copied, in
// Phase 8U-1, decision 526; ADR-012 Part 9). The project page renders it for
// both kinds; the Statistics window renders it for the report alone.
//
// * Nothing is asked of Rust until Export is clicked: then the render, the
//   checks of projectExport.ts, and today's export path, writing the one file
//   the user names through the save dialog.
// * One generation counter, the caller's, covers the render and the save, so a
//   late answer after the dialog has gone is dropped.
// * A refusal shows its sentence and keeps the dialog; a closed save dialog says
//   so through onDone.
import React, { useRef, useState } from 'react';
import { Button } from '../../../design-system';
import ProjectDialog from '../../../design-system/ProjectDialog';
import type { ProjectBridge } from './projectBridge';
import { EXPORTS, prepareExport, type ExportKind, type SaveFile } from './projectExport';
import { EXPORT_WORDS, PROJECT_TEXT, projectSentence } from './projectWording';
import type { Generation } from './useGeneration';

export interface ProjectExportDialogProps {
  kind: ExportKind;
  projectId: string;
  bridge: ProjectBridge;
  gen: Generation;
  save: SaveFile;
  openerAfter: () => HTMLElement | null;
  /** Closes the dialog; `said` is the page's status line, or null. */
  onDone: (said: string | null) => void;
}

export const ProjectExportDialog: React.FC<ProjectExportDialogProps> = ({ kind, projectId, bridge, gen, save, openerAfter, onDone }) => {
  const [pending, setPending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const inFlight = useRef(false);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const returnFocus = useRef<(() => HTMLElement | null) | null>(openerAfter);
  const e = EXPORTS[kind];

  const cancel = () => {
    if (!pending) onDone(null);
  };

  // On the Export click only: render, check, then today's export path.
  const run = async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    const g = gen.next();
    setPending(true);
    setRefusal(null);
    try {
      let rendered: unknown;
      try {
        rendered = kind === 'report' ? await bridge.report(projectId) : await bridge.provenance(projectId);
      } catch (err) {
        if (gen.is(g)) setRefusal(projectSentence(err));
        return;
      }
      if (!gen.is(g)) return;
      const ready = await prepareExport(rendered, kind);
      if (!gen.is(g)) return;
      if (!ready.ok) {
        setRefusal(ready.sentence);
        return;
      }
      let where: string | null;
      try {
        where = await save(ready.fileName, ready.text, e.mime);
      } catch {
        if (gen.is(g)) setRefusal(EXPORT_WORDS.writeFailed);
        return;
      }
      if (!gen.is(g)) return;
      onDone(where === null ? PROJECT_TEXT.exportCancelled : EXPORT_WORDS.saved(ready.sha256));
    } finally {
      inFlight.current = false;
      if (gen.is(g)) setPending(false);
    }
  };

  return (
    <ProjectDialog title={e.title} onClose={cancel} initialFocus={cancelButton} returnFocus={returnFocus} testId={`project-export-dialog`}>
      <p data-testid="project-export-sentence">{e.sentence}</p>
      {refusal && (
        <p role="alert" data-testid="project-export-refusal">
          {refusal}
        </p>
      )}
      <p role="status" aria-live="polite" data-testid="project-export-pending" style={{ margin: '4px 0' }}>
        {pending ? PROJECT_TEXT.exporting : ''}
      </p>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        <button
          ref={cancelButton}
          type="button"
          className="gds-btn gds-btn--secondary"
          onClick={cancel}
          disabled={pending}
          data-testid="project-export-cancel"
        >
          {PROJECT_TEXT.cancel}
        </button>
        <Button onClick={() => void run()} disabled={pending} data-testid="project-export-confirm">
          {PROJECT_TEXT.exportConfirm}
        </Button>
      </div>
    </ProjectDialog>
  );
};

export default ProjectExportDialog;
