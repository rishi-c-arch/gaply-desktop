// Gaply — pasted-image storage for the Note Creator (rich-editor Set 3).
//
// DESIGN (all proven read-only before building):
//  • Bytes come from the FREE paste path (clipboardData → getAsFile()), proven
//    to deliver real bytes in the real WKWebView. Drag-drop does NOT deliver in
//    this webview and is handled honestly upstream (a hint, never a dead-zone).
//  • Files live at  $APPDATA/note-images/<sha256>.<ext>  (fs plugin scope
//    already grants $APPDATA/**). CONTENT-HASH naming ⇒ the same screenshot
//    pasted twice is ONE file (dedupe), and two notes can SHARE one file.
//  • The note's markdown stores a portable ref  gaply-image://<sha256>.<ext>
//    (canonical, path-free). At RENDER time the ref resolves to a blob: object
//    URL (CSP already allows blob:) — so NO custom URI scheme, NO Rust handler,
//    NO CSP change is needed. That is the "asset-protocol footnote", resolved:
//    the blob path the spike proved replaces it, with no shell rebuild.
//  • Because files are shared by hash, GC MUST be reference-COUNTED: a file is
//    deleted ONLY when NO note still references its hash (see gcOrphans).

export const IMAGE_REF_PREFIX = 'gaply-image://';
export const IMAGE_SUBDIR = 'note-images';
export const MAX_IMAGE_BYTES = 10 * 1024 * 1024; // 10 MB — a single .md can't carry binaries; keep files sane.

// The image types we accept. mime → on-disk extension (also drives the blob
// type on the way back out). Kept small and explicit — an unknown type is
// rejected honestly rather than written with a wrong/empty extension.
const MIME_TO_EXT: Record<string, string> = {
  'image/png': 'png',
  'image/jpeg': 'jpg',
  'image/gif': 'gif',
  'image/webp': 'webp',
};
const EXT_TO_MIME: Record<string, string> = Object.fromEntries(
  Object.entries(MIME_TO_EXT).map(([m, e]) => [e, m]),
);

export const isImageRef = (s: string): boolean => s.startsWith(IMAGE_REF_PREFIX);

/** Every gaply-image ref present in a note body (markdown). Used for GC
 *  candidates and to detect image-free notes. Deduped; order-stable. */
export const imageRefsIn = (md: string): string[] => {
  const matches: string[] = md.match(/gaply-image:\/\/[A-Za-z0-9]+\.[A-Za-z0-9]+/g) ?? [];
  return matches.filter((ref, i) => matches.indexOf(ref) === i); // dedupe, order-stable
};

const refToFilename = (ref: string): string => ref.slice(IMAGE_REF_PREFIX.length);

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(digest)).map((b) => b.toString(16).padStart(2, '0')).join('');
}

/** Tauri plugin commands reject with a serialized string, not an Error — this
 *  normalizes either into readable text so the real cause is never swallowed. */
const errText = (e: unknown): string => (e instanceof Error ? e.message : String(e));

// Lazy Tauri imports (mirrors notesBridge) so vitest can run/mocks the pure
// logic without a live desktop shell.
const fs = () => import('@tauri-apps/plugin-fs');

/** Persist a pasted image and return its canonical ref. Content-hash naming
 *  makes this idempotent: pasting the same bytes again returns the same ref and
 *  writes nothing (dedupe). Throws an HONEST message on oversize / unsupported
 *  type — the caller surfaces it, never a silent failure. */
