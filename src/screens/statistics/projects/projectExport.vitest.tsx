// The two exports on the project page (Research OS, Phase 5B-3b; decision
// 151), over mock bridges and a recording stand-in for today's export path
// (utils/saveTextFile's signature).
//
// * Each button opens the small dialog with its decision 133 sentence first;
//   Cancel calls nothing.
// * Export renders on that click, checks the file name and the SHA-256, and
//   hands the text whose UTF-8 bytes are exactly the bytes Rust returned, and
//   the fixed file name, to the export path.
// * A wrong SHA-256, a wrong file name, bytes that are not UTF-8 and a render
//   refusal are refused with their sentences, and nothing is saved; a closed
//   save dialog writes nothing and says so quietly; a failed write says so.
// * Under StrictMode, one click renders once.
import { createHash } from 'node:crypto';
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockProjectBridge } from './projectBridge';
import { EXPORTS, prepareExport, type ExportKind } from './projectExport';
import { EXPORT_WORDS, PROJECT_TEXT, PROJECT_WORDS, PROVENANCE_EXPORT_SENTENCE, REPORT_EXPORT_SENTENCE } from './projectWording';
import { ProjectPage } from './ProjectPage';

vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || name === 'statsDescribe',
}));

afterEach(cleanup);

const A = '0123456789abcdef0123456789abcdef';
const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const REPORT_TEXT = '# HTPROJ_TEST\n\nThis report holds fingerprints — and “quotes” é ✓.\n';
const PROV_TEXT = '{"entity":{},"prefix":{"gaply":"x"}}\n';
const bytesOf = (t: string) => Array.from(new TextEncoder().encode(t));
const sha = (b: number[]) => createHash('sha256').update(Uint8Array.from(b)).digest('hex');
const view = (fileName: string, text: string) => ({ fileName, bytes: bytesOf(text), sha256: sha(bytesOf(text)) });
const REPORT = view('gaply_project_report.md', REPORT_TEXT);
const PROV = view('gaply_project_provenance.json', PROV_TEXT);

function page(responses: Record<string, unknown> = {}, save = vi.fn().mockResolvedValue('/chosen/place'), strict = false) {
  const bridge = makeMockProjectBridge({
    project_current: { projectId: A },
    project_list: [{ projectId: A, name: 'Chapter 3', bytes: 4096, savedAt: 1792396351, runCount: 0, refused: null }],
    project_sizes: { totalBytes: 4096, projectCount: 1, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 },
    project_runs: [],
    project_report: REPORT,
    project_provenance: PROV,
    ...responses,
  });
  const workspace = makeMockWorkspaceBridge({ workspace_state: ACTIVE });
  const ui = <ProjectPage projectId={A} bridge={bridge} workspace={workspace} desktop save={save} onClosed={vi.fn()} onBack={vi.fn()} />;
  render(strict ? <React.StrictMode>{ui}</React.StrictMode> : ui);
  return { bridge, save };
}

const renders = (b: ReturnType<typeof makeMockProjectBridge>) => b.calls.filter(([c]) => c === 'project_report' || c === 'project_provenance');

