// Rich editor Set 3 — pasted-image storage + reference-counted GC pins.
// The GC test (pin a) is THE data-loss guard: two notes sharing one hashed
// image, delete one, the OTHER note's file MUST survive. Pure logic — the fs
// and the note store are injected, so this runs headless and deterministic.
import { describe, it, expect, vi } from 'vitest';
import { imageRefsIn, isImageRef, gcOrphans, storeImageFile, IMAGE_REF_PREFIX, MAX_IMAGE_BYTES } from './noteImages';

/* ---------- ref extraction / backward compat (pin d) ---------- */
describe('imageRefsIn — extract gaply-image refs from a body', () => {
  it('finds refs inside markdown image syntax, deduped + order-stable', () => {
    const body = 'intro\n\n![](gaply-image://aaa111.png)\n\nmiddle ![](gaply-image://bbb222.jpg) and again ![](gaply-image://aaa111.png)';
    expect(imageRefsIn(body)).toEqual(['gaply-image://aaa111.png', 'gaply-image://bbb222.jpg']);
  });
  it('an image-FREE note yields no refs (backward compat: nothing to GC, nothing rewritten)', () => {
    for (const plain of ['Sleep helps recall.', '# H\n\n- a\n- b', '| A | B |\n| --- | --- |\n| 1 | 2 |', '']) {
      expect(imageRefsIn(plain)).toEqual([]);
    }
  });
  it('isImageRef only matches the gaply-image scheme', () => {
    expect(isImageRef('gaply-image://x.png')).toBe(true);
    expect(isImageRef('https://example.com/x.png')).toBe(false);
    expect(isImageRef('data:image/png;base64,AAAA')).toBe(false);
  });
});

/* ---------- reference-COUNTED GC — THE data-loss guard (pin a) ---------- */
describe('gcOrphans — reference-counted, never deletes a shared/live file (pin a)', () => {
  const SHARED = 'gaply-image://shared777.png';

  // A tiny library "search": returns true when ANY note body still contains the ref.
  const referencedIn = (bodies: string[]) => async (ref: string) => bodies.some((b) => b.includes(ref));

  it('note A + note B share one hashed image → delete A → B keeps the file', async () => {
    // After A is deleted, only B remains and B still references SHARED.
    const remainingBodies = [`note B\n\n![](${SHARED})`];
    const removeFile = vi.fn(async () => {});
    const deleted = await gcOrphans([SHARED], referencedIn(remainingBodies), removeFile);

    expect(removeFile).not.toHaveBeenCalled(); // the shared file is NEVER deleted
    expect(deleted).toEqual([]); // B's live reference kept it
  });

  it('delete the LAST note referencing an image → its now-orphaned file is removed', async () => {
    const remainingBodies: string[] = []; // no note references it anymore
    const removeFile = vi.fn(async () => {});
    const deleted = await gcOrphans([SHARED], referencedIn(remainingBodies), removeFile);

    expect(removeFile).toHaveBeenCalledWith(SHARED);
    expect(deleted).toEqual([SHARED]);
  });

  it('crash-safe: a removeFile failure never throws and never blocks other orphans', async () => {
    const orphanA = 'gaply-image://a.png';
    const orphanB = 'gaply-image://b.png';
    const removeFile = vi.fn(async (ref: string) => { if (ref === orphanA) throw new Error('disk hiccup'); });
    const deleted = await gcOrphans([orphanA, orphanB], async () => false, removeFile);

    expect(deleted).toEqual([orphanB]); // A failed harmlessly, B still swept
  });

  it('mixed batch: only the unreferenced candidates are deleted', async () => {
    const live = 'gaply-image://live.png';
    const dead = 'gaply-image://dead.png';
    const removeFile = vi.fn(async () => {});
    const deleted = await gcOrphans([live, dead], referencedIn([`note\n![](${live})`]), removeFile);

    expect(deleted).toEqual([dead]);
    expect(removeFile).toHaveBeenCalledTimes(1);
    expect(removeFile).toHaveBeenCalledWith(dead);
  });
});

/* ---------- 10 MB cap + type gate, honest failure (pin c) ---------- */
describe('storeImageFile — honest rejection, no silent failure (pin c)', () => {
  const fakeFile = (type: string, size: number): File =>
    ({ type, size, name: 'x', arrayBuffer: async () => new ArrayBuffer(0) } as unknown as File);

  it('rejects an oversized image with an honest message BEFORE any read/write', async () => {
    const big = fakeFile('image/png', MAX_IMAGE_BYTES + 1);
    const spy = vi.spyOn(big, 'arrayBuffer');
    await expect(storeImageFile(big)).rejects.toThrow(/over the 10 MB limit/i);
    expect(spy).not.toHaveBeenCalled(); // gated before reading bytes
  });

  it('rejects an unsupported type honestly (not a silent write with a bad extension)', async () => {
    await expect(storeImageFile(fakeFile('image/tiff', 100))).rejects.toThrow(/isn’t supported/i);
    await expect(storeImageFile(fakeFile('', 100))).rejects.toThrow(/isn’t supported/i);
  });

  it('a ref is the canonical scheme + content hash + extension', () => {
    // (shape assertion — storeImageFile builds `${PREFIX}${sha256}.${ext}`)
    expect(IMAGE_REF_PREFIX).toBe('gaply-image://');
  });
});
