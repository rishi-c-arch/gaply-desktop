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
  const [summaryTab, setSummaryTab] = useState<'hindi' | 'english' | 'combined'>('english');
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

  const downloadReport = () => {
    if (!result?.full_report_html) return;
    const html = `<!DOCTYPE html><html><head><meta charset="utf-8"><title>Research Deep Analysis Report</title>
<style>body{font-family:system-ui,sans-serif;max-width:800px;margin:40px auto;padding:20px;color:#1a1a2e;line-height:1.7}
h1{font-size:28px;border-bottom:3px solid #3b82f6;padding-bottom:12px}h2{font-size:22px;margin-top:32px}
table{width:100%;border-collapse:collapse;margin:16px 0}th{background:#f0f4ff;padding:12px;text-align:left}
td{padding:10px 12px;border-bottom:1px solid #e5e7eb}.footer{margin-top:40px;padding-top:16px;border-top:1px solid #e5e7eb;color:#6b7280;font-size:13px;text-align:center}</style></head><body>
${result.full_report_html}
<div class="footer">Generated by Gaply Research Deep Analysis — ${new Date().toLocaleString()}</div></body></html>`;
    const blob = new Blob([html], { type: 'text/html' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `research_deep_analysis_${new Date().toISOString().slice(0, 10)}.html`;
    a.click();
    URL.revokeObjectURL(a.href);
  };

  const speak = (text: string, lang: 'hi-IN' | 'en-US') => {
    if (!('speechSynthesis' in window) || !text) return;
    window.speechSynthesis.cancel();
    const u = new SpeechSynthesisUtterance(text.slice(0, 3000));
    u.lang = lang;
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
        <div className="rda-results">
          <div className="rda-results-header">
            <h2>Analysis Results</h2>
            <div className="rda-actions">
              <button className="rda-btn rda-btn-secondary" onClick={downloadReport}>Download Report</button>
              <button className="rda-btn rda-btn-secondary" onClick={reset}>New Analysis</button>
            </div>
          </div>

          <div className="rda-tabs">
            {(['english', 'hindi', 'combined'] as const).map(t => (
              <button
                key={t}
                className={`rda-tab ${summaryTab === t ? 'active' : ''}`}
                onClick={() => setSummaryTab(t)}
              >
                {t === 'hindi' ? 'हिंदी' : t === 'english' ? 'English' : 'Combined'}
              </button>
            ))}
          </div>

          <div className="rda-summary-section">
            <div className="rda-summary-header">
              <h3>Summary</h3>
              <button
                type="button"
                className="rda-audio-btn"
                onClick={() => speak((result[`summary_${summaryTab}`] || ''), summaryTab === 'hindi' ? 'hi-IN' : 'en-US')}
                title="Listen"
              >
                🔊 Play
              </button>
            </div>
            <div className="rda-summary-content">
              {result[`summary_${summaryTab}`] || 'No summary available.'}
            </div>
          </div>

          {result.research_gaps && result.research_gaps.length > 0 && (
            <div className="rda-section">
              <h3>Research Gaps</h3>
              <ul className="rda-list">
                {result.research_gaps.map((g: { gap?: string; severity?: string; papers_related?: string[] }, i: number) => (
                  <li key={i}>
                    <strong>{g.gap}</strong>
                    {g.severity && <span className="rda-badge-sm">{g.severity}</span>}
                    {g.papers_related && <span className="rda-meta">Papers: {g.papers_related.join(', ')}</span>}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {result.methodologies && result.methodologies.length > 0 && (
            <div className="rda-section">
              <h3>Methodologies</h3>
              <ul className="rda-list">
                {result.methodologies.map((m: { paper?: string; methodology?: string; strengths?: string[]; limitations?: string[] }, i: number) => (
                  <li key={i}>
                    <strong>Paper {m.paper}:</strong> {m.methodology}
                    {m.strengths?.length ? <div className="rda-sublist">Strengths: {m.strengths.join('; ')}</div> : null}
                    {m.limitations?.length ? <div className="rda-sublist">Limitations: {m.limitations.join('; ')}</div> : null}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {result.suggested_titles && result.suggested_titles.length > 0 && (
            <div className="rda-section">
              <h3>Suggested Titles</h3>
              <ol className="rda-list rda-numbered">
                {result.suggested_titles.map((t: string, i: number) => (
                  <li key={i}>{t}</li>
                ))}
              </ol>
            </div>
          )}

          {result.target_journals && result.target_journals.length > 0 && (
            <div className="rda-section">
              <h3>Target Journals</h3>
              <ul className="rda-list">
                {result.target_journals.map((j: { journal?: string; quartile?: string; reason?: string }, i: number) => (
                  <li key={i}>
                    <strong>{j.journal}</strong>
                    {j.quartile && <span className="rda-badge-sm">{j.quartile}</span>}
                    {j.reason && <p className="rda-reason">{j.reason}</p>}
                  </li>
                ))}
              </ul>
            </div>
          )}

          <div className="rda-chat-section">
            <h3>Chat with Gaply</h3>
            <div className="rda-chat-messages">
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
              <button className="rda-btn rda-btn-primary" onClick={sendChat} disabled={chatLoading}>
                {chatLoading ? '...' : 'Send'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default ResearchDeepAnalysisPage;
