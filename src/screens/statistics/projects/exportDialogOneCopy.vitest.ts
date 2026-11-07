// The project report's dialog, one copy (Research OS, Phase 8U-1, decision 526;
// ADR-012 Part 9): ProjectExportDialog.tsx holds it, its test ids and its
// export path; the project page and the Statistics window each import it, and
// the page holds no copy of its own. Read as source, comments blanked.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const HERE = __dirname;
const read = (p: string) => fs.readFileSync(path.join(HERE, p), 'utf8');
const code = (src: string) => src.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|\s)\/\/.*$/gm, '$1');

describe('one copy of the report dialog (decision 526)', () => {
  const dialog = code(read('ProjectExportDialog.tsx'));
  const page = code(read('ProjectPage.tsx'));
  const window = code(read('../window/StatisticsWindow.tsx'));

  it('ProjectExportDialog.tsx exports it, with its test ids and the export path', () => {
    expect(dialog).toMatch(/export const ProjectExportDialog\b/);
    expect(dialog).toContain('project-export-dialog');
    expect(dialog).toContain('project-export-confirm');
    expect(dialog).toContain('prepareExport(');
  });

  it('the page holds no copy: no dialog test id, no prepareExport( call, no component of its own', () => {
    expect(page).not.toContain('project-export-dialog');
    expect(page).not.toContain('prepareExport(');
    expect(page).not.toMatch(/const ExportDialog\b/);
  });

  it('the page and the window each import it, from the one file', () => {
    expect(page).toMatch(/import \{[^}]*\bProjectExportDialog\b[^}]*\} from '\.\/ProjectExportDialog';/);
    expect(window).toMatch(/import \{[^}]*\bProjectExportDialog\b[^}]*\} from '\.\.\/projects\/ProjectExportDialog';/);
    expect(window).not.toContain('prepareExport(');
  });
});