describe('the export dialog', () => {
  it.each([
    ['report', REPORT_EXPORT_SENTENCE, PROJECT_TEXT.exportReportTitle],
    ['provenance', PROVENANCE_EXPORT_SENTENCE, PROJECT_TEXT.exportProvenanceTitle],
  ] as const)('%s: shows its sentence first, with the focus on Cancel; Cancel calls nothing', async (kind, sentence, title) => {
    const { bridge, save } = page();
    fireEvent.click(await screen.findByTestId(`project-export-${kind}`));
    const dialog = screen.getByTestId('project-export-dialog');
    expect(dialog.getAttribute('role')).toBe('dialog');
    expect(dialog.textContent).toContain(title);
    expect(screen.getByTestId('project-export-sentence').textContent).toBe(sentence);
    expect(document.activeElement).toBe(screen.getByTestId('project-export-cancel'));
    fireEvent.click(screen.getByTestId('project-export-cancel'));
    expect(screen.queryByTestId('project-export-dialog')).toBeNull();
    expect(document.activeElement).toBe(screen.getByTestId(`project-export-${kind}`));
    expect(renders(bridge)).toEqual([]);
    expect(save).not.toHaveBeenCalled();
  });

  it.each([
    ['report', REPORT, 'text/markdown'],
    ['provenance', PROV, 'application/json'],
  ] as const)('%s: Export hands the bytes Rust returned, unchanged, and the fixed name to the export path', async (kind, returned, mime) => {
    const { bridge, save } = page();
    fireEvent.click(await screen.findByTestId(`project-export-${kind}`));
    fireEvent.click(screen.getByTestId('project-export-confirm'));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    const [name, text, type] = save.mock.calls[0];
    expect(name).toBe(returned.fileName);
    expect(Array.from(new TextEncoder().encode(text as string))).toEqual(returned.bytes);
    expect(type).toBe(mime);
    expect(renders(bridge)).toEqual([[`project_${kind}`, { projectId: A }]]);
    await waitFor(() => expect(screen.queryByTestId('project-export-dialog')).toBeNull());
    expect(screen.getByTestId('project-page-message').textContent).toBe(EXPORT_WORDS.saved(returned.sha256));
  });

  const refused: Array<[string, ExportKind, unknown, string]> = [
    ['a SHA-256 that does not match', 'report', { ...REPORT, sha256: 'f'.repeat(64) }, EXPORT_WORDS.wrongHash],
    ['another file name', 'report', { ...REPORT, fileName: 'HTPROJ_TEST.md' }, EXPORT_WORDS.wrongName],
    ["the other export's name", 'provenance', { ...PROV, fileName: 'gaply_project_report.md' }, EXPORT_WORDS.wrongName],
    ['bytes that are not UTF-8', 'report', { fileName: 'gaply_project_report.md', bytes: [0xff, 0xfe], sha256: sha([0xff, 0xfe]) }, EXPORT_WORDS.notExact],
    ['bytes that are not bytes', 'report', { fileName: 'gaply_project_report.md', bytes: [300], sha256: 'a'.repeat(64) }, EXPORT_WORDS.notExact],
  ];

  it.each(refused)('%s is refused with its sentence, and nothing is saved', async (_name, kind, returned, sentence) => {
    const { save } = page({ [`project_${kind}`]: returned });
    fireEvent.click(await screen.findByTestId(`project-export-${kind}`));
    fireEvent.click(screen.getByTestId('project-export-confirm'));
    const shown = await screen.findByTestId('project-export-refusal');
    expect(shown.textContent).toBe(sentence);
    expect(shown.getAttribute('role')).toBe('alert');
    expect(save).not.toHaveBeenCalled();
    expect(screen.getByTestId('project-export-dialog')).toBeTruthy();
  });

  it("a render refusal shows its code's sentence, and nothing is saved", async () => {
    const { save } = page({
      project_provenance: () => {
        throw { code: 'validation', message: JSON.stringify({ code: 'project.prov_cycle' }) };
      },
    });
    fireEvent.click(await screen.findByTestId('project-export-provenance'));
    fireEvent.click(screen.getByTestId('project-export-confirm'));
    expect((await screen.findByTestId('project-export-refusal')).textContent).toBe(PROJECT_WORDS['project.prov_cycle']({ code: 'project.prov_cycle' }));
    expect(save).not.toHaveBeenCalled();
  });

  it('a closed save dialog writes nothing and says so quietly', async () => {
    const save = vi.fn().mockResolvedValue(null);
    page({}, save);
    fireEvent.click(await screen.findByTestId('project-export-report'));
    fireEvent.click(screen.getByTestId('project-export-confirm'));
    await waitFor(() => expect(screen.queryByTestId('project-export-dialog')).toBeNull());
    const message = screen.getByTestId('project-page-message');
    expect(message.textContent).toBe(PROJECT_TEXT.exportCancelled);
    expect(message.getAttribute('role')).toBe('status');
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('a failed write says so, and never shows its message', async () => {
    const save = vi.fn().mockRejectedValue(new Error('RAW_SENTINEL_w /Users/someone/x.md'));
    page({}, save);
    fireEvent.click(await screen.findByTestId('project-export-report'));
    fireEvent.click(screen.getByTestId('project-export-confirm'));
    expect((await screen.findByTestId('project-export-refusal')).textContent).toBe(EXPORT_WORDS.writeFailed);
    expect(document.body.innerHTML).not.toContain('RAW_SENTINEL');
  });

  it('one render for a double click in one event turn, with Export disabled while pending, under StrictMode', async () => {
    let release: (v: unknown) => void = () => undefined;
    const pending = new Promise((r) => {
      release = r;
    });
    const { bridge, save } = page({ project_report: () => pending }, undefined, true);
    fireEvent.click(await screen.findByTestId('project-export-report'));
    const b = screen.getByTestId('project-export-confirm');
    act(() => {
      b.click();
      b.click();
    });
    expect((screen.getByTestId('project-export-confirm') as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId('project-export-cancel') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('project-export-pending').textContent).toBe(PROJECT_TEXT.exporting);
    await act(async () => release(REPORT));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(renders(bridge)).toHaveLength(1);
    expect(bridge.calls.filter(([c]) => ['project_open', 'project_close', 'project_save_run', 'project_delete'].indexOf(c) >= 0)).toEqual([]);
  });
});

describe('prepareExport', () => {
  it('passes each fixed name with its own export only', async () => {
    expect(EXPORTS.report.fileName).toBe('gaply_project_report.md');
    expect(EXPORTS.provenance.fileName).toBe('gaply_project_provenance.json');
    expect(await prepareExport(REPORT, 'report')).toEqual({ ok: true, fileName: REPORT.fileName, text: REPORT_TEXT, sha256: REPORT.sha256 });
    expect(await prepareExport(PROV, 'provenance')).toEqual({ ok: true, fileName: PROV.fileName, text: PROV_TEXT, sha256: PROV.sha256 });
    expect(await prepareExport(REPORT, 'provenance')).toEqual({ ok: false, sentence: EXPORT_WORDS.wrongName });
  });

  it('keeps a byte-order mark and every byte as it came', async () => {
    const text = '﻿# T\r\n';
    const v = view('gaply_project_report.md', text);
    const r = await prepareExport(v, 'report');
    expect(r.ok).toBe(true);
    if (r.ok) expect(Array.from(new TextEncoder().encode(r.text))).toEqual(v.bytes);
  });

  it('refuses what is not an export view, and an upper-case SHA-256', async () => {
    for (const bad of [null, 'x', [], { fileName: 'gaply_project_report.md' }]) {
      expect((await prepareExport(bad, 'report')).ok).toBe(false);
    }
    expect(await prepareExport({ ...REPORT, sha256: REPORT.sha256.toUpperCase() }, 'report')).toEqual({ ok: false, sentence: EXPORT_WORDS.wrongHash });
  });
});
