import React, { useState, useRef, useCallback } from 'react';
import SEO from './SEO';
import { apiFetch } from '../api/config';
import * as XLSX from 'xlsx';
import './DataMaestroProPage.css';

type Variable = { name: string; type: string; role: string; description: string };
type Hypothesis = { id: string; text: string; type: string; test_suggestion: string };
type RecommendedTest = { test_id: string; test_name: string; category: string; reason: string; priority: string };

const ANALYSIS_MODULES = [
  { id: 'data_preparation', label: 'Data Preparation' },
  { id: 'descriptive_stats', label: 'Descriptive Statistics' },
  { id: 'normality_tests', label: 'Normality & Assumptions' },
  { id: 'parametric_tests', label: 'Parametric Tests' },
  { id: 'non_parametric_tests', label: 'Non-Parametric Tests' },
  { id: 'correlation', label: 'Correlation Analysis' },
  { id: 'regression', label: 'Regression Analysis' },
  { id: 'anova', label: 'ANOVA / MANOVA' },
  { id: 'chi_square', label: 'Chi-Square Tests' },
  { id: 'reliability', label: 'Reliability Analysis' },
  { id: 'validity', label: 'Validity Analysis' },
  { id: 'factor_analysis', label: 'Factor Analysis / PCA' },
  { id: 'mediation', label: 'Mediation / Moderation' },
  { id: 'sem', label: 'SEM / Path Analysis' },
];

