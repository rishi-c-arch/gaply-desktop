// §11 D91. The PDF export landed as a .txt. Neither generator was at fault —
// both emit a valid %PDF-1.4 … %%EOF — so these pin the DELIVERY.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mkdtempSync, writeFileSync, readFileSync, existsSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';

vi.mock('./isTauri', () => ({ isTauri: true })); // force the desktop path
const save = vi.fn();
const writeFile = vi.fn();
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: (...a: unknown[]) => save(...a) }));
vi.mock('@tauri-apps/plugin-fs', () => ({ writeFile: (...a: unknown[]) => writeFile(...a) }));

import { saveBinaryFile, extensionOf, withExtension } from './saveBinaryFile';

beforeEach(() => { save.mockReset(); writeFile.mockReset(); });

describe('saveBinaryFile — the delivery, not the bytes', () => {
  it('CANCEL → null, and nothing is written', async () => {
    save.mockResolvedValue(null);
    expect(await saveBinaryFile('r.pdf', new Uint8Array([1]), 'application/pdf')).toBeNull();
    expect(writeFile).not.toHaveBeenCalled();
  });

  it('a genuine write FAILURE throws — distinct from cancel', async () => {
    save.mockResolvedValue('/Users/x/r.pdf');
    writeFile.mockRejectedValue(new Error('disk full'));
    await expect(saveBinaryFile('r.pdf', new Uint8Array([1]))).rejects.toThrow(/disk full/);
  });

  /** THE BUG. `<a download>` in the desktop webview is what mangled the file;
   *  the native dialog is what `saveTextFile` already uses, for this reason. */
  it('uses the NATIVE dialog on the desktop, never an anchor download', async () => {
    save.mockResolvedValue('/Users/x/report.pdf');
    writeFile.mockResolvedValue(undefined);
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click');

    const bytes = new Uint8Array([0x25, 0x50, 0x44, 0x46]);
    const path = await saveBinaryFile('report.pdf', bytes, 'application/pdf');

    expect(path).toBe('/Users/x/report.pdf');
    expect(writeFile).toHaveBeenCalledWith('/Users/x/report.pdf', bytes);
    expect(click).not.toHaveBeenCalled();
    click.mockRestore();
  });

  /** A filter is a REQUEST to the OS; the returned path is the fact. */
  it('names the file type to the dialog', async () => {
    save.mockResolvedValue('/Users/x/report.pdf');
    writeFile.mockResolvedValue(undefined);
    await saveBinaryFile('report.pdf', new Uint8Array([1]), 'application/pdf');
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        defaultPath: 'report.pdf',
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      }),
    );
  });

  it('re-attaches an extension the panel dropped', async () => {
    save.mockResolvedValue('/Users/x/report'); // macOS can hand back no extension
    writeFile.mockResolvedValue(undefined);
    const path = await saveBinaryFile('report.pdf', new Uint8Array([1]), 'application/pdf');
    expect(path).toBe('/Users/x/report.pdf');
    expect(writeFile).toHaveBeenCalledWith('/Users/x/report.pdf', expect.anything());
  });

  it('does not double an extension the panel kept', () => {
    expect(withExtension('/x/r.pdf', 'pdf')).toBe('/x/r.pdf');
    expect(withExtension('/x/r.PDF', 'pdf')).toBe('/x/r.PDF');
    expect(withExtension('/x/r', 'pdf')).toBe('/x/r.pdf');
    expect(extensionOf('a-citation-audit.pdf')).toBe('pdf');
    expect(extensionOf('.bashrc')).toBe('');
    expect(extensionOf('noext')).toBe('');
  });
});

/** THE FILE, not the bytes. What the user actually opens in Preview. */
describe('the exported PDF as it lands on disk', () => {
  it('is a real PDF file with a .pdf name', async () => {
    save.mockResolvedValue(join(mkdtempSync(join(tmpdir(), 'gaply-pdf-')), 'audit'));
    // Write for real, exactly as the Tauri fs plugin would.
    writeFile.mockImplementation(async (p: string, b: Uint8Array) => {
      writeFileSync(p, Buffer.from(b));
    });

    const { renderTextPdf } = await import('../screens/report/miniPdf');
    const blob = renderTextPdf([{ text: 'Gaply Integrity Report', size: 20 }]);
    const bytes = new Uint8Array(await blob.arrayBuffer());

    const path = await saveBinaryFile('audit.pdf', bytes, 'application/pdf');
    expect(path).toMatch(/\.pdf$/); // the extension survived the round trip
    expect(existsSync(path!)).toBe(true);

    // `file(1)` identifies a PDF by these two, and so does Preview.
    const onDisk = readFileSync(path!);
    expect(onDisk.subarray(0, 5).toString('latin1')).toBe('%PDF-');
    expect(onDisk.subarray(-6).toString('latin1').trim()).toBe('%%EOF');
    expect(onDisk.length).toBeGreaterThan(200);
  });
});
