import React, { useMemo, useState } from 'react';
import SEO from './SEO';
import { apiFetch } from '../api/config';
import MarkdownRenderer from './MarkdownRenderer';
import { downloadHTMLReport } from './HTMLReportGenerator';
import { downloadTurnitinStyleReport } from './TurnitinStyleReportGenerator';
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

const ManuscriptOrchestratorPage: React.FC = () => {
  const [files, setFiles] = useState<UploadFile[]>([]);
  const [pastedText, setPastedText] = useState('');
  const [journalLink, setJournalLink] = useState('');
  const [manuscriptLink, setManuscriptLink] = useState('');
  const [usePastedText, setUsePastedText] = useState(false);
  const [status, setStatus] = useState<'idle' | 'processing' | 'complete' | 'error'>('idle');
  const [statusMessage, setStatusMessage] = useState('Ready for upload.');
  const [resultPayload, setResultPayload] = useState<string>('');
  const [reportContext, setReportContext] = useState<string>('');
  const [reportData, setReportData] = useState<any>(null);
  const [refereeReview, setRefereeReview] = useState<string>('');
  const [lineReview, setLineReview] = useState<string>('');
  const [manuscriptText, setManuscriptText] = useState<string>('');
  const [processingChunks, setProcessingChunks] = useState<{id: string; name: string; status: 'pending' | 'processing' | 'complete'}[]>([]);
  const [currentChunkIndex, setCurrentChunkIndex] = useState(0);
  const [refereeExpanded, setRefereeExpanded] = useState(false);
  const [showGetSetGo, setShowGetSetGo] = useState(false);
  const [activeModal, setActiveModal] = useState<'download' | 'chat' | 'referee' | null>(null);
  const refereeData = useMemo(() => {
    if (!refereeReview) return null;
    try {
      return JSON.parse(refereeReview);
    } catch (err) {
      return null;
    }
  }, [refereeReview]);

  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [chatInput, setChatInput] = useState('');

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
    setChatMessages([]);
    setChatInput('');
    setProcessingChunks([]);
    setCurrentChunkIndex(0);
    setRefereeExpanded(false);
  };

  // Generate meaningful chunk names from text
  const generateChunkName = (text: string, index: number): string => {
    const firstLine = text.split('\n')[0].trim();
    if (firstLine.length > 0 && firstLine.length < 60) {
      // Check if it looks like a heading
      if (firstLine === firstLine.toUpperCase() || /^[A-Z][a-z]+/.test(firstLine)) {
        return firstLine.length > 50 ? firstLine.slice(0, 47) + '...' : firstLine;
      }
    }
    // Extract first meaningful sentence
    const sentences = text.split(/[.!?]\s+/).filter(s => s.trim().length > 20);
    if (sentences.length > 0) {
      const name = sentences[0].trim();
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
        const uploadJson = await uploadRes.json();
        if (!uploadRes.ok) {
          throw new Error(uploadJson?.error || 'Upload failed.');
        }
        sourceText = uploadJson.extracted_text || '';
        if (!sourceText) {
          throw new Error('No text extracted from upload. Try pasting text.');
        }
      }

      if (!sourceText) {
        throw new Error('Please upload a file or paste manuscript text.');
      }

      // Store manuscript text for chat context
      setManuscriptText(sourceText);

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
          method: 'POST',
          body: JSON.stringify({ journal_url: journalLink }),
        }),
        apiFetch('/api/ai/retrieved-docs', {
          method: 'POST',
          body: JSON.stringify({
            query: '',
            manuscript_text: sourceText,
            limit: 100,
          }),
        }),
      ]);
      const [guidelinesJson, retrievedJson] = await Promise.all([
        guidelinesRes.json(),
        retrievedRes.json(),
      ]);
      const retrievedDocs = retrievedJson?.retrieved_docs || [];
      const slimRetrievedDocs = retrievedDocs
        .slice(0, 20)
        .map((doc: any) => ({
          ...doc,
          snippet: typeof doc.snippet === 'string' ? doc.snippet.slice(0, 350) : doc.snippet,
        }));
      setStatusMessage(`Analyzing ${chunks.length} sections with deep review...`);
      const chunkResults = await runWithConcurrency(
        chunks,
        2,
        async (chunk, index) => {
          setCurrentChunkIndex(index);
          setProcessingChunks(prev => prev.map((c, i) => 
            i === index ? { ...c, status: 'processing' } : 
            i < index ? { ...c, status: 'complete' } : c
          ));
          setStatusMessage(`Analyzing: ${chunkNames[index].name}...`);
          const payload = {
            job_id: `job-${Date.now()}`,
            chunk_id: chunk.id,
            chunk_text: chunk.text,
            chunk_position: chunk.position,
            chunk_token_estimate: Math.floor(chunk.text.length / 4),
            journal_guidelines: guidelinesJson || { journal_url: journalLink },
            retrieved_docs: slimRetrievedDocs,
            tasks: ['guideline_check', 'novelty_check', 'plagiarism_check', 'ai_use_detection', 'suggest_edits'],
            max_tokens_for_response: 900,
          };

          const res = await apiFetch('/api/ai/document-orchestrator', {
            method: 'POST',
            body: JSON.stringify(payload),
          });
          const text = await res.text();
          const result = JSON.parse(text);
          
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
      const [pubText, refereeText, lineText] = await Promise.all([
        apiFetch('/api/ai/publication-chance', {
          method: 'POST',
          body: JSON.stringify(publicationPayload),
        }).then((res) => res.text()),
        apiFetch('/api/ai/referee-review', {
          method: 'POST',
          body: JSON.stringify(refereePayload),
        }).then((res) => res.text()),
        apiFetch('/api/ai/line-review', {
          method: 'POST',
          body: JSON.stringify({ manuscript_text: sourceText }),
        }).then((res) => res.text()),
      ]);
      const pubJson = JSON.parse(pubText);
      setRefereeReview(refereeText);
      setLineReview(lineText);

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
        chunks: chunkResults,
        publication_chance: pubJson?.result || pubJson,
        referee_review: refereeParsed,
        line_review: lineText,
      };

      const reportString = JSON.stringify(report, null, 2);
      setResultPayload(reportString);
      setReportContext(reportString);
      setReportData(report);

      setShowGetSetGo(true);
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
      setStatusMessage(err?.message || 'Processing failed.');
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
      const res = await apiFetch('/api/ai/chat', {
        method: 'POST',
        body: JSON.stringify({
          messages: [
            { role: 'user', content: trimmed },
          ],
          report_context: reportContext,
          journal_url: journalLink,
          manuscript_text: manuscriptText ? manuscriptText.slice(0, 2000) : '', // First 2000 chars for context
          max_tokens: 800,
          temperature: 0.7,
        }),
      });
      const data = await res.json();
      setChatMessages((prev) => [
        ...prev,
        {
          id: `assistant-${Date.now()}`,
          role: 'assistant',
          text: data?.response || 'No response received.',
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

  return (
    <div className="manuscript-page">
      <SEO
        title="Document Analysis Orchestrator | Gaply"
        description="Upload your manuscript, submit journal links, and receive a full analysis report with chat-based guidance."
        keywords="document analysis, journal submission, publication chance, academic AI assistant"
      />

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

      {status === 'idle' && (
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
                <p>Access all reports and download options</p>
              </button>
              <button className="result-option-card" onClick={() => setActiveModal('chat')}>
                <div className="option-icon">💭</div>
                <h3>Chat with Gaply</h3>
                <p>Get interactive guidance and answers</p>
              </button>
              <button className="result-option-card" onClick={() => setActiveModal('referee')}>
                <div className="option-icon">✓</div>
                <h3>Referee-Style Review</h3>
                <p>Deep evaluation aligned with journal feedback norms</p>
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
                    <button className="download-action-btn" onClick={() => window.open(journalLink || 'https://example.com/journal', '_blank')}>
                      <span className="download-icon">🔗</span>
                      <span className="download-text">Open Journal Link</span>
                    </button>
                    {manuscriptLink && (
                      <button className="download-action-btn" onClick={() => window.open(manuscriptLink, '_blank')}>
                        <span className="download-icon">📄</span>
                        <span className="download-text">Open Manuscript Link</span>
                      </button>
                    )}
                    {reportData && (
                      <>
                        <button 
                          className="download-action-btn primary" 
                          onClick={() => {
                            try {
                              downloadHTMLReport(reportData);
                            } catch (err) {
                              console.error('Failed to generate HTML report:', err);
                              alert('Failed to generate HTML report. Please try again.');
                            }
                          }}
                        >
                          <span className="download-icon">📥</span>
                          <span className="download-text">Download Summary Report</span>
                        </button>
                        {manuscriptText && (
                          <button 
                            className="download-action-btn primary" 
                            onClick={() => {
                              try {
                                downloadTurnitinStyleReport(reportData, manuscriptText);
                              } catch (err) {
                                console.error('Failed to generate Turnitin-style report:', err);
                                alert('Failed to generate Turnitin-style report. Please try again.');
                              }
                            }}
                          >
                            <span className="download-icon">📄</span>
                            <span className="download-text">Download Interactive Analysis Report</span>
                          </button>
                        )}
                      </>
                    )}
                  </div>
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
                            <MarkdownRenderer content={message.text} />
                          ) : (
                            <p>{message.text}</p>
                          )}
                          <span className="chat-timestamp">{message.timestamp}</span>
                        </div>
                      </div>
                    ))}
                  </div>
                  <div className="chat-input">
                    <input
                      value={chatInput}
                      onChange={(e) => setChatInput(e.target.value)}
                      onKeyPress={(e) => e.key === 'Enter' && sendChat()}
                      placeholder="Ask Gaply about revisions, methods, or journal fit..."
                    />
                    <button onClick={sendChat}>Send</button>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Referee Review Modal */}
          {activeModal === 'referee' && (
            <div className="modal-overlay" onClick={() => setActiveModal(null)}>
              <div className="modal-content referee-modal" onClick={(e) => e.stopPropagation()}>
                <div className="modal-header">
                  <div>
                    <h2>Referee-Style Review</h2>
                    <p className="modal-subtitle">Deep evaluation aligned with journal feedback norms</p>
                  </div>
                  <button className="modal-close" onClick={() => setActiveModal(null)}>×</button>
                </div>
                <div className="modal-body-referee">
                  {refereeData && !refereeData.error && (
                    <div className="referee-cards">
                      <div className="referee-card">
                        <h3>Methodology Rubric</h3>
                        <div className="score-grid">
                          {refereeData.methodology_rubric && (
                            <>
                              <span>Design</span>
                              <strong>{refereeData.methodology_rubric.design_clarity ?? '-'}</strong>
                              <span>Data Quality</span>
                              <strong>{refereeData.methodology_rubric.data_quality ?? '-'}</strong>
                              <span>Analysis Rigor</span>
                              <strong>{refereeData.methodology_rubric.analysis_rigor ?? '-'}</strong>
                              <span>Validity</span>
                              <strong>{refereeData.methodology_rubric.validity_threats ?? '-'}</strong>
                              <span>Reproducibility</span>
                              <strong>{refereeData.methodology_rubric.reproducibility ?? '-'}</strong>
                            </>
                          )}
                        </div>
                        {refereeData.methodology_rubric?.notes && (
                          <p className="muted">{refereeData.methodology_rubric.notes}</p>
                        )}
                      </div>
                      <div className="referee-card">
                        <h3>Citation Freshness</h3>
                        <div className="metric-row">
                          <span>Recent (≥ {refereeData.citation_freshness?.cutoff_year ?? 2020})</span>
                          <strong>{refereeData.citation_freshness?.recent_citations_pct ?? 0}%</strong>
                        </div>
                        <div className="mini-list">
                          {(refereeData.citation_freshness?.older_key_citations || []).slice(0, 5).map((item: any, idx: number) => (
                            <div key={`old-cite-${idx}`} className="mini-item">
                              <span>{item.citation || 'Untitled citation'}</span>
                              <em>{item.year || 'n/a'}</em>
                            </div>
                          ))}
                          {!refereeData.citation_freshness?.older_key_citations?.length && (
                            <div className="mini-item muted">No older key citations flagged.</div>
                          )}
                        </div>
                      </div>
                      <div className="referee-card">
                        <h3>Cross-Section Consistency</h3>
                        <div className="mini-list">
                          {(refereeData.cross_section_consistency || []).slice(0, 6).map((item: any, idx: number) => (
                            <div key={`cons-${idx}`} className="mini-item">
                              <strong className={`badge ${item.severity || 'minor'}`}>{item.severity || 'minor'}</strong>
                              <span>{item.sections || 'Sections'}</span>
                              <em>{item.issue || 'No issue provided'}</em>
                            </div>
                          ))}
                          {!refereeData.cross_section_consistency?.length && (
                            <div className="mini-item muted">No cross-section conflicts detected.</div>
                          )}
                        </div>
                      </div>
                      <div className="referee-card">
                        <h3>Evidence Table</h3>
                        <div className="mini-list">
                          {(refereeData.evidence_table || []).slice(0, 6).map((item: any, idx: number) => (
                            <div key={`evidence-${idx}`} className="mini-item">
                              <span>{item.claim || 'Claim not specified'}</span>
                              <em>{item.evidence_type || 'evidence'}</em>
                              <strong className={`badge ${item.strength || 'moderate'}`}>{item.strength || 'moderate'}</strong>
                            </div>
                          ))}
                          {!refereeData.evidence_table?.length && (
                            <div className="mini-item muted">No evidence mapping provided.</div>
                          )}
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
};

export default ManuscriptOrchestratorPage;
