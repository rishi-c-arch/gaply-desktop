// Gaply — the manuscript itself, with every judgement drawn where it happened
// (§11 D92). PDF only, by decision: a .docx does not record where its pages
// break, so there is no layout to annotate and a reconstruction that looks
// subtly wrong to the author is worse than none.
import './ai.css';
import 'react-pdf/dist/esm/Page/TextLayer.css';
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Document, Page } from 'react-pdf';
import { Button, Card } from '../../design-system/primitives';
import { aiBridge, errorText } from './aiBridge';
import { configurePdfWorker } from './pdfWorker';
import { Anchor, anchorSentences, PageText, Unplaced } from './anchorSentences';
import { AnnotationStatus, STATUS_ORDER, STATUS_STYLE, statusOf } from './annotationStatus';

/** One judged sentence, as `ai_job_results` reports it. */
export interface JudgedItem {
  seq: number;
  kind: string;
  page: number | null;
  sentence: string;
  result?: {
    output?: {
      verdict?: string;
      /** §11 D108. What the highlight now claims: that there are passages to
       *  read. The verdict beside it is a constant and decides nothing. */
      supporting_chunks?: unknown[];
    };
    category?: string;
  } | null;
}

export interface AnnotatedManuscriptProps {
  /** The manuscript the audit ran on. */
  path: string;
  items: JudgedItem[];
  /** Test seam: bytes without IPC. */
  loadBytes?: (path: string) => Promise<Uint8Array>;
  /** Test seam: page text without a real PDF. */
  loadPages?: (pdf: unknown) => Promise<PageText[]>;
  pageWidth?: number;
}

const verdictOf = (it: JudgedItem) => it.result?.output?.verdict ?? null;
/** §11 D108. How many source passages this item recorded. */
const passagesOf = (it: JudgedItem) => it.result?.output?.supporting_chunks?.length ?? 0;

/** Only what can be drawn: a status, and a sentence long enough to locate. */
export function annotatable(items: JudgedItem[]) {
  return items
    .map((it) => ({ it, status: statusOf(it.kind, verdictOf(it), passagesOf(it)) }))
    .filter((x): x is { it: JudgedItem; status: AnnotationStatus } => x.status !== null);
}

/** Read every page's positioned text — ALL pages, because anchoring searches
 *  all of them and the viewer only mounts a window. */
async function readPages(pdf: any): Promise<PageText[]> {
  const out: PageText[] = [];
  for (let n = 1; n <= pdf.numPages; n++) {
    const pg = await pdf.getPage(n);
    const vp = pg.getViewport({ scale: 1 });
    const tc = await pg.getTextContent();
    out.push({
      page: n,
      width: vp.width,
      height: vp.height,
      items: (tc.items as any[])
        .filter((i) => typeof i.str === 'string')
        .map((i) => ({
          str: i.str,
          x: i.transform[4],
          y: i.transform[5],
          w: i.width,
          h: i.height || Math.abs(i.transform[3]) || 10,
          hasEOL: i.hasEOL,
        })),
    });
  }
  return out;
}

