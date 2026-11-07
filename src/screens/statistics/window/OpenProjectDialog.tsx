// Gaply — Project, Open a project… (Research OS, Phase 8U-1, decision 525;
// ADR-012 Part 9). A window dialog on the design system's ProjectDialog,
// holding ProjectsPanel unchanged, with its own Card and heading, so its list,
// sizes, totals and Delete come with it. An Open closes the dialog and goes to
// the project's page, as the Statistics home does. A refusal shows in the panel,
// as it does on the home.
//
// It holds no project command of its own: the panel asks through the bridge it
// is given. The panel's Delete dialog is a ProjectDialog inside this one; each
// keeps Tab to its own box (ProjectDialog's target check).
import React from 'react';
import { Button } from '../../../design-system';
import { ProjectDialog } from '../../../design-system/ProjectDialog';
import type { WorkspaceBridge } from '../../workspace/workspaceBridge';
import type { ProjectBridge } from '../projects/projectBridge';
import { ProjectsPanel } from '../projects/ProjectsPanel';
import './MethodDialog.css';
import { HELP_CLOSE, OPEN_PROJECT_TITLE } from './windowWording';

export interface OpenProjectDialogProps {
  projects: ProjectBridge;
  workspace: WorkspaceBridge;
  /** Rust opened it: the dialog closes and the project's page is shown. */
  onOpened: (projectId: string) => void;
  onClose: () => void;
}

export const OpenProjectDialog: React.FC<OpenProjectDialogProps> = ({ projects, workspace, onOpened, onClose }) => (
  <ProjectDialog title={OPEN_PROJECT_TITLE} onClose={onClose} testId="sw-open-project-dialog">
    <ProjectsPanel bridge={projects} workspace={workspace} onOpened={onOpened} />
    <div className="sw-dialog__actions">
      <Button variant="secondary" onClick={onClose} data-testid="sw-open-project-close">
        {HELP_CLOSE}
      </Button>
    </div>
  </ProjectDialog>
);

export default OpenProjectDialog;
