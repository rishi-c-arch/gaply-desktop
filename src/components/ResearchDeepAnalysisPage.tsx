import React, { useState, useCallback, useEffect } from 'react';
import SEO from './SEO';
import { apiFetch } from '../api/config';
import { useAuth } from '../contexts/AuthContext';
import './ResearchDeepAnalysisPage.css';

const ACCEPTED_TYPES = ['.pdf', '.docx', '.txt'];

function sanitizeApiError(msg: string): string {
  const lower = msg.toLowerCase();
  if (lower.includes('quota') || lower.includes('openai') || lower.includes('insufficient') || lower.includes('exceeded')) {
    return 'Analysis temporarily unavailable. Please try again later.';
  }
  return msg;
}

const ResearchDeepAnalysisPage: React.FC = () => {
  const { canUseFeature } = useAuth();
  const [step, setStep] = useState<'upload' | 'analyzing' | 'results'>('upload');
  const [papers, setPapers] = useState<[File | null, File | null, File | null]>([null, null, null]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [progressMsg, setProgressMsg] = useState('');
  const [result, setResult] = useState<any>(null);
  const [sessionId, setSessionId] = useState('');
  const [audioTab, setAudioTab] = useState<'hindi' | 'english' | 'combined'>('english');
  const [chatExpanded, setChatExpanded] = useState(false);
  const [chatMessages, setChatMessages] = useState<{ role: 'user' | 'assistant'; content: string }[]>([]);
  const [chatInput, setChatInput] = useState('');
  const [chatLoading, setChatLoading] = useState(false);
  const [premiumBlocked, setPremiumBlocked] = useState(false);
  const [remainingUses, setRemainingUses] = useState(0);

  useEffect(() => {
    const check = async () => {
      try {
        const { canUse, remainingUses: r } = await canUseFeature('deep_eval');
        setRemainingUses(r);
        setPremiumBlocked(!canUse);
      } catch {
        setPremiumBlocked(true);
      }
    };
    check();
  }, [canUseFeature]);

  const handlePaper = useCallback((index: 0 | 1 | 2, file: File | null) => {
    setPapers(prev => {
      const next = [...prev] as [File | null, File | null, File | null];
      next[index] = file;
      return next;
    });
  }, []);

  const onDrop = useCallback((index: 0 | 1 | 2) => (e: React.DragEvent) => {
    e.preventDefault();
    const f = e.dataTransfer.files[0];
    if (f) {
      const ext = '.' + (f.name.split('.').pop() || '').toLowerCase();
      if (ACCEPTED_TYPES.includes(ext)) handlePaper(index, f);
    }
  }, [handlePaper]);

  const onFileSelect = useCallback((index: 0 | 1 | 2) => (e: React.ChangeEvent<HTMLInputElement>) => {
    const f = e.target.files?.[0];
    if (f) {
      const ext = '.' + (f.name.split('.').pop() || '').toLowerCase();
      if (ACCEPTED_TYPES.includes(ext)) handlePaper(index, f);
    }
    e.target.value = '';
  }, [handlePaper]);

  const canAnalyze = papers.some(p => p !== null);

  const handleAnalyze = async () => {
    if (!canAnalyze || loading || premiumBlocked) return;

    setLoading(true);
    setError(null);
    setStep('analyzing');
    setProgress(0);
    setProgressMsg('Extracting text from papers...');

    const msgs = ['Extracting text...', 'Analyzing papers...', 'Identifying research gaps...', 'Synthesizing opportunities...', 'Generating report...'];
    let mi = 0;
    const pInt = setInterval(() => setProgress(p => Math.min(p + Math.random() * 5, 92)), 500);
    const mInt = setInterval(() => { setProgressMsg(msgs[mi % msgs.length]); mi++; }, 2500);

    try {
      const form = new FormData();
      papers.forEach((p, i) => {
        if (p) form.append(`paper${i + 1}`, p);
      });

      const res = await apiFetch('/api/research-deep-analysis/analyze', {
        method: 'POST',
        body: form,
      });

      if (!res.ok) {
        const e = await res.json().catch(() => ({}));
        throw new Error((e as { error?: string }).error || 'Analysis failed');
      }

      const data = await res.json();
      setResult(data);
      setSessionId(data.session_id || '');
      setProgress(100);
      setProgressMsg('Analysis complete!');
      setTimeout(() => setStep('results'), 400);
    } catch (err: unknown) {
      setError(sanitizeApiError(err instanceof Error ? err.message : 'Analysis failed'));
      setStep('upload');
    } finally {
      clearInterval(pInt);
      clearInterval(mInt);
      setLoading(false);
    }
  };

  const sendChat = async () => {
    if (!chatInput.trim() || chatLoading || !sessionId) return;
    const msg = chatInput.trim();
    setChatInput('');
    setChatMessages(prev => [...prev, { role: 'user', content: msg }]);
    setChatLoading(true);
    try {
      const res = await apiFetch('/api/research-deep-analysis/chat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ session_id: sessionId, message: msg }),
      });
      const data = await res.json();
      setChatMessages(prev => [...prev, { role: 'assistant', content: (data as { reply?: string }).reply || 'I could not generate a response.' }]);
    } catch {
      setChatMessages(prev => [...prev, { role: 'assistant', content: 'Sorry, there was an error. Please try again.' }]);
    } finally {
      setChatLoading(false);
    }
  };

  const buildFallbackReportHtml = () => {
    const r = result as Record<string, unknown>;
    const parts: string[] = [];
    const h2 = (t: string) => parts.push(`<h2>${t}</h2>`);
    const h3 = (t: string) => parts.push(`<h3>${t}</h3>`);
    const p = (t: string) => parts.push(`<p>${String(t || '').replace(/</g, '&lt;').replace(/\n/g, '<br/>')}</p>`);
    if (r.summary_paper_1) { h2('Paper 1 Summary'); p(r.summary_paper_1 as string); }
    if (r.summary_paper_2) { h2('Paper 2 Summary'); p(r.summary_paper_2 as string); }
    if (r.summary_paper_3) { h2('Paper 3 Summary'); p(r.summary_paper_3 as string); }
    if (r.summary_combined) { h2('Combined Summary'); p(r.summary_combined as string); }
    if (r.comparative_analysis) { h2('Comparative Analysis'); p(r.comparative_analysis as string); }
    const gaps = (r.gap_inventory || r.research_gaps) as Array<{ gap?: string; category?: string; severity?: string }> | undefined;
    if (gaps?.length) {
      h2('Gap Inventory');
      parts.push('<ul>');
      gaps.forEach((g: { gap?: string; category?: string; severity?: string }) => {
        parts.push(`<li><strong>${g.gap || ''}</strong>${g.category ? ` [${g.category}]` : ''}${g.severity ? ` (${g.severity})` : ''}</li>`);
      });
      parts.push('</ul>');
    }
    const ro = r.research_opportunities as Array<{ gap_statement?: string; research_objective?: string }> | undefined;
    if (ro?.length) {
      h2('Research Opportunities');
      ro.forEach((o: { gap_statement?: string; research_objective?: string }) => {
        h3('Opportunity');
        if (o.gap_statement) p(o.gap_statement);
        if (o.research_objective) p(o.research_objective);
      });
    }
    const titles = r.suggested_titles as string[] | undefined;
    if (titles?.length) { h2('Suggested Titles'); parts.push('<ol>'); titles.forEach((t: string) => parts.push(`<li>${t}</li>`)); parts.push('</ol>'); }
    const journals = r.target_journals as Array<{ journal?: string; quartile?: string; reason?: string }> | undefined;
    if (journals?.length) {
      h2('Target Journals');
      parts.push('<ul>');
      journals.forEach((j: { journal?: string; quartile?: string; reason?: string }) => {
        parts.push(`<li><strong>${j.journal || ''}</strong>${j.quartile ? ` (${j.quartile})` : ''}${j.reason ? ` — ${j.reason}` : ''}</li>`);
      });
      parts.push('</ul>');
    }
    return parts.join('');
  };

  const downloadReport = () => {
    const r = result as { full_report_html?: string };
    const content = r?.full_report_html || buildFallbackReportHtml();
    if (!content) return;
    const html = `<!DOCTYPE html><html><head><meta charset="utf-8"><title>Research Deep Analysis Report</title>
<style>body{font-family:Georgia,serif;max-width:800px;margin:40px auto;padding:24px;color:#1a1a2e;line-height:1.7}
h1{font-size:28px;border-bottom:3px solid #3b82f6;padding-bottom:12px}h2{font-size:22px;margin-top:32px}h3{font-size:18px;margin-top:20px}
table{width:100%;border-collapse:collapse;margin:16px 0}th{background:#f0f4ff;padding:12px;text-align:left}
td{padding:10px 12px;border-bottom:1px solid #e5e7eb}ul,ol{margin:12px 0;padding-left:24px}
.footer{margin-top:40px;padding-top:16px;border-top:1px solid #e5e7eb;color:#6b7280;font-size:13px;text-align:center}</style></head><body>
${content}
<div class="footer">Generated by Gaply Research Deep Analysis — ${new Date().toLocaleString()}</div></body></html>`;
    const blob = new Blob([html], { type: 'text/html' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `research_deep_analysis_${new Date().toISOString().slice(0, 10)}.html`;
    a.click();
    URL.revokeObjectURL(a.href);
  };

  const getAudioText = () => {
    const t = result?.[`summary_${audioTab}`] || result?.summary_combined || result?.summary_english || '';
    return typeof t === 'string' ? t : '';
  };

  const speak = (text?: string, lang?: 'hi-IN' | 'en-US') => {
    const toSpeak = text || getAudioText();
    const langCode = lang ?? (audioTab === 'hindi' ? 'hi-IN' : 'en-US');
    if (!('speechSynthesis' in window) || !toSpeak) return;
    window.speechSynthesis.cancel();
    const u = new SpeechSynthesisUtterance(toSpeak.slice(0, 5000));
    u.lang = langCode;
    u.rate = 0.9;
    window.speechSynthesis.speak(u);
  };

  const reset = () => {
    setStep('upload');
    setPapers([null, null, null]);
    setResult(null);
    setSessionId('');
    setChatMessages([]);
    setChatInput('');
    setError(null);
  };

  return (
    <div className="rda-page">
      <SEO title="Research Deep Analysis | Gaply" description="Deep analysis of research papers with gap identification and publication suggestions" keywords="research analysis, literature gap, research papers, publication" />

      {error && (
        <div className="rda-error">
          ⚠️ {error}
          <button onClick={() => setError(null)} className="rda-error-close">×</button>
        </div>
      )}

      {step === 'upload' && (
        <div className="rda-hero">
          <div className="rda-hero-bg" />
          <div className="rda-hero-content">
            <span className="rda-badge">Pro</span>
            <h1>Research Deep Analysis</h1>
            <p>Upload 3 research papers (PDF, DOCX, or TXT) for a comprehensive analysis using the Researcher&apos;s Deep Analysis Framework. Identify gaps, methodologies, and publication opportunities.</p>
            {premiumBlocked && (
              <div className="rda-premium-block">
                <p>This feature requires a premium plan with Deep Analysis credits. You have {remainingUses} use(s) remaining.</p>
                <a href="/packages" className="rda-btn rda-btn-primary">Upgrade</a>
              </div>
            )}
            {!premiumBlocked && (
              <div className="rda-upload-zone">
                {([0, 1, 2] as const).map(i => (
                  <div
                    key={i}
                    className={`rda-paper-slot ${papers[i] ? 'has-file' : ''}`}
                    onDragOver={e => e.preventDefault()}
                    onDrop={onDrop(i)}
                  >
                    <input
                      type="file"
                      accept=".pdf,.docx,.txt,application/pdf,application/vnd.openxmlformats-officedocument.wordprocessingml.document,text/plain"
                      onChange={onFileSelect(i)}
                      id={`paper${i + 1}`}
                      style={{ display: 'none' }}
                    />
                    <label htmlFor={`paper${i + 1}`} className="rda-slot-label">
                      {papers[i] ? (
                        <span className="rda-file-name">📄 {papers[i]!.name}</span>
                      ) : (
                        <span>Paper {i + 1} — Drop or click</span>
                      )}
                    </label>
                    {papers[i] && (
                      <button type="button" className="rda-remove" onClick={() => handlePaper(i, null)}>✕</button>
                    )}
                  </div>
                ))}
                <button
                  className="rda-btn rda-btn-primary"
                  onClick={handleAnalyze}
                  disabled={!canAnalyze || loading}
                >
                  {loading ? 'Analyzing...' : 'Analyze Papers'}
                </button>
              </div>
            )}
          </div>
        </div>
      )}

      {step === 'analyzing' && (
        <div className="rda-loading">
          <div className="rda-loading-content">
            <div className="rda-progress-bar">
              <div className="rda-progress-fill" style={{ width: `${progress}%` }} />
            </div>
            <p className="rda-progress-msg">{progressMsg}</p>
          </div>
        </div>
      )}

      {step === 'results' && result && (
        <div className="rda-report-ready">
          <div className="rda-report-ready-header">
            <h2>Report Ready</h2>
            <p className="rda-report-ready-sub">You can review and chat with Gaply below.</p>
            <button className="rda-btn rda-btn-ghost" onClick={reset}>New Analysis</button>
          </div>

          <div className="rda-cards-grid">
            <div className="rda-card" onClick={downloadReport}>
              <div className="rda-card-icon">📥</div>
              <h3>Download the Report</h3>
              <p>Document Analysis Report — Paper summaries, comparative analysis, gap inventory, research opportunities, suggested titles, target journals.</p>
            </div>

            <div className={`rda-card rda-card-chat ${chatExpanded ? 'expanded' : ''}`} onClick={() => setChatExpanded(true)}>
              <div className="rda-card-icon">💬</div>
              <h3>Chat with Gaply</h3>
              <p>Talk or type — get detailed, context-aware answers about your papers, gaps, and publication strategy.</p>
              {chatExpanded && (
                <div className="rda-chat-inline" onClick={e => e.stopPropagation()}>
                  <div className="rda-chat-messages">
                    {chatMessages.length === 0 && (
                      <div className="rda-chat-empty">
                        <p>Ask about the papers, research gaps, comparative analysis, or publication suggestions.</p>
                        <p className="rda-chat-hint">Try: &quot;What are the main research gaps?&quot; or &quot;Compare the methodologies of Paper 1 and 2&quot;</p>
                      </div>
                    )}
                    {chatMessages.map((m, i) => (
                      <div key={i} className={`rda-msg rda-msg-${m.role}`}>
                        <span className="rda-msg-role">{m.role === 'user' ? 'You' : 'Gaply'}</span>
                        <div className="rda-msg-content">{m.content}</div>
                      </div>
                    ))}
                  </div>
                  <div className="rda-chat-input-wrap">
                    <input
                      type="text"
                      className="rda-chat-input"
                      placeholder="Ask about the analysis..."
                      value={chatInput}
                      onChange={e => setChatInput(e.target.value)}
                      onKeyDown={e => e.key === 'Enter' && !e.shiftKey && sendChat()}
                    />
                    <button className="rda-btn rda-btn-primary rda-chat-send" onClick={sendChat} disabled={chatLoading}>
                      {chatLoading ? '...' : 'Send'}
                    </button>
                  </div>
                  <button type="button" className="rda-chat-collapse" onClick={() => setChatExpanded(false)}>Collapse</button>
                </div>
              )}
            </div>

            <div className="rda-card" onClick={() => speak()}>
              <div className="rda-card-icon">🎧</div>
              <h3>Listen to Analysis</h3>
              <p>What&apos;s lacking, how to improve, and publication chances — audio summary.</p>
              <div className="rda-audio-tabs">
                {(['english', 'hindi', 'combined'] as const).map(t => (
                  <button
                    key={t}
                    type="button"
                    className={`rda-audio-tab ${audioTab === t ? 'active' : ''}`}
                    onClick={e => { e.stopPropagation(); setAudioTab(t); }}
                  >
                    {t === 'hindi' ? 'हिंदी' : t === 'english' ? 'English' : 'Combined'}
                  </button>
                ))}
              </div>
            </div>

            <div className="rda-card rda-card-muted">
              <div className="rda-card-icon">📚</div>
              <h3>References &amp; Citation Check</h3>
              <p>Style, authenticity, and journal compliance — coming soon.</p>
            </div>
          </div>

          <div className="rda-audio-bar">
            <span className="rda-audio-label">Audio summary</span>
            <div className="rda-audio-controls">
              <button type="button" className="rda-audio-play" onClick={() => speak()} title="Play">
                ▶ Play
              </button>
              <span className="rda-audio-duration">~2 min</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default ResearchDeepAnalysisPage;