export const AnnotatedManuscript: React.FC<AnnotatedManuscriptProps> = ({
  path,
  items,
  loadBytes,
  loadPages,
  pageWidth = 760,
}) => {
  const [bytes, setBytes] = useState<Uint8Array | null>(null);
  const [pages, setPages] = useState<PageText[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => { configurePdfWorker(); }, []);

  useEffect(() => {
    let alive = true;
    setError(null);
    setBytes(null);
    (loadBytes ? loadBytes(path) : aiBridge.manuscriptBytes(path))
      .then((b) => { if (alive) setBytes(b); })
      .catch((e) => { if (alive) setError(errorText(e)); });
    return () => { alive = false; };
  }, [path, loadBytes]);

  const drawable = useMemo(() => annotatable(items), [items]);

  const { anchors, unplaced } = useMemo(() => {
    if (!pages) return { anchors: [] as Anchor[], unplaced: [] as Unplaced[] };
    // Document order is what resolves a sentence the paper prints twice.
    const ordered = [...drawable].sort((a, b) => a.it.seq - b.it.seq);
    return anchorSentences(ordered.map((d) => ({ seq: d.it.seq, sentence: d.it.sentence })), pages);
  }, [pages, drawable]);

  const statusBySeq = useMemo(() => {
    const m = new Map<number, AnnotationStatus>();
    for (const d of drawable) m.set(d.it.seq, d.status);
    return m;
  }, [drawable]);

  const bySeq = useMemo(() => new Map(items.map((i) => [i.seq, i])), [items]);
  const anchorsByPage = useMemo(() => {
    const m = new Map<number, Anchor[]>();
    for (const a of anchors) {
      const list = m.get(a.page) ?? [];
      list.push(a);
      m.set(a.page, list);
    }
    return m;
  }, [anchors]);

  const onLoad = useCallback(async (pdf: any) => {
    try {
      setPages(loadPages ? await loadPages(pdf) : await readPages(pdf));
    } catch (e) {
      setError(errorText(e));
    }
  }, [loadPages]);

  if (error) {
    return (
      <Card title="Annotated manuscript" data-testid="annotated-error">
        <p className="gds-ai__hint" style={{ color: 'var(--g-flagged)' }}>{error}</p>
      </Card>
    );
  }

  return (
    <div data-testid="annotated-manuscript">
      <Card title="Your manuscript, annotated">
        {/* COLOUR IS NEVER THE ONLY SIGNAL: the legend names each status in
            words, and the edge treatment distinguishes them without hue. */}
        <ul className="gds-annot__legend" data-testid="annot-legend">
          {STATUS_ORDER.map((s) => (
            <li key={s} data-testid={`annot-legend-${s}`}>
              <span
                className={`gds-annot__swatch gds-annot__swatch--${STATUS_STYLE[s].edge}`}
                style={{ background: STATUS_STYLE[s].fill }}
                aria-hidden="true"
              />
              {STATUS_STYLE[s].label}
            </li>
          ))}
        </ul>
        <p className="gds-ai__hint" data-testid="annot-counts">
          {anchors.length} of {drawable.length} judged sentence
          {drawable.length === 1 ? '' : 's'} shown on the page
          {unplaced.length > 0 && `; ${unplaced.length} could not be located and are listed below`}.
        </p>

        {bytes && (
          <Document file={{ data: bytes }} onLoadSuccess={onLoad} loading="Opening the manuscript…">
            {(pages ?? []).map((p) => {
              const scale = pageWidth / p.width;
              return (
                <div
                  className="gds-annot__page"
                  key={p.page}
                  data-testid={`annot-page-${p.page}`}
                  style={{ position: 'relative', width: pageWidth }}
                >
                  <Page pageNumber={p.page} width={pageWidth} renderTextLayer renderAnnotationLayer={false} />
                  {(anchorsByPage.get(p.page) ?? []).map((a) => {
                    const st = statusBySeq.get(a.seq);
                    if (!st) return null;
                    const style = STATUS_STYLE[st];
                    return a.lines.map((r, i) => (
                      <div
                        key={`${a.seq}-${i}`}
                        data-testid={`annot-hl-${a.seq}`}
                        data-status={st}
                        title={`${style.label} — ${bySeq.get(a.seq)?.sentence.slice(0, 80) ?? ''}`}
                        style={{
                          position: 'absolute',
                          left: r.x * scale,
                          // PDF y is a bottom-left baseline; CSS top is from the top.
                          top: (p.height - r.y - r.h) * scale,
                          width: r.w * scale,
                          height: r.h * scale + 2,
                          background: style.fill,
                          // §11 D89: the suggestion is the faintest thing here.
                          opacity: style.weight === 'light' ? 0.22 : 0.34,
                          borderLeft: `3px ${style.edge} ${style.fill}`,
                          pointerEvents: 'none',
                        }}
                      />
                    ));
                  })}
                </div>
              );
            })}
          </Document>
        )}
      </Card>

      <Card title="What each highlight means" data-testid="annot-detail">
        {anchors.length === 0 && unplaced.length === 0 && (
          <p className="gds-ai__hint">Nothing was flagged on this manuscript.</p>
        )}
        {anchors.map((a) => {
          const it = bySeq.get(a.seq);
          const st = statusBySeq.get(a.seq);
          if (!it || !st) return null;
          const style = STATUS_STYLE[st];
          return (
            <div className="gds-annot__entry" key={a.seq} data-testid={`annot-entry-${a.seq}`}>
              <p className="gds-annot__entry-head">
                <span
                  className={`gds-annot__swatch gds-annot__swatch--${style.edge}`}
                  style={{ background: style.fill }}
                  aria-hidden="true"
                />
                {/* The word, again — a reader who cannot tell the colours apart
                    still gets the status. */}
                <b>{style.label}</b> · page {a.page}
              </p>
              <p className="gds-annot__sentence">“{it.sentence.trim()}”</p>
              <p className="gds-ai__hint">{style.meaning}</p>
              <p className="gds-ai__hint"><b>What to do:</b> {style.action}</p>
            </div>
          );
        })}

        {unplaced.length > 0 && (
          <div data-testid="annot-unplaced">
            <h4>Judged, but not shown on the page</h4>
            {unplaced.map((u) => {
              const it = bySeq.get(u.seq);
              const st = statusBySeq.get(u.seq);
              if (!it || !st) return null;
              return (
                <div className="gds-annot__entry" key={u.seq} data-testid={`annot-unplaced-${u.seq}`}>
                  <p className="gds-annot__entry-head">
                    <b>{STATUS_STYLE[st].label}</b>
                    {it.page !== null && ` · around page ${it.page}`}
                  </p>
                  <p className="gds-annot__sentence">“{it.sentence.trim()}”</p>
                  <p className="gds-ai__hint">{u.note}</p>
                  <p className="gds-ai__hint"><b>What to do:</b> {STATUS_STYLE[st].action}</p>
                </div>
              );
            })}
          </div>
        )}
      </Card>
    </div>
  );
};

/** Shown instead of the annotated view when the manuscript is a Word file. */
export const AnnotatedUnavailable: React.FC<{ onOpenReport?: () => void }> = ({ onOpenReport }) => (
  <Card title="Annotated view needs a PDF" data-testid="annotated-unavailable">
    <p className="gds-ai__hint">
      This manuscript is a Word file. Word documents don’t record where their pages break — the
      layout is decided by whichever machine opens them — so there is no page for Gaply to
      annotate, and drawing an approximation would put highlights in places your paper does not
      have.
    </p>
    <p className="gds-ai__hint">
      The report below locates every finding by <b>paragraph</b> instead, which is exact for a Word
      file. Export the manuscript as a PDF and re-run the audit to get the annotated view.
    </p>
    {onOpenReport && (
      <Button variant="secondary" onClick={onOpenReport} data-testid="annot-open-report">
        Show the report
      </Button>
    )}
  </Card>
);