export async function storeImageFile(file: File): Promise<string> {
  const ext = MIME_TO_EXT[file.type];
  if (!ext) {
    throw new Error(`That image type (${file.type || 'unknown'}) isn’t supported — use PNG, JPEG, GIF, or WebP.`);
  }
  // Size gate BEFORE reading bytes — an oversize paste costs nothing.
  if (file.size > MAX_IMAGE_BYTES) {
    throw new Error(`That image is ${(file.size / 1048576).toFixed(1)} MB — over the ${MAX_IMAGE_BYTES / 1048576} MB limit. Paste a smaller one.`);
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  const hash = await sha256Hex(bytes);
  const rel = `${IMAGE_SUBDIR}/${hash}.${ext}`;
  const { writeFile, mkdir, exists, BaseDirectory } = await fs();
  try {
    await mkdir(IMAGE_SUBDIR, { baseDir: BaseDirectory.AppData, recursive: true });
    // Dedupe: identical bytes ⇒ identical path ⇒ skip the write.
    if (!(await exists(rel, { baseDir: BaseDirectory.AppData }))) {
      await writeFile(rel, bytes, { baseDir: BaseDirectory.AppData });
    }
  } catch (e) {
    // Tauri fs rejects with a STRING, not an Error — surface the REAL reason
    // (e.g. an ACL denial) instead of letting it vanish into a generic message.
    throw new Error(`Saving the image to disk failed: ${errText(e)}`);
  }
  return `${IMAGE_REF_PREFIX}${hash}.${ext}`;
}

// Session cache: ref → blob object URL. The same image reused across notes /
// re-renders shares one URL. Revoked when the file is GC'd.
const resolved = new Map<string, string>();

/** Resolve a gaply-image ref to a displayable blob: URL (reads the file once,
 *  caches the object URL). This is the render path — CSP already allows blob:. */
export async function resolveImageRef(ref: string): Promise<string> {
  const hit = resolved.get(ref);
  if (hit) return hit;
  const name = refToFilename(ref);
  const ext = name.split('.').pop() ?? '';
  const { readFile, BaseDirectory } = await fs();
  let bytes: Uint8Array;
  try {
    bytes = await readFile(`${IMAGE_SUBDIR}/${name}`, { baseDir: BaseDirectory.AppData });
  } catch (e) {
    throw new Error(`Loading the image from disk failed: ${errText(e)}`);
  }
  const url = URL.createObjectURL(new Blob([bytes], { type: EXT_TO_MIME[ext] ?? 'application/octet-stream' }));
  resolved.set(ref, url);
  return url;
}

/** Read an image's RAW bytes + mime from disk (NOT a blob URL) — the docx export
 *  path needs the actual bytes to embed via docx's ImageRun. Throws with the real
 *  reason on a read/ACL failure (never swallowed). */
export async function readImageBytes(ref: string): Promise<{ data: Uint8Array; mime: string }> {
  const name = refToFilename(ref);
  const ext = name.split('.').pop() ?? '';
  const { readFile, BaseDirectory } = await fs();
  try {
    const data = await readFile(`${IMAGE_SUBDIR}/${name}`, { baseDir: BaseDirectory.AppData });
    return { data, mime: EXT_TO_MIME[ext] ?? 'application/octet-stream' };
  } catch (e) {
    throw new Error(`Reading the image bytes failed: ${errText(e)}`);
  }
}

/** The on-disk filename behind a ref (e.g. "<hash>.png") — for honest export
 *  placeholders when bytes can't be embedded. */
export const imageFilenameOf = (ref: string): string => refToFilename(ref);

/** Delete one image file from disk + drop its cached object URL. Best-effort:
 *  a missing file is a no-op (it may already be gone). Only ever called by GC
 *  on a CONFIRMED orphan. */
export async function deleteImageFile(ref: string): Promise<void> {
  const { remove, exists, BaseDirectory } = await fs();
  const rel = `${IMAGE_SUBDIR}/${refToFilename(ref)}`;
  try {
    if (await exists(rel, { baseDir: BaseDirectory.AppData })) {
      await remove(rel, { baseDir: BaseDirectory.AppData });
    }
  } catch (e) {
    // gcOrphans treats this as best-effort (a failed delete just leaves a
    // harmless orphan) — but surface the real reason so a denial isn't silent.
    throw new Error(`Deleting the orphaned image failed: ${errText(e)}`);
  }
  const url = resolved.get(ref);
  if (url) { URL.revokeObjectURL(url); resolved.delete(ref); }
}

/**
 * Reference-COUNTED garbage collection — THE data-loss guard.
 *
 * For each candidate ref (an image that MIGHT have just become orphaned by a
 * note delete/edit), delete its file ONLY when `isReferenced(ref)` is false —
 * i.e. NO note in the whole library still references that hash. Because files
 * are shared by content-hash, a per-note delete would be catastrophic: deleting
 * note A must NOT remove an image note B still shows. Here, B's surviving
 * reference keeps the file.
 *
 * MUST be called AFTER the note mutation commits, so `isReferenced` reflects the
 * post-delete/post-edit world (else the note's own about-to-be-removed ref would
 * inflate the count and nothing would ever GC).
 *
 * Crash-safe: we only ever delete a CONFIRMED orphan, so an interrupted sweep
 * leaves a harmless unreferenced file on disk (cleaned by a later sweep) and
 * can NEVER remove a live image. Returns the refs actually deleted.
 */
export async function gcOrphans(
  candidateRefs: string[],
  isReferenced: (ref: string) => Promise<boolean>,
  removeFile: (ref: string) => Promise<void> = deleteImageFile,
): Promise<string[]> {
  const deleted: string[] = [];
  const unique = candidateRefs.filter((r, i) => candidateRefs.indexOf(r) === i);
  for (const ref of unique) {
    if (await isReferenced(ref)) continue; // a live reference survives — keep the file
    try {
      await removeFile(ref);
      deleted.push(ref);
    } catch {
      // Orphan cleanup is best-effort; a failure just leaves a harmless file.
    }
  }
  return deleted;
}