const DataMaestroProPage: React.FC = () => {
  const [step, setStep] = useState(1);
  const [sessionId, setSessionId] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [progressMsg, setProgressMsg] = useState('');
  const [title, setTitle] = useState('');
  const [objectives, setObjectives] = useState(['']);
  const [methodology, setMethodology] = useState('');
  const [researchArea, setResearchArea] = useState('');
  const [aiSetup, setAiSetup] = useState<any>(null);
  const [hypotheses, setHypotheses] = useState<Hypothesis[]>([]);
  const [variables, setVariables] = useState<Variable[]>([]);
  const [selectedModules, setSelectedModules] = useState<string[]>([]);
  const [autoSelect, setAutoSelect] = useState(true);
  const [uploadedFile, setUploadedFile] = useState<File | null>(null);
  const [parsedData, setParsedData] = useState<any>(null);
  const [questionnaireText, setQuestionnaireText] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [analysisResult, setAnalysisResult] = useState<any>(null);
  const [chatMessages, setChatMessages] = useState<{ role: 'user' | 'assistant'; content: string }[]>([]);
  const [chatInput, setChatInput] = useState('');
  const [chatLoading, setChatLoading] = useState(false);
  const chatEndRef = useRef<HTMLDivElement>(null);

  const handleSetup = async () => {
    if (!title.trim()) { setError('Please enter a research title'); return; }
    const validObj = objectives.filter(o => o.trim());
    if (validObj.length === 0) { setError('Please enter at least one objective'); return; }
    setLoading(true); setError(null); setProgress(0); setProgressMsg('Analyzing your research context...');
    const interval = setInterval(() => setProgress(p => Math.min(p + Math.random() * 8, 90)), 400);
    try {
      const res = await apiFetch('/api/datamaestro/setup', { method: 'POST', body: JSON.stringify({ title: title.trim(), objectives: validObj, methodology, research_area: researchArea }) });
      if (!res.ok) { const e = await res.json().catch(() => ({})); throw new Error(e.error || 'Setup failed'); }
      const data = await res.json();
      setSessionId(data.session_id);
      const setup = data.setup || {};
      if (setup.hypotheses) setHypotheses(setup.hypotheses);
      if (setup.variables) setVariables(setup.variables);
      if (setup.recommended_tests) {
        const autoMods = (setup.recommended_tests as RecommendedTest[]).filter(t => t.priority === 'required' || t.priority === 'recommended').map(t => t.category).filter((v, i, a) => a.indexOf(v) === i);
        setSelectedModules(autoMods.length > 0 ? autoMods : ['descriptive_stats', 'parametric_tests', 'regression', 'correlation']);
      }
      setAiSetup(setup); setStep(2);
    } catch (err: any) { setError(err.message || 'Failed to analyze'); } finally { clearInterval(interval); setLoading(false); setProgress(0); }
  };

  const handleFile = useCallback(async (file: File) => {
    setUploadedFile(file);
    const ext = file.name.split('.').pop()?.toLowerCase();
    if (['csv', 'xls', 'xlsx', 'tsv'].includes(ext || '')) {
      try {
        const buf = await file.arrayBuffer();
        const wb = XLSX.read(buf, { type: 'array' });
        const sheet = wb.Sheets[wb.SheetNames[0]];
        const json = XLSX.utils.sheet_to_json(sheet, { defval: '' });
        const cols = json.length > 0 ? Object.keys(json[0] as object) : [];
        setParsedData({ file_name: file.name, n_rows: json.length, n_columns: cols.length, columns: cols.map(c => ({ name: c, detected_type: typeof (json[0] as any)?.[c] === 'number' ? 'numeric' : 'text' })), sample_rows: json.slice(0, 20) });
      } catch { setParsedData({ file_name: file.name, error: 'Could not parse' }); }
    } else { setParsedData({ file_name: file.name, file_type: ext, n_rows: 0 }); }
  }, []);

  const onDrop = useCallback((e: React.DragEvent) => { e.preventDefault(); setIsDragging(false); if (e.dataTransfer.files[0]) handleFile(e.dataTransfer.files[0]); }, [handleFile]);

  const handleAnalyze = async () => {
    setLoading(true); setError(null); setProgress(0); setProgressMsg('Preparing analysis...'); setStep(3);
    const msgs = ['Processing dataset...', 'Running assumption checks...', 'Performing descriptive analysis...', 'Running inferential tests...', 'Computing effect sizes...', 'Building regression models...', 'Generating figures...', 'Writing interpretations...', 'Compiling methodology report...', 'Finalizing...'];
    let mi = 0;
    const pInt = setInterval(() => setProgress(p => Math.min(p + Math.random() * 3, 92)), 800);
    const mInt = setInterval(() => { setProgressMsg(msgs[mi % msgs.length]); mi++; }, 2500);
    try {
      const payload: any = { session_id: sessionId, title, objectives: objectives.filter(o => o.trim()), hypotheses: hypotheses.map(h => h.text), research_questions: [], methodology: { design: methodology, notes: researchArea }, variables: variables.map(v => ({ name: v.name, type: v.type, role: v.role })), selected_modules: selectedModules, auto_select_tests: autoSelect, questionnaire_text: questionnaireText, dataset_summary: parsedData ? { n_rows: parsedData.n_rows, n_columns: parsedData.n_columns, sample_rows: parsedData.sample_rows || [] } : {}, parsed_tables: parsedData?.sample_rows ? [{ table_id: 'main', source_file: parsedData.file_name, n_rows: parsedData.n_rows, n_columns: parsedData.n_columns, columns: parsedData.columns || [], sample_rows: parsedData.sample_rows || [] }] : [] };
      const res = await apiFetch('/api/datamaestro/analyze', { method: 'POST', body: JSON.stringify(payload) });
      if (!res.ok) { const e = await res.json().catch(() => ({})); throw new Error(e.error || 'Analysis failed'); }
      const data = await res.json();
      setAnalysisResult(data.analysis || {}); setProgress(100); setProgressMsg('Analysis complete!');
      setTimeout(() => setStep(4), 400);
    } catch (err: any) { setError(err.message || 'Analysis failed'); setStep(2); } finally { clearInterval(pInt); clearInterval(mInt); setLoading(false); }
  };

  const sendChat = async () => {
    if (!chatInput.trim() || chatLoading) return;
    const msg = chatInput.trim(); setChatInput('');
    setChatMessages(prev => [...prev, { role: 'user', content: msg }]); setChatLoading(true);
    try {
      const res = await apiFetch('/api/datamaestro/chat', { method: 'POST', body: JSON.stringify({ session_id: sessionId, message: msg, context: { title, objectives: objectives.filter(o => o.trim()), analysis_summary: analysisResult?.executive_summary || '' } }) });
      const data = await res.json();
      setChatMessages(prev => [...prev, { role: 'assistant', content: data.reply || 'I could not generate a response.' }]);
    } catch { setChatMessages(prev => [...prev, { role: 'assistant', content: 'An error occurred. Please try again.' }]); } finally { setChatLoading(false); setTimeout(() => chatEndRef.current?.scrollIntoView({ behavior: 'smooth' }), 100); }
  };

  const generateChartJS = (figures: any[]) => {
    if (!figures?.length) return '';
    const colors = ['#2563eb','#7c3aed','#059669','#d97706','#dc2626','#0891b2','#c026d3','#ea580c'];
    return figures.map((fig: any, idx: number) => {
      if (!fig?.data?.labels || !fig?.data?.datasets?.[0]?.data) return '';
      const canvasId = `chart_${idx}`;
      const chartType = fig.type === 'grouped_bar' || fig.type === 'stacked_bar' || fig.type === 'horizontal_bar' ? 'bar' : (fig.type || 'bar');
      const bgColors = fig.data.datasets[0].backgroundColor || colors.slice(0, fig.data.labels.length);
      const datasetsJS = fig.data.datasets.map((ds: any, di: number) => `{label:"${ds.label || ''}",data:${JSON.stringify(ds.data)},backgroundColor:${JSON.stringify(Array.isArray(bgColors) ? bgColors : colors)},borderColor:"${colors[di % colors.length]}",borderWidth:${chartType === 'line' || chartType === 'scatter' ? 2 : 0},fill:false,tension:0.3}`).join(',');
      return `<div class="figure-container"><h3 class="figure-title">${fig.title || `Figure ${idx + 1}`}</h3><canvas id="${canvasId}" width="700" height="350"></canvas>${fig.description ? `<p class="figure-desc">${fig.description}</p>` : ''}${fig.apa_note ? `<p class="figure-note">${fig.apa_note}</p>` : ''}<script>new Chart(document.getElementById('${canvasId}'),{type:'${chartType}',data:{labels:${JSON.stringify(fig.data.labels)},datasets:[${datasetsJS}]},options:{responsive:true,plugins:{legend:{position:'bottom',labels:{font:{size:12}}},title:{display:false}},scales:{y:{beginAtZero:true,grid:{color:'#e2e8f0'}},x:{grid:{display:false}}}${fig.type === 'horizontal_bar' ? ",indexAxis:'y'" : ''}}});<\/script></div>`;
    }).join('');
  };

  const downloadReport = (type: 'results' | 'methodology') => {
    const r = analysisResult; if (!r) return;
    const chartjsCDN = '<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.7/dist/chart.umd.min.js"><\/script>';
    const css = `<style>
*{margin:0;padding:0;box-sizing:border-box}body{font-family:'Segoe UI','Inter',system-ui,sans-serif;max-width:960px;margin:0 auto;padding:52px 44px;color:#1a1a2e;line-height:1.8;background:#fff}
h1{font-size:28px;font-weight:700;color:#0f172a;border-bottom:3px solid #2563eb;padding-bottom:16px;margin-bottom:10px}
h2{font-size:21px;font-weight:600;color:#1e3a5f;margin:40px 0 16px;padding-bottom:10px;border-bottom:1px solid #e2e8f0}
h3{font-size:17px;font-weight:600;color:#334155;margin:28px 0 12px}
p{margin:10px 0;font-size:15px;color:#334155}
.meta{color:#64748b;font-size:14px;margin-bottom:28px;border-bottom:1px solid #f1f5f9;padding-bottom:16px}
table{width:100%;border-collapse:collapse;margin:20px 0;font-size:13.5px;border:1px solid #e2e8f0;page-break-inside:avoid}
th{background:linear-gradient(135deg,#eff6ff,#f0f4ff);padding:12px 14px;text-align:left;font-weight:700;color:#1e3a5f;border:1px solid #e2e8f0;font-size:12px;text-transform:uppercase;letter-spacing:0.05em}
td{padding:10px 14px;border:1px solid #e2e8f0;color:#475569;vertical-align:top}
tr:nth-child(even){background:#f8fafc}
.interpretation{background:#f8fafc;border-left:4px solid #6366f1;padding:16px 20px;margin:18px 0;border-radius:0 8px 8px 0;font-size:14px;color:#475569}
.finding{background:#eff6ff;border-left:4px solid #2563eb;padding:16px 20px;margin:14px 0;border-radius:0 8px 8px 0}
.badge{display:inline-block;padding:4px 14px;border-radius:6px;font-size:12px;font-weight:700;letter-spacing:0.03em}
.supported{background:#dcfce7;color:#166534}.not-supported{background:#fee2e2;color:#991b1b}.partial{background:#fef9c3;color:#854d0e}
ul,ol{margin:10px 0;padding-left:24px}li{margin:5px 0;font-size:14px}
.figure-container{margin:28px 0;padding:20px;background:#fafbfc;border:1px solid #e2e8f0;border-radius:10px;page-break-inside:avoid}
.figure-title{font-size:14px;font-weight:600;color:#1e3a5f;margin:0 0 12px;text-align:center}
.figure-desc{font-size:13px;color:#475569;margin:12px 0 4px;font-style:italic;text-align:center}
.figure-note{font-size:12px;color:#94a3b8;margin:4px 0 0;text-align:center}
.what-why-how th:nth-child(1){width:18%}.what-why-how th:nth-child(2){width:27%}.what-why-how th:nth-child(3){width:28%}.what-why-how th:nth-child(4){width:27%}
.footer{margin-top:52px;padding-top:18px;border-top:2px solid #e2e8f0;color:#94a3b8;font-size:12px;text-align:center}
@media print{body{padding:20px;font-size:13px}h1{font-size:22px}h2{font-size:18px}.figure-container{break-inside:avoid}canvas{max-height:280px}}
</style>`;

    let html = `<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><title>${title} - ${type === 'results' ? 'Data Analysis & Results Report' : 'Methodology Report'}</title>${css}${chartjsCDN}</head><body>`;

    if (type === 'results') {
      html += `<h1>Data Analysis &amp; Results Report</h1><div class="meta"><strong>Research:</strong> ${title}<br/><strong>Date:</strong> ${new Date().toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' })}<br/><strong>Analyses Performed:</strong> ${r.analyses_performed?.length || 0} statistical tests</div>`;
      if (r.executive_summary) html += `<h2>1. Executive Summary</h2><p>${r.executive_summary}</p>`;
      if (r.descriptive_statistics?.table_html) html += `<h2>2. Descriptive Statistics</h2>${r.descriptive_statistics.summary ? `<p>${r.descriptive_statistics.summary}</p>` : ''}${r.descriptive_statistics.table_html}`;
      if (r.results_chapter?.sections) {
        let secNum = 3;
        if (r.results_chapter.introduction) html += `<h2>${secNum}. Results</h2><p>${r.results_chapter.introduction}</p>`;
        r.results_chapter.sections.forEach((s: any) => {
          html += `<h3>${s.heading}</h3><div>${s.content || ''}</div>`;
          if (s.tables) s.tables.forEach((t: string) => { html += t; });
          if (s.figures) html += generateChartJS(s.figures);
        });
      }
      if (r.analyses_performed) {
        html += `<h2>Detailed Statistical Analyses</h2>`;
        r.analyses_performed.forEach((a: any, i: number) => {
          html += `<h3>${i + 1}. ${a.test_name}</h3>`;
          if (a.why_this_test) html += `<p><em>${a.why_this_test}</em></p>`;
          if (a.assumptions_checked?.length) { html += `<p><strong>Assumptions:</strong> ${a.assumptions_checked.map((ac: any) => `${ac.assumption}: ${ac.result}${ac.detail ? ` (${ac.detail})` : ''}`).join('; ')}</p>`; }
          if (a.result_table_html) html += a.result_table_html;
          if (a.interpretation) html += `<div class="interpretation"><strong>Interpretation:</strong> ${a.interpretation}</div>`;
          if (a.finding_paragraph) html += `<div class="finding">${a.finding_paragraph}</div>`;
        });
      }
      if (r.hypothesis_results?.length) {
        html += `<h2>Hypothesis Testing Summary</h2><table><thead><tr><th>Hypothesis</th><th>Status</th><th>Evidence</th><th>p-value</th></tr></thead><tbody>`;
        r.hypothesis_results.forEach((h: any) => { const cls = h.status?.toLowerCase().includes('not') ? 'not-supported' : h.status?.toLowerCase().includes('partial') ? 'partial' : 'supported'; html += `<tr><td>${h.hypothesis}</td><td><span class="badge ${cls}">${h.status}</span></td><td>${h.evidence || '-'}</td><td>${h.p_value != null ? Number(h.p_value).toFixed(3) : '-'}</td></tr>`; });
        html += `</tbody></table>`;
      }
      if (r.findings_summary) {
        html += `<h2>Key Findings &amp; Implications</h2>`;
        if (r.findings_summary.key_findings?.length) html += `<h3>Key Findings</h3><ol>${r.findings_summary.key_findings.map((f: string) => `<li>${f}</li>`).join('')}</ol>`;
        if (r.findings_summary.practical_implications?.length) html += `<h3>Practical Implications</h3><ul>${r.findings_summary.practical_implications.map((f: string) => `<li>${f}</li>`).join('')}</ul>`;
        if (r.findings_summary.limitations?.length) html += `<h3>Limitations</h3><ul>${r.findings_summary.limitations.map((f: string) => `<li>${f}</li>`).join('')}</ul>`;
        if (r.findings_summary.future_research?.length) html += `<h3>Recommendations for Future Research</h3><ul>${r.findings_summary.future_research.map((f: string) => `<li>${f}</li>`).join('')}</ul>`;
      }
      if (r.results_chapter?.conclusion) html += `<h2>Conclusion</h2><p>${r.results_chapter.conclusion}</p>`;
    } else {
      html += `<h1>Methodology Report</h1><div class="meta"><strong>Research:</strong> ${title}<br/><strong>Date:</strong> ${new Date().toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' })}</div>`;
      if (r.methodology_report?.overview) html += `<h2>1. Overview</h2><p>${r.methodology_report.overview}</p>`;
      if (r.methodology_report?.analysis_justification_table?.length) {
        html += `<h2>2. Analysis Justification Matrix</h2><p>The table below provides a clear, accessible explanation of every statistical analysis performed in this study. Each row describes <em>what</em> the analysis does, <em>why</em> it was chosen for this specific research context, and <em>how</em> it was conducted.</p>`;
        html += `<table class="what-why-how"><thead><tr><th>Analysis</th><th>What It Does</th><th>Why We Used It</th><th>How We Did It</th></tr></thead><tbody>`;
        r.methodology_report.analysis_justification_table.forEach((row: any) => { html += `<tr><td><strong>${row.analysis_name}</strong></td><td>${row.what}</td><td>${row.why}</td><td>${row.how}</td></tr>`; });
        html += `</tbody></table>`;
      }
      if (r.methodology_report?.sections) {
        let secNum = 3;
        r.methodology_report.sections.forEach((s: any) => { html += `<h2>${secNum++}. ${s.heading}</h2><div>${s.content || ''}</div>`; });
      }
    }
    html += `<div class="footer">Generated by Gaply DataMaestro Pro &mdash; ${new Date().toLocaleString()}</div></body></html>`;
    const blob = new Blob([html], { type: 'text/html' }); const a = document.createElement('a'); a.href = URL.createObjectURL(blob);
    a.download = `${title.replace(/[^a-zA-Z0-9 ]/g, '').replace(/\s+/g, '_')}_${type}_report.html`; a.click(); URL.revokeObjectURL(a.href);
  };

  const formatAssistantMsg = (content: string) => {
    return content
      .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
      .replace(/\n\n/g, '</p><p>')
      .replace(/\n- /g, '<br/>&#8226; ')
      .replace(/\n(\d+)\. /g, '<br/>$1. ')
      .replace(/\n/g, '<br/>');
  };

  return (
    <div className="dm-pro-page">
      <SEO title="DataMaestro Pro | Gaply" description="AI-powered statistical analysis for academic research" keywords="statistical analysis, data analysis, research, DataMaestro" />
      <div className="dm-steps">{[1, 2, 3, 4].map(s => <div key={s} className={`dm-step-dot ${step === s || (step === 1.5 && s === 1) ? 'active' : step > s ? 'done' : ''}`} />)}</div>
      {error && <div className="dm-error" style={{ maxWidth: 960, margin: '16px auto' }}>{error}<button onClick={() => setError(null)} style={{ marginLeft: 'auto', background: 'none', border: 'none', color: '#fca5a5', cursor: 'pointer', fontSize: 18 }}>x</button></div>}

      {step === 1 && !loading && (
        <div className="dm-hero"><div className="dm-hero-bg" />
          <div className="dm-hero-content">
            <div className="dm-badge">Pro</div>
            <h1>DataMaestro Pro</h1>
            <p>Comprehensive AI-powered statistical analysis for academic research. Upload your dataset and receive publication-ready reports with tables, charts, and interpretations.</p>
            <button className="dm-btn dm-btn-primary" onClick={() => (setStep as any)(1.5)}>Start Analysis<span style={{ marginLeft: 8 }}>&rarr;</span></button>
          </div>
        </div>
      )}

      {(step === 1.5 || (step === 1 && loading)) && (
        <div className="dm-form-container">
          <h1 className="dm-form-title">Research Setup</h1>
          <p className="dm-form-subtitle">Provide your research details. Our AI designs the optimal analysis plan.</p>
          <div className="dm-card"><h3>Research Title <span className="required">*</span></h3><input className="dm-input" placeholder="e.g., Impact of Social Media on Academic Performance" value={title} onChange={e => setTitle(e.target.value)} /></div>
          <div className="dm-card"><h3>Research Objectives <span className="required">*</span></h3>
            {objectives.map((o, i) => (<div className="dm-list-item" key={i}><input className="dm-input" placeholder={`Objective ${i + 1}`} value={o} onChange={e => { const n = [...objectives]; n[i] = e.target.value; setObjectives(n); }} />{objectives.length > 1 && <button className="dm-remove-btn" onClick={() => setObjectives(objectives.filter((_, j) => j !== i))}>x</button>}</div>))}
            <button className="dm-add-btn" onClick={() => setObjectives([...objectives, ''])}>+ Add Objective</button>
          </div>
          <div className="dm-card"><h3>Research Methodology</h3><textarea className="dm-textarea" placeholder="e.g., Quantitative survey-based study..." value={methodology} onChange={e => setMethodology(e.target.value)} /></div>
          <div className="dm-card"><h3>Research Area</h3><input className="dm-input" placeholder="e.g., Education, Psychology, Business..." value={researchArea} onChange={e => setResearchArea(e.target.value)} /></div>
          <div className="dm-footer-actions">
            <button className="dm-btn dm-btn-secondary" onClick={() => setStep(1)}>Back</button>
            <button className="dm-btn dm-btn-primary" onClick={handleSetup} disabled={loading}>{loading ? 'Analyzing...' : 'Generate Analysis Plan'}</button>
          </div>
        </div>
      )}

      {step === 1 && loading && (<div className="dm-loading"><div className="dm-loading-ring" /><h2>Analyzing Research Context</h2><p>{progressMsg}</p><div className="dm-progress-bar"><div className="dm-progress-fill" style={{ width: `${progress}%` }} /></div></div>)}

      {step === 2 && (
        <div className="dm-form-container">
          <h1 className="dm-form-title">Analysis Configuration</h1>
          <p className="dm-form-subtitle">Review AI suggestions, upload data, and configure analysis modules.</p>
          {aiSetup?.methodology_assessment && (<div className="dm-ai-card"><h4>AI Assessment</h4><p style={{ color: 'rgba(255,255,255,0.7)', fontSize: 14 }}>Design: {aiSetup.methodology_assessment.design_type}</p>{aiSetup.methodology_assessment.suggestions?.map((s: string, i: number) => <div className="dm-ai-tag recommended" key={i}>{s}</div>)}</div>)}
          {aiSetup?.recommended_tests && (<div className="dm-ai-card"><h4>Recommended Tests</h4><div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>{(aiSetup.recommended_tests as RecommendedTest[]).map((t, i) => <div className={`dm-ai-tag ${t.priority}`} key={i} title={t.reason}>{t.test_name}</div>)}</div></div>)}
          <div className="dm-card"><h3>Hypotheses</h3>{hypotheses.map((h, i) => (<div className="dm-list-item" key={i}><input className="dm-input" value={h.text} onChange={e => { const n = [...hypotheses]; n[i] = { ...n[i], text: e.target.value }; setHypotheses(n); }} /><button className="dm-remove-btn" onClick={() => setHypotheses(hypotheses.filter((_, j) => j !== i))}>x</button></div>))}<button className="dm-add-btn" onClick={() => setHypotheses([...hypotheses, { id: `H${hypotheses.length + 1}`, text: '', type: 'directional', test_suggestion: '' }])}>+ Add Hypothesis</button></div>
          <div className="dm-card"><h3>Variables</h3>{variables.map((v, i) => (<div className="dm-var-row" key={i}><input className="dm-input" placeholder="Name" value={v.name} onChange={e => { const n = [...variables]; n[i] = { ...n[i], name: e.target.value }; setVariables(n); }} /><select className="dm-select" value={v.type} onChange={e => { const n = [...variables]; n[i] = { ...n[i], type: e.target.value }; setVariables(n); }}><option value="nominal">Nominal</option><option value="ordinal">Ordinal</option><option value="interval">Interval</option><option value="ratio">Ratio</option></select><select className="dm-select" value={v.role} onChange={e => { const n = [...variables]; n[i] = { ...n[i], role: e.target.value }; setVariables(n); }}><option value="iv">Independent</option><option value="dv">Dependent</option><option value="covariate">Covariate</option><option value="mediator">Mediator</option><option value="moderator">Moderator</option></select><button className="dm-remove-btn" onClick={() => setVariables(variables.filter((_, j) => j !== i))}>x</button></div>))}<button className="dm-add-btn" onClick={() => setVariables([...variables, { name: '', type: 'nominal', role: 'iv', description: '' }])}>+ Add Variable</button></div>
          <div className="dm-card"><h3>Upload Dataset</h3><div className={`dm-upload-zone ${isDragging ? 'dragging' : ''}`} onClick={() => fileInputRef.current?.click()} onDragOver={e => { e.preventDefault(); setIsDragging(true); }} onDragLeave={() => setIsDragging(false)} onDrop={onDrop}><p style={{ fontSize: 28, marginBottom: 8, color: 'rgba(255,255,255,0.2)' }}>+</p><p><strong>Drop file here</strong> or click to browse</p><p style={{ fontSize: 13 }}>CSV, XLS, XLSX, PDF, DOCX, TXT, JSON</p></div><input ref={fileInputRef} type="file" accept=".csv,.xls,.xlsx,.tsv,.txt,.json,.pdf,.docx" style={{ display: 'none' }} onChange={e => { if (e.target.files?.[0]) handleFile(e.target.files[0]); }} />{uploadedFile && <div className="dm-file-info"><span className="file-name">{uploadedFile.name}</span><span className="file-size">{(uploadedFile.size / 1024).toFixed(1)} KB</span>{parsedData?.n_rows > 0 && <span style={{ color: '#86efac', fontSize: 13 }}>{parsedData.n_rows} rows, {parsedData.n_columns} cols</span>}</div>}</div>
          <div className="dm-card"><h3>Questionnaire / Instrument</h3><textarea className="dm-textarea" placeholder="Paste questionnaire items or instrument description..." value={questionnaireText} onChange={e => setQuestionnaireText(e.target.value)} style={{ minHeight: 80 }} /></div>
          <div className="dm-card"><h3>Analysis Modules</h3><div className="dm-toggle-row"><span style={{ fontSize: 14, color: 'rgba(255,255,255,0.6)' }}>Auto-select optimal tests</span><button className={`dm-toggle ${autoSelect ? 'on' : ''}`} onClick={() => setAutoSelect(!autoSelect)} /></div>{!autoSelect && <div className="dm-module-grid">{ANALYSIS_MODULES.map(m => <div key={m.id} className={`dm-module-chip ${selectedModules.includes(m.id) ? 'selected' : ''}`} onClick={() => setSelectedModules(prev => prev.includes(m.id) ? prev.filter(x => x !== m.id) : [...prev, m.id])}><div className="dm-module-check">{selectedModules.includes(m.id) ? '✓' : ''}</div><span>{m.label}</span></div>)}</div>}</div>
          <div className="dm-footer-actions"><button className="dm-btn dm-btn-secondary" onClick={() => (setStep as any)(1.5)}>Back</button><button className="dm-btn dm-btn-primary" onClick={handleAnalyze}>Run Analysis</button></div>
        </div>
      )}

      {step === 3 && <div className="dm-loading"><div className="dm-loading-ring" /><h2>Performing Comprehensive Analysis</h2><p>{progressMsg}</p><div className="dm-progress-bar"><div className="dm-progress-fill" style={{ width: `${progress}%` }} /></div><p style={{ marginTop: 20, fontSize: 13, color: 'rgba(255,255,255,0.3)' }}>This may take 1-2 minutes for thorough analysis</p></div>}

      {step === 4 && analysisResult && (
        <div className="dm-results">
          {/* Completion Banner */}
          <div className="dm-completion-banner">
            <div className="dm-completion-icon">
              <svg width="48" height="48" viewBox="0 0 48 48" fill="none"><circle cx="24" cy="24" r="24" fill="#059669" fillOpacity="0.15"/><path d="M14 24l7 7 13-13" stroke="#059669" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"/></svg>
            </div>
            <div>
              <h1 style={{ fontSize: 24, fontWeight: 700, margin: '0 0 4px' }}>Analysis Complete</h1>
              <p style={{ color: 'rgba(255,255,255,0.5)', fontSize: 14, margin: 0 }}>{title}</p>
              <p style={{ color: 'rgba(255,255,255,0.35)', fontSize: 13, margin: '4px 0 0' }}>{analysisResult.analyses_performed?.length || 0} statistical analyses performed</p>
            </div>
          </div>

          {/* Download Section */}
          <div className="dm-download-section">
            <div className="dm-download-card" onClick={() => downloadReport('results')}>
              <div className="dm-download-icon">
                <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M14 3v4a1 1 0 001 1h4M14 3H7a2 2 0 00-2 2v14a2 2 0 002 2h10a2 2 0 002-2V8l-5-5z"/><path d="M12 11v6M9 14l3 3 3-3"/></svg>
              </div>
              <h3>Data Analysis & Results Report</h3>
              <p>Complete statistical analysis with tables, charts, interpretations, hypothesis testing, and findings. Publication-ready format with interactive graphs.</p>
              <span className="dm-download-badge">HTML with Charts</span>
            </div>
            <div className="dm-download-card" onClick={() => downloadReport('methodology')}>
              <div className="dm-download-icon">
                <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M14 3v4a1 1 0 001 1h4M14 3H7a2 2 0 00-2 2v14a2 2 0 002 2h10a2 2 0 002-2V8l-5-5z"/><path d="M9 9h1M9 13h6M9 17h6"/></svg>
              </div>
              <h3>Methodology Report</h3>
              <p>Clear explanation of every analysis performed — What it does, Why it was chosen for your research, and How it was conducted. Written in accessible language.</p>
              <span className="dm-download-badge">What / Why / How</span>
            </div>
          </div>

          <button className="dm-btn dm-btn-secondary" onClick={() => { setStep(1); setAnalysisResult(null); setChatMessages([]); }} style={{ marginBottom: 40 }}>Start New Analysis</button>

          {/* Chat Section */}
          <div className="dm-chat-section">
            <h2>Chat with Gaply</h2>
            <p style={{ fontSize: 14, color: 'rgba(255,255,255,0.45)', marginBottom: 20 }}>Ask questions about your analysis, request clarifications, explore findings, or request additional interpretations.</p>
            <div className="dm-chat">
              <div className="dm-chat-messages">
                {chatMessages.length === 0 && (
                  <div className="dm-chat-welcome">
                    <p style={{ fontSize: 16, fontWeight: 600, marginBottom: 12 }}>How can I help you understand your analysis?</p>
                    <div className="dm-chat-suggestions">
                      {['Explain the key findings in simple terms', 'What are the limitations of this analysis?', 'How should I interpret the regression results?', 'Suggest additional analyses I should consider'].map((s, i) => (
                        <button key={i} className="dm-chat-suggestion" onClick={() => { setChatInput(s); }}>{s}</button>
                      ))}
                    </div>
                  </div>
                )}
                {chatMessages.map((m, i) => (
                  <div key={i} className={`dm-chat-msg ${m.role}`}>
                    {m.role === 'assistant' && <div className="dm-chat-avatar">G</div>}
                    <div className="dm-chat-bubble">
                      {m.role === 'assistant' ? <div dangerouslySetInnerHTML={{ __html: formatAssistantMsg(m.content) }} /> : m.content}
                    </div>
                  </div>
                ))}
                {chatLoading && <div className="dm-chat-msg assistant"><div className="dm-chat-avatar">G</div><div className="dm-chat-bubble dm-typing"><span /><span /><span /></div></div>}
                <div ref={chatEndRef} />
              </div>
              <div className="dm-chat-input-row">
                <input placeholder="Ask about your analysis..." value={chatInput} onChange={e => setChatInput(e.target.value)} onKeyDown={e => { if (e.key === 'Enter') sendChat(); }} />
                <button className="dm-btn dm-btn-primary dm-btn-sm" onClick={sendChat} disabled={chatLoading}>Send</button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default DataMaestroProPage;
