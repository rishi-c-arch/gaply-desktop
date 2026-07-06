import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTheme } from '../../contexts/ThemeContext';
import { sanitizeHtml } from '../../utils/sanitizeHtml';
import './CitationGeneratorPage.css';
import {
  detectInputFormat,
  fetchCrossrefByDoi,
  fetchOpenLibraryByIsbn,
  fetchPubmedByPmid,
  normalizeDoi,
  parseEndNoteXml,
  parsePlainTextReference,
  type CslLike,
} from './citationFetchers';
import {
  type CiteLocale,
  type CiteTemplate,
  ensureCitationPlugins,
  exportBibtex,
  exportData,
  exportRis,
  formatBibliography,
  formatCitation,
  fromCslItems,
  getCiteClass,
  parseToCite,
} from './citeEngine';

const STORAGE_KEY = 'gaply_citation_library_v1';

type SavedRef = { id: string; csl: CslLike; addedAt: number };

function loadLibrary(): SavedRef[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const p = JSON.parse(raw) as SavedRef[];
    return Array.isArray(p) ? p : [];
  } catch {
    return [];
  }
}

function saveLibrary(refs: SavedRef[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(refs.slice(0, 500)));
  } catch {
    /* quota */
  }
}

function fingerprint(c: CslLike): string {
  const doi = String(c.DOI || c.doi || '');
  const pmid = String(c.PMID || '');
  const t = String(c.title || '')
    .toLowerCase()
    .slice(0, 160);
  const y = JSON.stringify(c.issued);
  return `${doi}|${pmid}|${t}|${y}`;
}

function stripHtml(html: string): string {
  const d = document.createElement('div');
  d.innerHTML = html;
  return d.textContent || d.innerText || '';
}

function narrativePreview(c: CslLike): string {
  const authors = (c.author as Array<{ family?: string; given?: string; literal?: string }> | undefined) || [];
  const first = authors[0];
  let name = 'Author';
  if (first?.family) name = first.family;
  else if (first?.literal) name = first.literal.split(',')[0]?.trim() || first.literal;
  const parts = (c.issued as { 'date-parts'?: number[][] } | undefined)?.['date-parts']?.[0];
  const y = parts?.[0] || 'n.d.';
  return `${name} (${y})`;
}

const TEMPLATES: { value: CiteTemplate; label: string }[] = [
  { value: 'apa', label: 'APA 7 (built-in)' },
  { value: 'vancouver', label: 'Vancouver' },
  { value: 'harvard1', label: 'Harvard' },
];

const LOCALES: { value: CiteLocale; label: string }[] = [
  { value: 'en-US', label: 'English (US)' },
  { value: 'es-ES', label: 'Español' },
  { value: 'de-DE', label: 'Deutsch' },
  { value: 'fr-FR', label: 'Français' },
  { value: 'nl-NL', label: 'Nederlands' },
];

