import React, { useMemo, useState, useRef, useCallback } from 'react';
import SEO from './SEO';
import { apiFetch } from '../api/config';
import { useAuth } from '../contexts/AuthContext';
import MarkdownRenderer from './MarkdownRenderer';
import { downloadHTMLReport, downloadReportAsPDF } from './HTMLReportGenerator';
import { createReport } from '../services/dashboardService';
import { playHumanTTS, HumanTTSController } from '../utils/humanTTS';
import './ManuscriptOrchestratorPage.css';

type UploadFile = {
  file: File;
  id: string;
};

type ChatMessage = {
  id: string;
  role: 'user' | 'assistant';
  text: string;
  timestamp: string;
};

const ACCEPTED_TYPES = ['.pdf', '.docx', '.txt'];

function AudioStoryModal({
  reportContext,
  manuscriptText,
  journalLink,
  reportData,
  analysisJobId,
  onClose,
}: {
  reportContext: string;
  manuscriptText: string;
  journalLink: string;
  reportData: any;
  analysisJobId: string;
  onClose: () => void;
}) {
  const [loading, setLoading] = useState(false);
  const [scriptEn, setScriptEn] = useState('');
  const [scriptHi, setScriptHi] = useState('');
  const [lang, setLang] = useState<'en' | 'hi'>('en');
  const [playing, setPlaying] = useState(false);
  const [paused, setPaused] = useState(false);
  const [error, setError] = useState('');
  const ttsControllerRef = useRef<HumanTTSController | null>(null);

  const generate = useCallback(async () => {
    setError('');
    setLoading(true);
    try {
      const aiHeaders: Record<string, string> = analysisJobId ? { 'X-Gaply-Job-Id': analysisJobId } : {};
      const res = await apiFetch('/api/ai/audio-story', {
        method: 'POST',
        headers: Object.keys(aiHeaders).length ? aiHeaders : undefined,
        body: JSON.stringify({
          report_context: reportContext,
          manuscript_text: (manuscriptText || '').slice(0, 12000),
          journal_url: journalLink,
          report_data: reportData,
        }),
      });
      const text = await res.text();
      let data: any = null;
      try {
        data = text ? JSON.parse(text) : null;
      } catch {
        // non-JSON response (e.g. HTML 404); fall through
      }
      if (!res.ok) {
        const friendly =
          res.status === 401
            ? 'Session expired. Please log out and log in again.'
            : res.status === 502 || res.status === 503
              ? `Orchestrator temporarily unavailable (${res.status}). Ensure the orchestrator is deployed and ORCHESTRATOR_URL is set on the backend.`
              : (data && (data.error || data.message)) || `Audio story request failed (${res.status}). Please try again.`;
        setError(typeof friendly === 'string' && friendly.length > 400 ? 'Orchestrator temporarily unavailable. Please try again.' : friendly);
        return;
      }
      setScriptEn(data?.script_en || data?.script || '');
      setScriptHi(data?.script_hi || '');
    } catch (e: any) {
      setError(e?.message || 'Failed to generate audio story.');
    } finally {
      setLoading(false);
    }
  }, [reportContext, manuscriptText, journalLink, reportData, analysisJobId]);

  const playOrResume = useCallback(() => {
    const script = lang === 'hi' ? scriptHi : scriptEn;
    if (!script.trim()) return;
    setError('');

    // Resume if paused
    if (paused && ttsControllerRef.current) {
      ttsControllerRef.current.resume();
      setPlaying(true);
      setPaused(false);
      return;
    }

    // Fresh play
    if (ttsControllerRef.current) {
      ttsControllerRef.current.stop();
      ttsControllerRef.current = null;
    }
    setPlaying(true);
    setPaused(false);
    playHumanTTS(script, {
      voice: 'shimmer',
      apiFetch,
      jobId: analysisJobId,
      onEnd: () => {
        ttsControllerRef.current = null;
        setPlaying(false);
        setPaused(false);
      },
      onError: (err) => {
        setError(err);
        setPlaying(false);
        setPaused(false);
      },
    }).then((ctrl) => {
      ttsControllerRef.current = ctrl;
    });
  // eslint-disable-next-line react-hooks/exhaustive-deps -- apiFetch is stable from config
  }, [lang, scriptEn, scriptHi, paused, analysisJobId]);

  const pause = useCallback(() => {
    if (ttsControllerRef.current) {
      ttsControllerRef.current.pause();
      setPlaying(false);
      setPaused(true);
    }
  }, []);

  const stop = useCallback(() => {
    if (ttsControllerRef.current) {
      ttsControllerRef.current.stop();
      ttsControllerRef.current = null;
    }
    setPlaying(false);
    setPaused(false);
  }, []);

  const script = lang === 'hi' ? scriptHi : scriptEn;
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content referee-modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 560 }}>
        <div className="modal-header">
          <div>
            <h2>Listen to Analysis</h2>
            <p className="modal-subtitle">What’s lacking, how to improve, and publication chances — in simple language</p>
          </div>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>
        <div className="modal-body-referee" style={{ padding: 20 }}>
          {error && <p style={{ color: '#d70015', marginBottom: 12 }}>{error}</p>}
          {!scriptEn && !scriptHi && !loading && (
            <button className="primary" onClick={generate} style={{ marginBottom: 12 }}>
              Generate audio story
            </button>
          )}
          {loading && <p style={{ color: '#6e6e73' }}>Generating deep analysis narrative…</p>}
          {(scriptEn || scriptHi) && (
            <>
              <div style={{ display: 'flex', gap: 10, marginBottom: 12 }}>
                <button
                  className={lang === 'en' ? 'primary' : 'secondary'}
                  onClick={() => setLang('en')}
                >
                  English
                </button>
                <button
                  className={lang === 'hi' ? 'primary' : 'secondary'}
                  onClick={() => setLang('hi')}
                >
                  हिंदी
                </button>
              </div>
              <div style={{ maxHeight: 200, overflowY: 'auto', marginBottom: 12, fontSize: 14, lineHeight: 1.6 }}>
                {script || '(No script for this language)'}
              </div>
              <div style={{ display: 'flex', gap: 8 }}>
                <button
                  className="primary"
                  onClick={playOrResume}
                  disabled={!script.trim()}
                >
                  {!playing && paused ? 'Resume' : 'Play'}
                </button>
                {playing && (
                  <button className="secondary" onClick={pause}>
                    Pause
                  </button>
                )}
                {(playing || paused) && (
                  <button className="secondary" onClick={stop}>
                    Stop
                  </button>
                )}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export interface ManuscriptOrchestratorPageProps {
  /** Called before analysis starts; return false to abort (e.g. premium use check) */
  onBeforeSubmit?: () => Promise<boolean>;
}

const ManuscriptOrchestratorPage: React.FC<ManuscriptOrchestratorPageProps> = ({ onBeforeSubmit }) => {
  const { token } = useAuth();
  const [files, setFiles] = useState<UploadFile[]>([]);
  const [pastedText, setPastedText] = useState('');
  const [journalLink, setJournalLink] = useState('');
  const [manuscriptLink, setManuscriptLink] = useState('');
  const [usePastedText, setUsePastedText] = useState(false);
  const [status, setStatus] = useState<'idle' | 'processing' | 'complete' | 'error'>('idle');
  const [statusMessage, setStatusMessage] = useState('Ready for upload.');
  const [, setResultPayload] = useState<string>('');
  const [reportContext, setReportContext] = useState<string>('');
  const [reportData, setReportData] = useState<any>(null);
  const [, setRefereeReview] = useState<string>('');
  const [, setLineReview] = useState<string>('');
  const [manuscriptText, setManuscriptText] = useState<string>('');
  const [processingChunks, setProcessingChunks] = useState<{id: string; name: string; status: 'pending' | 'processing' | 'complete'}[]>([]);
  const [currentChunkIndex, setCurrentChunkIndex] = useState(0);
  const [, setRefereeExpanded] = useState(false);
  const [showGetSetGo, setShowGetSetGo] = useState(false);
  const [activeModal, setActiveModal] = useState<'download' | 'chat' | 'audio-story' | 'citation-check' | null>(null);
  const [citationCheck, setCitationCheck] = useState<any>(null);
  const [analysisJobId, setAnalysisJobId] = useState<string>('');
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [chatInput, setChatInput] = useState('');
  const [isRecording, setIsRecording] = useState(false);
  const [speakingId, setSpeakingId] = useState<string | null>(null);
  const recognitionRef = useRef<any>(null);
  const chatTTSControllerRef = useRef<HumanTTSController | null>(null);

  const canSubmit = useMemo(() => {
    return files.length > 0 || pastedText.trim().length > 0;
  }, [files, pastedText]);

  const handleFiles = (fileList: FileList | null) => {
    if (!fileList) return;
    const next: UploadFile[] = [];
    Array.from(fileList).forEach((file) => {
      const ext = `.${file.name.split('.').pop() || ''}`.toLowerCase();
      if (ACCEPTED_TYPES.includes(ext)) {
        next.push({ file, id: `${file.name}-${file.size}-${Date.now()}` });
      }
    });
    if (next.length) {
      setFiles((prev) => [...prev, ...next]);
    }
  };

  const removeFile = (id: string) => {
    setFiles((prev) => prev.filter((item) => item.id !== id));
  };

  const resetAll = () => {
    setFiles([]);
    setPastedText('');
    setJournalLink('');
    setManuscriptLink('');
    setUsePastedText(false);
    setStatus('idle');
    setStatusMessage('Ready for upload.');
    setResultPayload('');
    setReportContext('');
    setReportData(null);
    setRefereeReview('');
    setLineReview('');
    setCitationCheck(null);
    setChatMessages([]);
    setChatInput('');
    setProcessingChunks([]);
    setCurrentChunkIndex(0);
    setRefereeExpanded(false);
  };

  // Detect if content is raw PDF/binary so we never render it in the UI
  const isLikelyBinaryOrCorrupted = (text: string): boolean => {
    if (!text || text.length < 50) return false;
    const sample = text.slice(0, 4000);
    if (/\/FlateDecode|\/Length\s+\d+|%PDF|<<\s*\/Filter/.test(sample)) return true;
    const printable = (sample.match(/[\x20-\x7E\r\n\t]/g) || []).length;
    return printable / sample.length < 0.7;
  };

  // Generate meaningful chunk names; never show binary/corrupted content in UI
  const generateChunkName = (text: string, index: number): string => {
    if (!text || isLikelyBinaryOrCorrupted(text)) return `Section ${index + 1}`;
    const firstLine = text.split('\n')[0].trim();
    if (firstLine.length > 0 && firstLine.length < 60) {
      if (/[^\x20-\x7E\r\n\t]/.test(firstLine)) return `Section ${index + 1}`;
      if (firstLine === firstLine.toUpperCase() || /^[A-Z][a-z]+/.test(firstLine)) {
        return firstLine.length > 50 ? firstLine.slice(0, 47) + '...' : firstLine;
      }
    }
    const sentences = text.split(/[.!?]\s+/).filter(s => s.trim().length > 20);
    if (sentences.length > 0) {
      const name = sentences[0].trim();
      if (/[^\x20-\x7E\r\n\t]/.test(name)) return `Section ${index + 1}`;
      return name.length > 50 ? name.slice(0, 47) + '...' : name;
    }
    return `Section ${index + 1}`;
  };

  const buildChunks = (text: string) => {
    const chunks: { id: string; position: number; text: string }[] = [];
    const maxSize = 1800;
    const paragraphs = text.split(/\n{2,}/).map((p) => p.trim()).filter(Boolean);
    let buffer = '';
    let position = 1;

    for (const paragraph of paragraphs) {
      if ((buffer + '\n\n' + paragraph).length <= maxSize) {
        buffer = buffer ? `${buffer}\n\n${paragraph}` : paragraph;
        continue;
      }
      if (buffer) {
        chunks.push({ id: `chunk-${position}`, position, text: buffer });
        position += 1;
        buffer = paragraph;
      } else {
        // Fallback split by sentence if a single paragraph is too large
        const sentences = paragraph.split(/(?<=[.!?])\s+/);
        let sentenceBuf = '';
        for (const sentence of sentences) {
          if ((sentenceBuf + ' ' + sentence).length <= maxSize) {
            sentenceBuf = sentenceBuf ? `${sentenceBuf} ${sentence}` : sentence;
            continue;
          }
          chunks.push({ id: `chunk-${position}`, position, text: sentenceBuf });
          position += 1;
          sentenceBuf = sentence;
        }
        if (sentenceBuf) {
          chunks.push({ id: `chunk-${position}`, position, text: sentenceBuf });
          position += 1;
        }
        buffer = '';
      }
    }

    if (buffer) {
      chunks.push({ id: `chunk-${position}`, position, text: buffer });
    }

    return chunks;
  };

  const runWithConcurrency = async <T, R>(
    items: T[],
    concurrency: number,
    worker: (item: T, index: number) => Promise<R>,
  ) => {
    const results: R[] = new Array(items.length);
    let nextIndex = 0;

    const runners = Array.from({ length: Math.max(1, concurrency) }, async () => {
      while (true) {
        const current = nextIndex;
        nextIndex += 1;
        if (current >= items.length) return;
        results[current] = await worker(items[current], current);
      }
    });

    await Promise.all(runners);
    return results;
  };

  const handleSubmit = async () => {
    if (!canSubmit) {
      setStatus('error');
      setStatusMessage('Please upload a file or paste text.');
      return;
    }

    if (!token) {
      setStatus('error');
      setStatusMessage('Please log in to use PublishReady.');
      return;
    }

    const consumeRes = await apiFetch('/api/dashboard/consume-publishready', { method: 'POST' });
    if (!consumeRes.ok) {
      setStatus('error');
      setStatusMessage(
        consumeRes.status === 403
          ? 'No remaining PublishReady uses. Please upgrade your plan.'
          : 'Unable to start analysis. Please try again.'
      );
      return;
    }
    let consumptionToken = '';
    try {
      const consumeJson = await consumeRes.json();
      consumptionToken = consumeJson?.consumption_token || '';
    } catch {
      /* ignore */
    }

    if (onBeforeSubmit) {
      const ok = await onBeforeSubmit();
      if (!ok) {
        setStatus('error');
        setStatusMessage('Unable to use PublishReady. Please check your premium plan or upgrade.');
        return;
      }
    }

    setStatus('processing');
    setStatusMessage('Analyzing manuscript and generating report...');

    try {
      let sourceText = '';

      if (usePastedText && pastedText.trim()) {
        sourceText = pastedText.trim();
      } else if (files.length > 0) {
        const formData = new FormData();
        formData.append('document', files[0].file);
        const uploadRes = await apiFetch('/api/v1/upload', {
          method: 'POST',
          body: formData,
          headers: {},
        });
        const uploadText = await uploadRes.text();
        let uploadJson: any = {};
        try {
          uploadJson = uploadText ? JSON.parse(uploadText) : {};
        } catch {
          uploadJson = { error: uploadRes.status === 503 ? 'Upload service is temporarily unavailable.' : 'Upload failed.' };
        }
        if (!uploadRes.ok) {
          const msg = uploadJson?.error || (uploadRes.status === 503 ? 'Upload service is temporarily unavailable. Try again or paste your text below.' : 'Upload failed.');
          throw new Error(msg);
        }
        sourceText = uploadJson.extracted_text || '';
        if (!sourceText) {
          throw new Error('No text extracted from upload. Try pasting text.');
        }
        if (isLikelyBinaryOrCorrupted(sourceText)) {
          throw new Error(
            'The uploaded file did not produce readable text (possible PDF extraction issue). Please paste your manuscript text below instead, or try a different file.'
          );
        }
      }

      if (!sourceText) {
        throw new Error('Please upload a file or paste manuscript text.');
      }

      // Store manuscript text for chat context
      setManuscriptText(sourceText);

      const jobId = `job-${Date.now()}`;
      setAnalysisJobId(jobId);
      const aiHeaders: Record<string, string> = { 'X-Gaply-Job-Id': jobId };
      if (consumptionToken) aiHeaders['X-Consumption-Token'] = consumptionToken;

      const chunks = buildChunks(sourceText);
      
      // Initialize processing chunks with meaningful names
      const chunkNames = chunks.map((chunk, idx) => ({
        id: chunk.id,
        name: generateChunkName(chunk.text, idx),
        status: 'pending' as const,
      }));
      setProcessingChunks(chunkNames);
      setCurrentChunkIndex(0);
      
      setStatusMessage('Fetching journal guidelines and research context...');
      const [guidelinesRes, retrievedRes] = await Promise.all([
        apiFetch('/api/ai/guidelines', {
          headers: aiHeaders,
          method: 'POST',
          body: JSON.stringify({ journal_url: journalLink }),
        }),
        apiFetch('/api/ai/retrieved-docs', {
          method: 'POST',
          headers: aiHeaders,
          body: JSON.stringify({
            query: '',
            manuscript_text: sourceText,
            limit: 100,
          }),
        }),
      ]);
      const safeJson = async (res: Response): Promise<any> => {
        const t = await res.text();
        if (!res.ok) return null;
        try { return t?.trim() ? JSON.parse(t) : null; } catch { return null; }
      };
      const [guidelinesJson, retrievedJson] = await Promise.all([
        safeJson(guidelinesRes),
        safeJson(retrievedRes),
      ]);
      const retrievedDocs = retrievedJson?.retrieved_docs || [];
      const slimRetrievedDocs = retrievedDocs
        .slice(0, 20)
        .map((doc: any) => ({
          ...doc,
          snippet: typeof doc.snippet === 'string' ? doc.snippet.slice(0, 350) : doc.snippet,
        }));
      setStatusMessage(`Analyzing ${chunks.length} sections...`);
      const chunkResults = await runWithConcurrency(
        chunks,
        1, // Process one chunk at a time to avoid overloading and 502s
        async (chunk, index) => {
          setCurrentChunkIndex(index);
          setProcessingChunks(prev => prev.map((c, i) => 
            i === index ? { ...c, status: 'processing' } : 
            i < index ? { ...c, status: 'complete' } : c
          ));
          setStatusMessage(`Analyzing section ${index + 1} of ${chunks.length}: ${chunkNames[index].name}...`);
          const payload = {
            job_id: jobId,
            chunk_id: chunk.id,
            chunk_text: chunk.text,
            chunk_position: chunk.position,
            chunk_token_estimate: Math.floor(chunk.text.length / 4),
            journal_guidelines: guidelinesJson || { journal_url: journalLink },
            retrieved_docs: slimRetrievedDocs,
            tasks: ['guideline_check', 'novelty_check', 'plagiarism_check', 'ai_use_detection', 'suggest_edits', 'writing_quality', 'research_quality'],
            max_tokens_for_response: 2400,
          };

          const DOC_ORCHESTRATOR_TIMEOUT_MS = 600000; // 10 min per chunk (Railway max ~15 min)
          const MAX_ATTEMPTS = 5;
          let text = '';
          let res: Response | null = null;
          for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
            const ac = new AbortController();
            const timeoutId = setTimeout(() => ac.abort(), DOC_ORCHESTRATOR_TIMEOUT_MS);
            try {
              res = await apiFetch('/api/ai/document-orchestrator', {
                method: 'POST',
                headers: aiHeaders,
                body: JSON.stringify(payload),
                signal: ac.signal,
              });
              clearTimeout(timeoutId);
              text = await res.text();
              if (res.ok) break;
              if (res.status === 502 || res.status === 503) {
                if (attempt < MAX_ATTEMPTS) {
                  const backoffMs = index === 0
                    ? Math.min(8000 * attempt, 60000)
                    : Math.min(4000 * Math.pow(2, attempt - 1), 30000);
                  setStatusMessage(`Section ${index + 1} temporarily unavailable, retrying (${attempt}/${MAX_ATTEMPTS}) in ${backoffMs / 1000}s...`);
                  await new Promise((r) => setTimeout(r, backoffMs));
                  continue;
                }
              }
              const msg = res.status === 502 || res.status === 503
                ? `Analysis service temporarily unavailable (${res.status}). Check that the orchestrator is deployed and ORCHESTRATOR_URL is set on the backend.`
                : (text && text.length < 200 ? text : `Analysis failed (${res.status}). Try again.`);
              throw new Error(msg);
            } catch (err: any) {
              clearTimeout(timeoutId);
              const isRetryable = err?.message?.includes('aborted') || err?.message?.includes('fetch') || err?.message?.includes('502') || err?.message?.includes('503') || err?.message?.includes('network');
              if (attempt < MAX_ATTEMPTS && isRetryable) {
                const backoffMs = index === 0
                  ? Math.min(8000 * attempt, 60000)
                  : Math.min(4000 * Math.pow(2, attempt - 1), 30000);
                setStatusMessage(`Section ${index + 1} retrying (${attempt}/${MAX_ATTEMPTS}) in ${backoffMs / 1000}s...`);
                await new Promise((r) => setTimeout(r, backoffMs));
                continue;
              }
              break;
            }
          }
          let result: any;
          if (!res?.ok) {
            // Return placeholder result so analysis can continue with other chunks
            console.warn(`Section ${index + 1} failed after ${MAX_ATTEMPTS} attempts; using placeholder.`);
            result = {
              status: 'partial',
              section_name: chunkNames[index]?.name || `Section ${index + 1}`,
              chunk_id: chunk.id,
              task_results: {
                suggest_edits: { task_status: 'failed', summary: 'Analysis unavailable for this section.', details: [], confidence: 0 },
                writing_quality: { task_status: 'failed', summary: 'Analysis unavailable.', details: {}, confidence: 0 },
                research_quality: { task_status: 'failed', summary: 'Analysis unavailable.', details: {}, confidence: 0 },
              },
              warnings: ['Section analysis failed due to service timeout; partial results shown.'],
            };
          } else {
            try {
              result = text && text.trim() ? JSON.parse(text) : {};
            } catch {
              result = {
                status: 'partial',
                section_name: chunkNames[index]?.name || `Section ${index + 1}`,
                chunk_id: chunk.id,
                task_results: {},
                warnings: ['Invalid response; partial results shown.'],
              };
            }
          }
          
          // Mark chunk as complete
          setProcessingChunks(prev => prev.map((c, i) => 
            i === index ? { ...c, status: 'complete' } : c
          ));
          
          return result;
        },
      );

      const publicationPayload = {
        email: 'demo@gaply.in', // Default email for backend
        journal_url: journalLink,
        journal_name: journalLink ? 'Target Journal' : '',
        manuscript_summary: sourceText.slice(0, 1200),
        journal_guidelines: journalLink ? { journal_url: journalLink } : null,
      };
      setStatusMessage('Finalizing referee review and line-by-line corrections...');

      const refereePayload = {
        journal_url: journalLink,
        manuscript_link: manuscriptLink,
        manuscript_text: sourceText,
        journal_guidelines: guidelinesJson,
        retrieved_docs: retrievedDocs,
      };
      const [pubText, refereeText, lineText, citationText] = await Promise.all([
        apiFetch('/api/ai/publication-chance', {
          method: 'POST',
          headers: aiHeaders,
          body: JSON.stringify(publicationPayload),
        }).then(async (res) => {
          const t = await res.text();
          if (!res.ok) return JSON.stringify({ error: t || `HTTP ${res.status}` });
          return t;
        }),
        apiFetch('/api/ai/referee-review', {
          method: 'POST',
          headers: aiHeaders,
          body: JSON.stringify(refereePayload),
        }).then(async (res) => {
          const t = await res.text();
          if (!res.ok) return JSON.stringify({ error: t || `Referee review failed (${res.status})`, status: res.status });
          return t;
        }),
        apiFetch('/api/ai/line-review', {
          method: 'POST',
          headers: aiHeaders,
          body: JSON.stringify({ manuscript_text: sourceText }),
        }).then(async (res) => {
          const t = await res.text();
          if (!res.ok) return JSON.stringify({ error: t || `Line review failed (${res.status})`, status: res.status });
          return t;
        }),
        apiFetch('/api/ai/citation-reference-check', {
          method: 'POST',
          headers: aiHeaders,
          body: JSON.stringify({
            manuscript_text: sourceText,
            journal_url: journalLink,
            journal_guidelines: guidelinesJson,
          }),
        }).then(async (res) => {
          const t = await res.text();
          if (!res.ok) {
            const msg = res.status === 502 || res.status === 503
              ? `Citation check service temporarily unavailable (${res.status}). Ensure the orchestrator is deployed and ORCHESTRATOR_URL is set on the backend.`
              : (t && t.length < 300 ? t : `Citation check failed (${res.status}). Try again.`);
            return JSON.stringify({ error: msg });
          }
          return t;
        }),
      ]);
      let pubJson: any;
      try {
        pubJson = (pubText && pubText.trim() && pubText.trim().startsWith('{')) ? JSON.parse(pubText) : { result: {}, error: 'Service unavailable' };
      } catch {
        pubJson = { result: {}, error: 'Publication chance service returned invalid data. Try again.' };
      }
      setRefereeReview(refereeText);
      setLineReview(lineText);
      let citationParsed: any = null;
      try {
        if (typeof citationText === 'string' && citationText.trim().startsWith('{')) {
          citationParsed = JSON.parse(citationText);
          if (citationParsed && citationParsed.error && (citationParsed.error.length > 400 || citationParsed.error.includes('<'))) {
            citationParsed.error = 'Citation check service temporarily unavailable. Please try again.';
          }
        } else {
          citationParsed = { error: citationText && citationText.length < 200 ? citationText : 'Citation check service temporarily unavailable. Please try again.' };
        }
        setCitationCheck(citationParsed);
      } catch {
        citationParsed = { error: 'Citation check service temporarily unavailable. Please try again.' };
        setCitationCheck(citationParsed);
      }

      let refereeParsed: any = refereeText;
      try {
        refereeParsed = JSON.parse(refereeText);
      } catch (err) {
        refereeParsed = { error: 'Invalid referee review JSON', raw: refereeText };
      }

      const report = {
        status: 'ok',
        report_id: `report-${Date.now()}`,
        journal_url: journalLink,
        manuscript_link: manuscriptLink || null,
        chunks: chunkResults.map((cr: any, i: number) => ({
          ...cr,
          chunk_text: chunks[i]?.text || cr.chunk_text,
          chunk_id: chunks[i]?.id || cr.chunk_id,
        })),
        publication_chance: pubJson?.result || pubJson,
        referee_review: refereeParsed,
        line_review: lineText,
        citation_reference_check: citationParsed,
      };

      const reportString = JSON.stringify(report, null, 2);
      setResultPayload(reportString);
      setReportContext(reportString);
      setReportData(report);

      setShowGetSetGo(true);
      const sourceName = usePastedText ? 'Pasted manuscript' : (files[0]?.file?.name || 'Manuscript');
      createReport(token, sourceName, 'publishready');
      setTimeout(() => {
        setShowGetSetGo(false);
        setStatus('complete');
        setStatusMessage('Report ready. You can review and chat with Gaply below.');
      }, 3000);
      setChatMessages([
        {
          id: `assistant-${Date.now()}`,
          role: 'assistant',
          text: 'Your report is ready. Ask me about revisions, structure, or journal fit.',
          timestamp: new Date().toLocaleTimeString(),
        },
      ]);
    } catch (err: any) {
      setStatus('error');
      const raw = String(err?.message || 'Processing failed.');
      const isNetwork =
        raw.includes('ERR_CONNECTION_RESET') ||
        raw.includes('ERR_NAME_NOT_RESOLVED') ||
        raw.includes('ERR_NETWORK_IO_SUSPENDED') ||
        raw.includes('ERR_NETWORK_CHANGED') ||
        raw.includes('Failed to fetch') ||
        raw.includes('JSON') ||
        raw.includes('aborted');
      const msg = isNetwork
        ? 'Analysis service unreachable or timed out. Check your connection and that the backend is deployed; try again in a moment.'
        : raw;
      setStatusMessage(msg);
    }
  };

  const sendChat = async () => {
    const trimmed = chatInput.trim();
    if (!trimmed) return;

    const newMessage: ChatMessage = {
      id: `user-${Date.now()}`,
      role: 'user',
      text: trimmed,
      timestamp: new Date().toLocaleTimeString(),
    };
    setChatMessages((prev) => [...prev, newMessage]);
    setChatInput('');

    try {
      const chatHeaders: Record<string, string> = analysisJobId ? { 'X-Gaply-Job-Id': analysisJobId } : {};
      const res = await apiFetch('/api/ai/chat', {
        method: 'POST',
        headers: Object.keys(chatHeaders).length ? chatHeaders : undefined,
        body: JSON.stringify({
          messages: [{ role: 'user', content: trimmed }],
          report_context: reportContext,
          journal_url: journalLink,
          manuscript_text: manuscriptText ? manuscriptText.slice(0, 8000) : '',
          max_tokens: 2000,
          temperature: 0.7,
          reasoning_effort: 'high',
        }),
      });
      const data = await res.json().catch(() => ({}));
      let reply = data?.response || data?.message;
      if (!res.ok) {
        reply = res.status === 401
          ? 'Session expired. Please log out and log in again to continue chatting.'
          : (data?.error || data?.message || `Request failed (${res.status}). Please try again.`);
      }
      if (!reply) reply = 'No response received. Please try again.';
      setChatMessages((prev) => [
        ...prev,
        {
          id: `assistant-${Date.now()}`,
          role: 'assistant',
          text: reply,
          timestamp: new Date().toLocaleTimeString(),
        },
      ]);
    } catch (err) {
      setChatMessages((prev) => [
        ...prev,
        {
          id: `assistant-${Date.now()}`,
          role: 'assistant',
          text: 'Chat failed to reach the server. Please try again.',
          timestamp: new Date().toLocaleTimeString(),
        },
      ]);
    }
  };

  const toggleRecording = useCallback(() => {
    const win = typeof window !== 'undefined' ? window : null;
    const SpeechRecognition = win ? (win as any).SpeechRecognition || (win as any).webkitSpeechRecognition : null;
    if (!SpeechRecognition) {
      alert('Voice input is not supported in this browser. Try Chrome or Edge.');
      return;
    }
    if (isRecording) {
      try {
        if (recognitionRef.current) recognitionRef.current.stop();
      } catch (_) {}
      recognitionRef.current = null;
      setIsRecording(false);
      return;
    }
    const rec = new SpeechRecognition();
    rec.continuous = true;
    rec.interimResults = true;
    rec.lang = 'en-IN';
    let final = '';
    rec.onresult = (e: any) => {
      for (let i = e.resultIndex; i < e.results.length; i++) {
        const r = e.results[i];
        if (r.isFinal) final += r[0].transcript;
      }
      if (final) setChatInput((prev) => (prev ? prev + ' ' + final : final));
    };
    rec.onerror = () => setIsRecording(false);
    rec.onend = () => { recognitionRef.current = null; setIsRecording(false); };
    recognitionRef.current = rec;
    rec.start();
    setIsRecording(true);
  }, [isRecording]);

  const speakMessage = useCallback((text: string, id: string) => {
    const plain = text.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
    if (!plain) return;
    if (chatTTSControllerRef.current) {
      chatTTSControllerRef.current.stop();
      chatTTSControllerRef.current = null;
    }
    setSpeakingId(id);
    playHumanTTS(plain, {
      voice: 'shimmer',
      apiFetch,
      onEnd: () => {
        chatTTSControllerRef.current = null;
        setSpeakingId(null);
      },
      onError: () => setSpeakingId(null),
    }).then((ctrl) => {
      chatTTSControllerRef.current = ctrl;
    });
  }, []);

  const stopSpeaking = useCallback(() => {
    if (chatTTSControllerRef.current) {
      chatTTSControllerRef.current.stop();
      chatTTSControllerRef.current = null;
    }
    setSpeakingId(null);
  }, []);

  return (
    <div className="manuscript-page">
      <SEO
        title="Document Analysis Orchestrator | Gaply"
        description="Upload your manuscript, submit journal links, and receive a full analysis report with chat-based guidance."
        keywords="document analysis, journal submission, publication chance, academic AI assistant"
      />

      {status === 'error' && (
        <section className="manuscript-hero" style={{ background: 'rgba(215,0,21,0.08)', border: '1px solid #d70015', borderRadius: 12, padding: 16, margin: '16px 0' }}>
          <p style={{ margin: 0, color: '#d70015', fontWeight: 600 }}>{statusMessage}</p>
          <p style={{ margin: '8px 0 0 0', fontSize: 14, color: '#666' }}>You can try again, use pasted text instead, or contact support if the problem continues.</p>
        </section>
      )}

      {status === 'idle' && (
        <section className="manuscript-hero">
          <div className="hero-copy">
            <h1>Document Analysis Orchestrator</h1>
            <p>
              Clean, fast, and trustworthy analysis. Upload your manuscript, add your journal link, and
              get a full report with a chat concierge for next steps.
            </p>
          </div>
        </section>
      )}

      {status === 'processing' && !showGetSetGo && (
        <section className="processing-section">
          <div className="processing-hero">
            <h1>Analyzing Your Manuscript</h1>
            <p className="processing-subtitle">{statusMessage}</p>
            <div className="progress-container">
              <div className="progress-bar-processing">
                <div 
                  className="progress-fill-dynamic" 
                  style={{ 
                    width: `${((currentChunkIndex + 1) / processingChunks.length) * 100}%`,
                    backgroundColor: processingChunks.every(c => c.status === 'complete') ? '#34c759' : '#d70015'
                  }}
                />
              </div>
              <p className="progress-text">
                {currentChunkIndex + 1} of {processingChunks.length} sections
              </p>
            </div>
            <div className="chunks-list-simple">
              {processingChunks.map((chunk, idx) => (
                <div key={chunk.id} className={`chunk-item-simple ${chunk.status}`}>
                  {chunk.status === 'complete' && <span className="check-green">✓</span>}
                  {chunk.status === 'processing' && <div className="spinner-green" />}
                  {chunk.status === 'pending' && <span className="dot-pending">•</span>}
                  <span className="chunk-name-simple">{chunk.name}</span>
                </div>
              ))}
            </div>
          </div>
        </section>
      )}

      {showGetSetGo && (
        <section className="get-set-go-section">
          <div className="get-set-go-animation">
            <h1>Get Set Go</h1>
          </div>
        </section>
      )}

      {status === 'complete' && !showGetSetGo && (
        <section className="complete-hero-premium">
          <h1>Report Ready</h1>
          <p>You can review and chat with Gaply below.</p>
        </section>
      )}

      {(status === 'idle' || status === 'error') && (
        <section className="manuscript-grid">
          <div className="panel upload-panel">
            <h2>Upload Manuscript</h2>
          <div className="dropzone">
            <input
              type="file"
              accept={ACCEPTED_TYPES.join(',')}
              multiple
              onChange={(e) => handleFiles(e.target.files)}
            />
            <div>
              <strong>Drag & drop</strong> your PDF, DOCX, or TXT files
              <p>Supported: {ACCEPTED_TYPES.join(', ')}</p>
            </div>
          </div>
          <div className="file-list">
            {files.map((item) => (
              <div key={item.id} className="file-chip">
                <span>{item.file.name}</span>
                <button onClick={() => removeFile(item.id)}>Remove</button>
              </div>
            ))}
          </div>
          <label className="field-label">Or paste your text</label>
          <textarea
            value={pastedText}
            onChange={(e) => setPastedText(e.target.value)}
            placeholder="Paste manuscript text here..."
            style={{ color: '#1d1d1f' }}
          />
          <div className="toggle-row">
            <input
              type="checkbox"
              checked={usePastedText}
              onChange={(e) => setUsePastedText(e.target.checked)}
            />
            <span>Use pasted text instead of uploaded files</span>
          </div>
        </div>

          <div className="panel input-panel">
            <div className="input-panel-header">
              <h2>Journal Inputs</h2>
              <p className="input-panel-description">Provide your journal details to get targeted analysis</p>
            </div>
            <div className="input-fields-wrapper">
              <label className="field-label">Journal Website Link</label>
              <input
                value={journalLink}
                onChange={(e) => setJournalLink(e.target.value)}
                placeholder="https://journal-website.com"
                style={{ color: '#1d1d1f' }}
              />
              <label className="field-label">Manuscript Link (optional)</label>
              <input
                value={manuscriptLink}
                onChange={(e) => setManuscriptLink(e.target.value)}
                placeholder="https://drive.google.com/..."
                style={{ color: '#1d1d1f' }}
              />
            </div>
          <div className="action-row">
            <button className="primary" onClick={handleSubmit} disabled={false}>
              Generate Report
            </button>
            <button className="secondary" onClick={resetAll}>Clear</button>
          </div>
          </div>
        </section>
      )}

      {status === 'complete' && !showGetSetGo && (
        <>
          <section className="results-options-section">
            <div className="results-options-grid">
              <button className="result-option-card" onClick={() => setActiveModal('download')}>
                <div className="option-icon">⬇</div>
                <h3>Download the Report</h3>
                <p>Document Analysis Report — Manuscript Evaluation</p>
              </button>
              <button className="result-option-card" onClick={() => setActiveModal('chat')}>
                <div className="option-icon">💭</div>
                <h3>Chat with Gaply</h3>
                <p>Talk or type — get detailed, context-aware corrections</p>
              </button>
              <button className="result-option-card" onClick={() => setActiveModal('audio-story')}>
                <div className="option-icon">🎧</div>
                <h3>Listen to Analysis</h3>
                <p>What’s lacking, how to improve, and publication chances</p>
              </button>
              <button className="result-option-card" onClick={() => setActiveModal('citation-check')}>
                <div className="option-icon">📚</div>
                <h3>References & Citation Check</h3>
                <p>Style, authenticity, and journal compliance</p>
              </button>
            </div>
          </section>

          {/* Download Modal */}
          {activeModal === 'download' && (
            <div className="modal-overlay" onClick={() => setActiveModal(null)}>
              <div className="modal-content" onClick={(e) => e.stopPropagation()}>
                <div className="modal-header">
                  <h2>Download Reports</h2>
                  <button className="modal-close" onClick={() => setActiveModal(null)}>×</button>
                </div>
                <div className="modal-body-download">
                  <div className="download-actions-grid">
                    {reportData && (
                      <>
                        <button
                          className="download-action-btn primary"
                          onClick={() => {
                            try {
                              downloadHTMLReport(reportData);
                            } catch (err) {
                              console.error('Failed to generate summary report:', err);
                              alert('Failed to generate summary report. Please try again.');
                            }
                          }}
                        >
                          <span className="download-icon">📄</span>
                          <span className="download-text">Download Summary Report (HTML)</span>
                        </button>
                        <button
                          className="download-action-btn"
                          onClick={() => {
                            try {
                              downloadReportAsPDF(reportData);
                            } catch (err) {
                              console.error('Failed to open PDF print:', err);
                              alert('Failed to open print dialog. Please try the HTML download and print from your browser.');
                            }
                          }}
                        >
                          <span className="download-icon">📕</span>
                          <span className="download-text">Download as PDF</span>
                        </button>
                      </>
                    )}
                    {/* Advanced: interactive line-by-line report (hidden by default per user preference)
                    {reportData && manuscriptText && (
                      <button
                        className="download-action-btn"
                        onClick={() => {
                          try {
                            downloadTurnitinStyleReport(reportData, manuscriptText);
                          } catch (err) {
                            console.error('Failed to generate interactive report:', err);
                            alert('Failed to generate interactive report. Please try again.');
                          }
                        }}
                      >
                        <span className="download-icon">🖊️</span>
                        <span className="download-text">Download Interactive Line-by-Line Report</span>
                      </button>
                    )} */}
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Citation Check Modal */}
          {activeModal === 'citation-check' && citationCheck && (
            <div className="modal-overlay" onClick={() => setActiveModal(null)}>
              <div className="modal-content referee-modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 720 }}>
                <div className="modal-header">
                  <div>
                    <h2>References & Citation Check</h2>
                    <p className="modal-subtitle">Style, authenticity, and journal compliance</p>
                  </div>
                  <button className="modal-close" onClick={() => setActiveModal(null)}>×</button>
                </div>
                <div className="modal-body-referee" style={{ padding: 20, maxHeight: '80vh', overflowY: 'auto' }}>
                  {citationCheck.error ? (
                    <p style={{ color: '#d70015' }}>{citationCheck.error}</p>
                  ) : (
                    <>
                      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 12, marginBottom: 16 }}>
                        <span><strong>Style detected:</strong> {citationCheck.reference_style_detected || '—'} ({citationCheck.style_confidence || '—'})</span>
                        {citationCheck.journal_required_style && <span><strong>Journal requires:</strong> {citationCheck.journal_required_style}</span>}
                        <span><strong>Journal compliance:</strong> <span style={{ color: citationCheck.journal_compliance === 'compliant' ? '#34c759' : citationCheck.journal_compliance === 'non_compliant' ? '#d70015' : '#6e6e73' }}>{citationCheck.journal_compliance || 'unknown'}</span></span>
                      </div>
                      {citationCheck.summary && <p style={{ marginBottom: 16, lineHeight: 1.6 }}>{citationCheck.summary}</p>}
                      {Array.isArray(citationCheck.journal_violations) && citationCheck.journal_violations.length > 0 && (
                        <div style={{ marginBottom: 16 }}>
                          <h4 style={{ marginBottom: 8 }}>Journal violations</h4>
                          <ul style={{ paddingLeft: 20, margin: 0 }}>
                            {citationCheck.journal_violations.map((v: string, i: number) => <li key={i}>{v}</li>)}
                          </ul>
                        </div>
                      )}
                      {Array.isArray(citationCheck.references) && citationCheck.references.length > 0 && (
                        <div style={{ marginBottom: 16 }}>
                          <h4 style={{ marginBottom: 8 }}>References ({citationCheck.references.length})</h4>
                          <div style={{ fontSize: 13, overflowX: 'auto' }}>
                            {citationCheck.references.slice(0, 30).map((ref: any, i: number) => (
                              <div key={i} style={{ marginBottom: 10, padding: 10, background: ref.status !== 'correct' ? 'rgba(255,59,48,0.08)' : '#fafafa', borderRadius: 8, borderLeft: `3px solid ${ref.status === 'correct' ? '#34c759' : ref.status === 'suspicious' ? '#ff9500' : '#d70015'}` }}>
                                <div><strong>[{ref.ref_id}]</strong> {ref.raw_string || `${ref.authors || ''} (${ref.year || ''}). ${ref.title || ''}`}</div>
                                <div style={{ color: '#6e6e73', marginTop: 4 }}>Status: {ref.status || '—'} {Array.isArray(ref.issues) && ref.issues.length > 0 && `— ${ref.issues.join('; ')}`}</div>
                              </div>
                            ))}
                            {citationCheck.references.length > 30 && <p style={{ color: '#6e6e73' }}>… and {citationCheck.references.length - 30} more</p>}
                          </div>
                        </div>
                      )}
                      {Array.isArray(citationCheck.in_text_citations) && citationCheck.in_text_citations.length > 0 && (
                        <div style={{ marginBottom: 16 }}>
                          <h4 style={{ marginBottom: 8 }}>In-text citations ({citationCheck.in_text_citations.length})</h4>
                          <div style={{ fontSize: 12 }}>
                            {citationCheck.in_text_citations.slice(0, 15).map((c: any, i: number) => (
                              <div key={i} style={{ marginBottom: 6 }}><span style={{ color: c.status !== 'correct' ? '#d70015' : undefined }}>[{c.cite_id}]</span> {c.pattern} → refs {Array.isArray(c.linked_ref_ids) ? c.linked_ref_ids.join(', ') : '—'} {c.status !== 'correct' && `(${c.status})`}</div>
                            ))}
                            {citationCheck.in_text_citations.length > 15 && <p style={{ color: '#6e6e73' }}>… and {citationCheck.in_text_citations.length - 15} more</p>}
                          </div>
                        </div>
                      )}
                      {Array.isArray(citationCheck.orphan_references) && citationCheck.orphan_references.length > 0 && (
                        <p style={{ color: '#ff9500' }}><strong>Orphan references (not cited in text):</strong> {citationCheck.orphan_references.join(', ')}</p>
                      )}
                    </>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* Chat Modal */}
          {activeModal === 'chat' && (
            <div className="modal-overlay" onClick={() => setActiveModal(null)}>
              <div className="modal-content chat-modal" onClick={(e) => e.stopPropagation()}>
                <div className="modal-header">
                  <h2>Chat with Gaply</h2>
                  <button className="modal-close" onClick={() => setActiveModal(null)}>×</button>
                </div>
                <div className="modal-body-chat">
                  <div className="chat-window-full">
                    {chatMessages.map((message) => (
                      <div key={message.id} className={`chat-message ${message.role}`}>
                        <div className="bubble">
                          {message.role === 'assistant' ? (
                            <>
                              <MarkdownRenderer content={message.text} />
                              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 6 }}>
                                <button
                                  type="button"
                                  className="chat-audio-btn"
                                  onClick={() => speakingId === message.id ? stopSpeaking() : speakMessage(message.text, message.id)}
                                  title={speakingId === message.id ? 'Stop' : 'Listen'}
                                  style={{ background: 'none', border: 'none', cursor: 'pointer', padding: 4, fontSize: 18 }}
                                >
                                  {speakingId === message.id ? '⏹' : '🔊'}
                                </button>
                                {speakingId === message.id && <span style={{ fontSize: 12, color: '#6e6e73' }}>Playing…</span>}
                              </div>
                            </>
                          ) : (
                            <p>{message.text}</p>
                          )}
                          <span className="chat-timestamp">{message.timestamp}</span>
                        </div>
                      </div>
                    ))}
                  </div>
                  <div className="chat-input">
                    <button
                      type="button"
                      className={`chat-mic-btn ${isRecording ? 'recording' : ''}`}
                      onClick={toggleRecording}
                      title={isRecording ? 'Stop recording' : 'Voice input'}
                      style={{ background: isRecording ? '#d70015' : 'transparent', border: '1px solid #d2d2d7', borderRadius: 8, cursor: 'pointer', padding: '8px 12px', fontSize: 18 }}
                    >
                      🎤
                    </button>
                    <input
                      value={chatInput}
                      onChange={(e) => setChatInput(e.target.value)}
                      onKeyPress={(e) => e.key === 'Enter' && sendChat()}
                      placeholder="Ask Gaply about revisions, methods, or journal fit... (or use mic)"
                    />
                    <button onClick={sendChat}>Send</button>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Audio Story Modal */}
          {activeModal === 'audio-story' && (
            <AudioStoryModal
              reportContext={reportContext}
              manuscriptText={manuscriptText}
              journalLink={journalLink}
              reportData={reportData}
              analysisJobId={analysisJobId}
              onClose={() => setActiveModal(null)}
            />
          )}
        </>
      )}
    </div>
  );
};

export default ManuscriptOrchestratorPage;