const CitationGeneratorPage: React.FC = () => {
  const { theme } = useTheme();
  const [mainTab, setMainTab] = useState<'generator' | 'converter'>('generator');
  const [genTab, setGenTab] = useState<'ids' | 'paste' | 'manual'>('ids');
  const [doi, setDoi] = useState('');
  const [isbn, setIsbn] = useState('');
  const [pmid, setPmid] = useState('');
  const [paste, setPaste] = useState('');
  const [manual, setManual] = useState({
    title: '',
    author: '',
    year: '',
    journal: '',
    doiField: '',
  });
  const [template, setTemplate] = useState<CiteTemplate>('apa');
  const [locale, setLocale] = useState<CiteLocale>('en-US');
  const [items, setItems] = useState<CslLike[]>([]);
  const [library, setLibrary] = useState<SavedRef[]>([]);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [bibHtml, setBibHtml] = useState('');
  const [bibText, setBibText] = useState('');
  const [intext, setIntext] = useState('');
  const [narrative, setNarrative] = useState('');
  const [convOut, setConvOut] = useState('');
  const [convFormat, setConvFormat] = useState<'bibtex' | 'ris' | 'json' | 'bibliography'>('bibtex');
  const [libSearch, setLibSearch] = useState('');
  const [sortBy, setSortBy] = useState<'added' | 'title' | 'year'>('added');

  useEffect(() => {
    setLibrary(loadLibrary());
  }, []);

  useEffect(() => {
    saveLibrary(library);
  }, [library]);

  const refreshOutput = useCallback(
    async (nextItems: CslLike[]) => {
      if (!nextItems.length) {
        setBibHtml('');
        setBibText('');
        setIntext('');
        setNarrative('');
        return;
      }
      setErr(null);
      try {
        const cite = await fromCslItems(nextItems);
        const html = await formatBibliography(cite, template, locale, true);
        const text = await formatBibliography(cite, template, locale, false);
        setBibHtml(html);
        setBibText(text);
        setIntext(await formatCitation(cite));
        setNarrative(narrativePreview(nextItems[0]));
      } catch (e) {
        setErr(e instanceof Error ? e.message : 'Format error');
      }
    },
    [template, locale],
  );

  useEffect(() => {
    if (items.length) void refreshOutput(items);
  }, [items, refreshOutput]);

  const handleFetchDoi = async () => {
    const d = normalizeDoi(doi);
    if (!d) {
      setErr('Enter a DOI');
      return;
    }
    setLoading(true);
    setErr(null);
    try {
      await ensureCitationPlugins();
      let csl: CslLike;
      try {
        csl = await fetchCrossrefByDoi(d);
      } catch {
        const Cite = await getCiteClass();
        const cite = await Cite.async(d);
        const row = cite.data[0] as CslLike | undefined;
        if (!row) throw new Error('Empty DOI response');
        csl = row;
      }
      setItems([csl]);
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'DOI lookup failed');
      setItems([]);
    } finally {
      setLoading(false);
    }
  };

  const handleFetchIsbn = async () => {
    setLoading(true);
    setErr(null);
    try {
      const csl = await fetchOpenLibraryByIsbn(isbn);
      setItems([csl]);
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'ISBN lookup failed');
      setItems([]);
    } finally {
      setLoading(false);
    }
  };

  const handleFetchPmid = async () => {
    setLoading(true);
    setErr(null);
    try {
      const csl = await fetchPubmedByPmid(pmid);
      setItems([csl]);
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'PubMed lookup failed');
      setItems([]);
    } finally {
      setLoading(false);
    }
  };

  const handleParsePaste = async () => {
    setLoading(true);
    setErr(null);
    try {
      await ensureCitationPlugins();
      const det = detectInputFormat(paste);
      if (det === 'xml') {
        const parsed = parseEndNoteXml(paste);
        if (!parsed.length) throw new Error('No records found in XML');
        setItems(parsed);
        return;
      }
      if (det === 'plain') {
        const lines = paste.split(/\n+/).map((l) => l.trim()).filter(Boolean);
        const parsed: CslLike[] = [];
        for (const line of lines) {
          const p = parsePlainTextReference(line);
          if (p) parsed.push(p);
        }
        if (parsed.length) {
          setItems(parsed);
          return;
        }
      }
      const cite = await parseToCite(paste, det === 'json' ? 'json' : undefined);
      setItems((cite.data as CslLike[]) ?? []);
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Could not parse input');
      setItems([]);
    } finally {
      setLoading(false);
    }
  };

  const handleManual = async () => {
    if (!manual.title.trim()) {
      setErr('Title is required');
      return;
    }
    const year = manual.year.trim() ? parseInt(manual.year.trim(), 10) : NaN;
    const authorParts = manual.author
      .split(/;|,/)
      .map((s) => s.trim())
      .filter(Boolean)
      .map((name) => ({ literal: name }));
    const csl: CslLike = {
      type: manual.journal.trim() ? 'article-journal' : 'book',
      id: `manual-${Date.now()}`,
      title: manual.title.trim(),
      author: authorParts.length ? authorParts : [{ literal: 'Unknown' }],
      issued: !Number.isNaN(year) ? { 'date-parts': [[year]] } : undefined,
      'container-title': manual.journal.trim() || undefined,
      DOI: manual.doiField.trim() || undefined,
    };
    setErr(null);
    setItems([csl]);
  };

  const addToLibrary = () => {
    if (!items.length) return;
    const next = [...library];
    const seen = new Set(next.map((r) => fingerprint(r.csl)));
    for (const csl of items) {
      const fp = fingerprint(csl);
      if (seen.has(fp)) continue;
      seen.add(fp);
      next.push({
        id: `saved-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`,
        csl,
        addedAt: Date.now(),
      });
    }
    setLibrary(next);
  };

  const dedupeLibrary = () => {
    const seen = new Set<string>();
    setLibrary(library.filter((r) => {
      const fp = fingerprint(r.csl);
      if (seen.has(fp)) return false;
      seen.add(fp);
      return true;
    }));
  };

  const removeFromLibrary = (id: string) => setLibrary(library.filter((r) => r.id !== id));

  const loadFromLibrary = (csl: CslLike) => setItems([csl]);

  const exportLibraryFile = async (kind: 'bibtex' | 'ris' | 'json') => {
    if (!library.length) return;
    await ensureCitationPlugins();
    const cite = await fromCslItems(library.map((r) => r.csl));
    let body: string;
    let ext: string;
    if (kind === 'bibtex') {
      body = await exportBibtex(cite);
      ext = 'bib';
    } else if (kind === 'ris') {
      body = await exportRis(cite);
      ext = 'ris';
    } else {
      body = await exportData(cite);
      ext = 'json';
    }
    const blob = new Blob([body], { type: 'text/plain;charset=utf-8' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `gaply-references.${ext}`;
    a.click();
    URL.revokeObjectURL(a.href);
  };

  const runConverter = async () => {
    setLoading(true);
    setErr(null);
    setConvOut('');
    try {
      await ensureCitationPlugins();
      const det = detectInputFormat(paste);
      let cite;
      if (det === 'xml') {
        const parsed = parseEndNoteXml(paste);
        if (!parsed.length) throw new Error('No EndNote records parsed');
        cite = await fromCslItems(parsed);
      } else {
        cite = await parseToCite(paste, det === 'json' ? 'json' : undefined);
      }
      if (convFormat === 'bibtex') setConvOut(await exportBibtex(cite));
      else if (convFormat === 'ris') setConvOut(await exportRis(cite));
      else if (convFormat === 'json') setConvOut(await exportData(cite));
      else setConvOut(await formatBibliography(cite, template, locale, false));
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Conversion failed');
    } finally {
      setLoading(false);
    }
  };

  const filteredLibrary = useMemo(() => {
    let rows = [...library];
    const q = libSearch.trim().toLowerCase();
    if (q) {
      rows = rows.filter((r) => JSON.stringify(r.csl).toLowerCase().includes(q));
    }
    rows.sort((a, b) => {
      if (sortBy === 'added') return b.addedAt - a.addedAt;
      const ta = String(a.csl.title || '');
      const tb = String(b.csl.title || '');
      if (sortBy === 'title') return ta.localeCompare(tb);
      const ya =
        (a.csl.issued as { 'date-parts'?: number[][] } | undefined)?.['date-parts']?.[0]?.[0] || 0;
      const yb =
        (b.csl.issued as { 'date-parts'?: number[][] } | undefined)?.['date-parts']?.[0]?.[0] || 0;
      return yb - ya;
    });
    return rows;
  }, [library, libSearch, sortBy]);

  const onDropFile = (e: React.DragEvent) => {
    e.preventDefault();
    const f = e.dataTransfer.files[0];
    if (!f) return;
    const r = new FileReader();
    r.onload = () => setPaste(String(r.result || ''));
    r.readAsText(f);
  };

  return (
    <div className="gaply-cite-page" data-theme={theme}>
      <div className="gaply-cite-page__noise" aria-hidden />
      <div className="gaply-cite-page__bg" />

      <div className="gaply-cite-page__inner">
        <header className="gaply-cite-page__hero">
          <span className="gaply-cite-page__badge">100% free · No sign-in · Runs in your browser</span>
          <h1 className="gaply-cite-page__title">Citation generator &amp; reference converter</h1>
          <p className="gaply-cite-page__subtitle">
            Build citations and convert BibTeX, RIS, CSL-JSON, and basic EndNote XML using Citation.js (MIT) with built-in APA,
            Vancouver, and Harvard styles. Metadata from Crossref, Open Library, and PubMed where available. Saved lists use
            local storage only on your device.
          </p>
        </header>

        <div className="gaply-cite-page__tabs" role="tablist">
          <button
            type="button"
            role="tab"
            aria-selected={mainTab === 'generator'}
            className={`gaply-cite-page__tab ${mainTab === 'generator' ? 'gaply-cite-page__tab--on' : ''}`}
            onClick={() => setMainTab('generator')}
          >
            Citation generator
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={mainTab === 'converter'}
            className={`gaply-cite-page__tab ${mainTab === 'converter' ? 'gaply-cite-page__tab--on' : ''}`}
            onClick={() => setMainTab('converter')}
          >
            Reference converter
          </button>
        </div>

        {err && <div className="gaply-cite-page__error">{err}</div>}

        {mainTab === 'generator' && (
          <>
            <div className="gaply-cite-page__subtabs">
              <button
                type="button"
                className={genTab === 'ids' ? 'gaply-cite-page__subtab--on' : ''}
                onClick={() => setGenTab('ids')}
              >
                DOI / ISBN / PubMed
              </button>
              <button
                type="button"
                className={genTab === 'paste' ? 'gaply-cite-page__subtab--on' : ''}
                onClick={() => setGenTab('paste')}
              >
                Paste or file
              </button>
              <button
                type="button"
                className={genTab === 'manual' ? 'gaply-cite-page__subtab--on' : ''}
                onClick={() => setGenTab('manual')}
              >
                Manual entry
              </button>
            </div>

            <section className="gaply-cite-page__panel">
              {genTab === 'ids' && (
                <div className="gaply-cite-page__grid3">
                  <div className="gaply-cite-page__field">
                    <label htmlFor="gc-doi">DOI</label>
                    <input
                      id="gc-doi"
                      value={doi}
                      onChange={(e) => setDoi(e.target.value)}
                      placeholder="10.1038/nature12373"
                    />
                    <button type="button" className="gaply-cite-page__btn" disabled={loading} onClick={handleFetchDoi}>
                      Lookup (Crossref)
                    </button>
                  </div>
                  <div className="gaply-cite-page__field">
                    <label htmlFor="gc-isbn">ISBN</label>
                    <input id="gc-isbn" value={isbn} onChange={(e) => setIsbn(e.target.value)} placeholder="9780262033848" />
                    <button type="button" className="gaply-cite-page__btn" disabled={loading} onClick={handleFetchIsbn}>
                      Lookup (Open Library)
                    </button>
                  </div>
                  <div className="gaply-cite-page__field">
                    <label htmlFor="gc-pmid">PubMed ID</label>
                    <input id="gc-pmid" value={pmid} onChange={(e) => setPmid(e.target.value)} placeholder="30335344" />
                    <button type="button" className="gaply-cite-page__btn" disabled={loading} onClick={handleFetchPmid}>
                      Lookup (NCBI)
                    </button>
                  </div>
                </div>
              )}

              {genTab === 'paste' && (
                <div
                  className="gaply-cite-page__drop"
                  onDragOver={(e) => e.preventDefault()}
                  onDrop={onDropFile}
                >
                  <label className="gaply-cite-page__label-block" htmlFor="gc-paste">
                    BibTeX, RIS, CSL-JSON, or EndNote XML — or drop a .bib / .ris / .xml file
                  </label>
                  <textarea
                    id="gc-paste"
                    className="gaply-cite-page__textarea"
                    value={paste}
                    onChange={(e) => setPaste(e.target.value)}
                    rows={8}
                    spellCheck={false}
                  />
                  <button type="button" className="gaply-cite-page__btn" disabled={loading} onClick={handleParsePaste}>
                    Parse &amp; preview
                  </button>
                </div>
              )}

              {genTab === 'manual' && (
                <div className="gaply-cite-page__manual">
                  <div className="gaply-cite-page__field">
                    <label htmlFor="gc-title">Title *</label>
                    <input id="gc-title" value={manual.title} onChange={(e) => setManual({ ...manual, title: e.target.value })} />
                  </div>
                  <div className="gaply-cite-page__field">
                    <label htmlFor="gc-author">Authors (comma or semicolon separated)</label>
                    <input
                      id="gc-author"
                      value={manual.author}
                      onChange={(e) => setManual({ ...manual, author: e.target.value })}
                    />
                  </div>
                  <div className="gaply-cite-page__row2">
                    <div className="gaply-cite-page__field">
                      <label htmlFor="gc-year">Year</label>
                      <input id="gc-year" value={manual.year} onChange={(e) => setManual({ ...manual, year: e.target.value })} />
                    </div>
                    <div className="gaply-cite-page__field">
                      <label htmlFor="gc-journal">Journal / book</label>
                      <input
                        id="gc-journal"
                        value={manual.journal}
                        onChange={(e) => setManual({ ...manual, journal: e.target.value })}
                      />
                    </div>
                  </div>
                  <div className="gaply-cite-page__field">
                    <label htmlFor="gc-mdoi">DOI (optional)</label>
                    <input
                      id="gc-mdoi"
                      value={manual.doiField}
                      onChange={(e) => setManual({ ...manual, doiField: e.target.value })}
                    />
                  </div>
                  <button type="button" className="gaply-cite-page__btn" onClick={handleManual}>
                    Add to preview
                  </button>
                </div>
              )}
            </section>

            <section className="gaply-cite-page__panel gaply-cite-page__panel--tools">
              <div className="gaply-cite-page__row2">
                <div className="gaply-cite-page__field">
                  <label htmlFor="gc-style">Citation style</label>
                  <select id="gc-style" value={template} onChange={(e) => setTemplate(e.target.value as CiteTemplate)}>
                    {TEMPLATES.map((t) => (
                      <option key={t.value} value={t.value}>
                        {t.label}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="gaply-cite-page__field">
                  <label htmlFor="gc-locale">Locale</label>
                  <select id="gc-locale" value={locale} onChange={(e) => setLocale(e.target.value as CiteLocale)}>
                    {LOCALES.map((l) => (
                      <option key={l.value} value={l.value}>
                        {l.label}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
              <div className="gaply-cite-page__actions">
                <button type="button" className="gaply-cite-page__btn gaply-cite-page__btn--ghost" disabled={!bibText} onClick={() => navigator.clipboard.writeText(bibText)}>
                  Copy bibliography (plain)
                </button>
                <button
                  type="button"
                  className="gaply-cite-page__btn gaply-cite-page__btn--ghost"
                  disabled={!bibHtml}
                  onClick={() => navigator.clipboard.writeText(stripHtml(bibHtml))}
                >
                  Copy bibliography (text from HTML)
                </button>
                <button type="button" className="gaply-cite-page__btn gaply-cite-page__btn--ghost" disabled={!intext} onClick={() => navigator.clipboard.writeText(intext)}>
                  Copy in-text (parenthetical)
                </button>
                <button type="button" className="gaply-cite-page__btn gaply-cite-page__btn--ghost" disabled={!narrative} onClick={() => navigator.clipboard.writeText(narrative)}>
                  Copy narrative (first author, year)
                </button>
                <button type="button" className="gaply-cite-page__btn" disabled={!items.length} onClick={addToLibrary}>
                  Add to saved list
                </button>
              </div>
            </section>

            <section className="gaply-cite-page__panel">
              <h2 className="gaply-cite-page__h2">Live preview</h2>
              {loading && <p className="gaply-cite-page__muted">Loading…</p>}
              {!loading && items.length === 0 && <p className="gaply-cite-page__muted">Nothing to show yet.</p>}
              {items.length > 0 && (
                <>
                  <div className="gaply-cite-page__preview gaply-cite-page__preview--intext">
                    <span className="gaply-cite-page__preview-label">In-text</span>
                    <code>{intext || '—'}</code>
                  </div>
                  <div className="gaply-cite-page__preview gaply-cite-page__preview--narrative">
                    <span className="gaply-cite-page__preview-label">Narrative hint</span>
                    <code>{narrative || '—'}</code>
                  </div>
                  <div
                    className="gaply-cite-page__preview gaply-cite-page__preview--bib gaply-cite-bibliography"
                    dangerouslySetInnerHTML={{ __html: sanitizeHtml(bibHtml || '<p>—</p>') }}
                  />
                </>
              )}
            </section>
          </>
        )}

        {mainTab === 'converter' && (
          <section className="gaply-cite-page__panel">
            <p className="gaply-cite-page__muted">
              Paste references or drop a file. Output style for “Formatted bibliography” uses the style and locale selected
              above in the generator tab.
            </p>
            <div className="gaply-cite-page__row2">
              <div className="gaply-cite-page__field">
                <label htmlFor="cv-fmt">Output</label>
                <select id="cv-fmt" value={convFormat} onChange={(e) => setConvFormat(e.target.value as typeof convFormat)}>
                  <option value="bibtex">BibTeX</option>
                  <option value="ris">RIS</option>
                  <option value="json">CSL-JSON</option>
                  <option value="bibliography">Formatted bibliography (plain)</option>
                </select>
              </div>
            </div>
            <textarea
              className="gaply-cite-page__textarea"
              value={paste}
              onChange={(e) => setPaste(e.target.value)}
              rows={12}
              spellCheck={false}
              placeholder="Paste BibTeX, RIS, CSL-JSON, or EndNote XML…"
            />
            <button type="button" className="gaply-cite-page__btn" disabled={loading} onClick={runConverter}>
              Convert
            </button>
            {convOut && (
              <>
                <h2 className="gaply-cite-page__h2">Output</h2>
                <pre className="gaply-cite-page__pre">{convOut}</pre>
                <button
                  type="button"
                  className="gaply-cite-page__btn gaply-cite-page__btn--ghost"
                  onClick={() => navigator.clipboard.writeText(convOut)}
                >
                  Copy output
                </button>
              </>
            )}
          </section>
        )}

        <section className="gaply-cite-page__panel">
          <h2 className="gaply-cite-page__h2">Saved list ({library.length})</h2>
          <p className="gaply-cite-page__muted">
            Stored in this browser only (<code className="gaply-cite-page__code">{STORAGE_KEY}</code>).
          </p>
          <div className="gaply-cite-page__libtools">
            <input
              type="search"
              className="gaply-cite-page__input"
              placeholder="Search saved…"
              value={libSearch}
              onChange={(e) => setLibSearch(e.target.value)}
            />
            <select value={sortBy} onChange={(e) => setSortBy(e.target.value as typeof sortBy)}>
              <option value="added">Newest saved</option>
              <option value="title">Title A–Z</option>
              <option value="year">Year</option>
            </select>
            <button type="button" className="gaply-cite-page__btn gaply-cite-page__btn--ghost" onClick={dedupeLibrary}>
              Remove duplicates
            </button>
            <button type="button" className="gaply-cite-page__btn gaply-cite-page__btn--ghost" onClick={() => exportLibraryFile('bibtex')} disabled={!library.length}>
              Export .bib
            </button>
            <button type="button" className="gaply-cite-page__btn gaply-cite-page__btn--ghost" onClick={() => exportLibraryFile('ris')} disabled={!library.length}>
              Export .ris
            </button>
            <button type="button" className="gaply-cite-page__btn gaply-cite-page__btn--ghost" onClick={() => exportLibraryFile('json')} disabled={!library.length}>
              Export .json
            </button>
            <button type="button" className="gaply-cite-page__btn gaply-cite-page__btn--ghost" onClick={() => setLibrary([])} disabled={!library.length}>
              Clear all
            </button>
          </div>
          <ul className="gaply-cite-page__liblist">
            {filteredLibrary.map((r) => (
              <li key={r.id} className="gaply-cite-page__libitem">
                <div className="gaply-cite-page__libmain">
                  <strong>{String(r.csl.title || 'Untitled')}</strong>
                  <span className="gaply-cite-page__muted">
                    {String((r.csl.author as { literal?: string }[])?.[0]?.literal || '')}{' '}
                    {(r.csl.issued as { 'date-parts'?: number[][] } | undefined)?.['date-parts']?.[0]?.[0] || ''}
                  </span>
                </div>
                <div className="gaply-cite-page__libactions">
                  <button type="button" className="gaply-cite-page__btn--mini" onClick={() => loadFromLibrary(r.csl)}>
                    Load
                  </button>
                  <button type="button" className="gaply-cite-page__btn--mini" onClick={() => removeFromLibrary(r.id)}>
                    Remove
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </section>
      </div>
    </div>
  );
};

export default CitationGeneratorPage;
