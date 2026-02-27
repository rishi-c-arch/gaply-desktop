import React, { useState, useRef, useCallback } from 'react';
import SEO from './SEO';
import { apiFetch } from '../api/config';
import * as XLSX from 'xlsx';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, PieChart, Pie, Cell, CartesianGrid, Legend, LineChart, Line, ScatterChart, Scatter, ZAxis } from 'recharts';
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

const COLORS = ['#2563eb', '#7c3aed', '#059669', '#d97706', '#dc2626', '#0891b2', '#c026d3', '#ea580c', '#0d9488', '#9333ea'];

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
        setSelectedModules(autoMods.length > 0 ? autoMods : ['descriptive_stats', 'parametric_tests']);
      }
      setAiSetup(setup); setStep(2);
    } catch (err: any) { setError(err.message || 'Failed to analyze research context'); } finally { clearInterval(interval); setLoading(false); setProgress(0); }
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
      } catch { setParsedData({ file_name: file.name, error: 'Could not parse file' }); }
    } else { setParsedData({ file_name: file.name, file_type: ext, n_rows: 0 }); }
  }, []);

  const onDrop = useCallback((e: React.DragEvent) => { e.preventDefault(); setIsDragging(false); if (e.dataTransfer.files[0]) handleFile(e.dataTransfer.files[0]); }, [handleFile]);

  const handleAnalyze = async () => {
    setLoading(true); setError(null); setProgress(0); setProgressMsg('Preparing analysis...'); setStep(3);
    const msgs = ['Processing dataset...', 'Running statistical tests...', 'Building tables and figures...', 'Generating interpretations...', 'Compiling findings...', 'Finalizing report...'];
    let mi = 0;
    const pInt = setInterval(() => setProgress(p => Math.min(p + Math.random() * 4, 92)), 600);
    const mInt = setInterval(() => { setProgressMsg(msgs[mi % msgs.length]); mi++; }, 3000);
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
    } catch { setChatMessages(prev => [...prev, { role: 'assistant', content: 'Sorry, there was an error. Please try again.' }]); } finally { setChatLoading(false); setTimeout(() => chatEndRef.current?.scrollIntoView({ behavior: 'smooth' }), 100); }
  };

  const downloadReport = (type: 'results' | 'methodology') => {
    const r = analysisResult; if (!r) return;
    const css = `<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Segoe UI','Inter',system-ui,-apple-system,sans-serif;max-width:920px;margin:0 auto;padding:48px 40px;color:#1a1a2e;line-height:1.75;background:#fff}
h1{font-size:26px;font-weight:700;color:#0f172a;border-bottom:3px solid #2563eb;padding-bottom:14px;margin-bottom:8px}
h2{font-size:20px;font-weight:600;color:#1e3a5f;margin:36px 0 14px;padding-bottom:8px;border-bottom:1px solid #e2e8f0}
h3{font-size:17px;font-weight:600;color:#334155;margin:24px 0 10px}
p{margin:10px 0;font-size:15px;color:#334155}
table{width:100%;border-collapse:collapse;margin:20px 0;font-size:14px;border:1px solid #e2e8f0}
th{background:#f0f4ff;padding:12px 14px;text-align:left;font-weight:600;color:#1e3a5f;border:1px solid #e2e8f0;font-size:13px;text-transform:uppercase;letter-spacing:0.03em}
td{padding:10px 14px;border:1px solid #e2e8f0;color:#475569}
tr:nth-child(even){background:#f8fafc}
.meta{color:#64748b;font-size:14px;margin-bottom:24px}
.badge{display:inline-block;padding:4px 14px;border-radius:6px;font-size:13px;font-weight:600}
.supported{background:#dcfce7;color:#166534}.not-supported{background:#fee2e2;color:#991b1b}.partial{background:#fef9c3;color:#854d0e}
.note{background:#f8fafc;border-left:4px solid #2563eb;padding:14px 18px;margin:16px 0;font-size:14px;color:#475569;border-radius:0 8px 8px 0}
.footer{margin-top:48px;padding-top:16px;border-top:1px solid #e2e8f0;color:#94a3b8;font-size:12px;text-align:center}
.what-why-how th:nth-child(1){width:20%}
.what-why-how th:nth-child(2){width:26%}
.what-why-how th:nth-child(3){width:27%}
.what-why-how th:nth-child(4){width:27%}
@media print{body{padding:20px}h1{font-size:22px}h2{font-size:18px}table{page-break-inside:avoid}}
</style>`;

    let html = `<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><title>${title} - ${type === 'results' ? 'Data Analysis Report' : 'Methodology Report'}</title>${css}</head><body>`;

    if (type === 'results') {
      html += `<h1>Data Analysis &amp; Results Report</h1><p class="meta"><strong>Research:</strong> ${title}<br/><strong>Date:</strong> ${new Date().toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' })}</p>`;
      if (r.executive_summary) html += `<h2>Executive Summary</h2><p>${r.executive_summary}</p>`;
      if (r.descriptive_statistics?.table_html) html += `<h2>Descriptive Statistics</h2><p>${r.descriptive_statistics.summary || ''}</p>${r.descriptive_statistics.table_html}`;
      if (r.results_chapter?.sections) {
        r.results_chapter.sections.forEach((s: any) => {
          html += `<h2>${s.heading}</h2><div>${s.content || ''}</div>`;
          if (s.tables) s.tables.forEach((t: string) => { html += t; });
        });
      }
      if (r.analyses_performed) {
        html += `<h2>Detailed Statistical Results</h2>`;
        r.analyses_performed.forEach((a: any, i: number) => {
          html += `<h3>${i + 1}. ${a.test_name}</h3>`;
          if (a.result_table_html) html += a.result_table_html;
          if (a.interpretation) html += `<div class="note"><strong>Interpretation:</strong> ${a.interpretation}</div>`;
          if (a.finding_paragraph) html += `<p>${a.finding_paragraph}</p>`;
        });
      }
      if (r.hypothesis_results) {
        html += `<h2>Hypothesis Testing Summary</h2><table><thead><tr><th>Hypothesis</th><th>Status</th><th>Evidence</th><th>p-value</th></tr></thead><tbody>`;
        r.hypothesis_results.forEach((h: any) => {
          const cls = h.status?.toLowerCase().includes('not') ? 'not-supported' : h.status?.toLowerCase().includes('partial') ? 'partial' : 'supported';
          html += `<tr><td>${h.hypothesis}</td><td><span class="badge ${cls}">${h.status}</span></td><td>${h.evidence || '-'}</td><td>${h.p_value != null ? Number(h.p_value).toFixed(3) : '-'}</td></tr>`;
        });
        html += `</tbody></table>`;
      }
      if (r.findings_summary) {
        html += `<h2>Key Findings</h2><ul>${(r.findings_summary.key_findings || []).map((f: string) => `<li>${f}</li>`).join('')}</ul>`;
        if (r.findings_summary.practical_implications?.length) html += `<h3>Practical Implications</h3><ul>${r.findings_summary.practical_implications.map((f: string) => `<li>${f}</li>`).join('')}</ul>`;
        if (r.findings_summary.limitations?.length) html += `<h3>Limitations</h3><ul>${r.findings_summary.limitations.map((f: string) => `<li>${f}</li>`).join('')}</ul>`;
      }
    } else {
      html += `<h1>Methodology Report</h1><p class="meta"><strong>Research:</strong> ${title}<br/><strong>Date:</strong> ${new Date().toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' })}</p>`;
      if (r.methodology_report?.overview) html += `<h2>Overview</h2><p>${r.methodology_report.overview}</p>`;
      if (r.methodology_report?.analysis_justification_table?.length) {
        html += `<h2>Analysis Justification</h2><p>The following table provides a clear explanation of every statistical analysis performed in this study — what each analysis does, why it was selected for this specific research, and how it was conducted.</p>`;
        html += `<table class="what-why-how"><thead><tr><th>Analysis</th><th>What</th><th>Why</th><th>How</th></tr></thead><tbody>`;
        r.methodology_report.analysis_justification_table.forEach((row: any) => {
          html += `<tr><td><strong>${row.analysis_name}</strong></td><td>${row.what}</td><td>${row.why}</td><td>${row.how}</td></tr>`;
        });
        html += `</tbody></table>`;
      }
      if (r.methodology_report?.sections) {
        r.methodology_report.sections.forEach((s: any) => { html += `<h2>${s.heading}</h2><div>${s.content || ''}</div>`; });
      }
    }
    html += `<div class="footer">Generated by Gaply DataMaestro Pro &mdash; ${new Date().toLocaleString()}</div></body></html>`;
    const blob = new Blob([html], { type: 'text/html' });
    const a = document.createElement('a'); a.href = URL.createObjectURL(blob);
    a.download = `${title.replace(/[^a-zA-Z0-9 ]/g, '').replace(/\s+/g, '_')}_${type}_report.html`;
    a.click(); URL.revokeObjectURL(a.href);
  };

  const renderChart = (fig: any, idx: number) => {
    if (!fig?.data?.labels || !fig?.data?.datasets?.[0]?.data) return null;
    const { labels, datasets } = fig.data;
    const chartData = labels.map((l: string, i: number) => {
      const point: any = { name: l };
      datasets.forEach((ds: any, di: number) => { point[ds.label || `Series ${di + 1}`] = ds.data[i]; });
      return point;
    });
    const keys = datasets.map((ds: any, di: number) => ds.label || `Series ${di + 1}`);

    return (
      <div className="dm-chart-wrap" key={idx}>
        <div className="dm-chart-title">{fig.title}</div>
        <ResponsiveContainer width="100%" height={320}>
          {fig.type === 'pie' ? (
            <PieChart>
              <Pie data={labels.map((l: string, i: number) => ({ name: l, value: datasets[0].data[i] }))} dataKey="value" nameKey="name" cx="50%" cy="50%" outerRadius={110} label={({ name, percent }: any) => `${name} ${(percent * 100).toFixed(0)}%`}>
                {labels.map((_: any, i: number) => <Cell key={i} fill={COLORS[i % COLORS.length]} />)}
              </Pie>
              <Tooltip contentStyle={{ background: '#fff', border: '1px solid #e2e8f0', borderRadius: 8, fontSize: 13 }} />
              <Legend wrapperStyle={{ fontSize: 13 }} />
            </PieChart>
          ) : fig.type === 'line' ? (
            <LineChart data={chartData}>
              <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.06)" />
              <XAxis dataKey="name" tick={{ fill: 'rgba(255,255,255,0.5)', fontSize: 12 }} />
              <YAxis tick={{ fill: 'rgba(255,255,255,0.5)', fontSize: 12 }} />
              <Tooltip contentStyle={{ background: '#1e1e2e', border: '1px solid #333', borderRadius: 8, color: '#fff', fontSize: 13 }} />
              <Legend wrapperStyle={{ fontSize: 13 }} />
              {keys.map((k: string, ki: number) => <Line key={ki} type="monotone" dataKey={k} stroke={COLORS[ki % COLORS.length]} strokeWidth={2} dot={{ r: 4 }} />)}
            </LineChart>
          ) : fig.type === 'scatter' ? (
            <ScatterChart>
              <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.06)" />
              <XAxis dataKey="name" tick={{ fill: 'rgba(255,255,255,0.5)', fontSize: 12 }} />
              <YAxis tick={{ fill: 'rgba(255,255,255,0.5)', fontSize: 12 }} />
              <ZAxis range={[40, 400]} />
              <Tooltip contentStyle={{ background: '#1e1e2e', border: '1px solid #333', borderRadius: 8, color: '#fff', fontSize: 13 }} />
              <Scatter data={chartData} fill={COLORS[0]} />
            </ScatterChart>
          ) : (
            <BarChart data={chartData}>
              <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.06)" />
              <XAxis dataKey="name" tick={{ fill: 'rgba(255,255,255,0.5)', fontSize: 12 }} />
              <YAxis tick={{ fill: 'rgba(255,255,255,0.5)', fontSize: 12 }} />
              <Tooltip contentStyle={{ background: '#1e1e2e', border: '1px solid #333', borderRadius: 8, color: '#fff', fontSize: 13 }} />
              <Legend wrapperStyle={{ fontSize: 13 }} />
              {keys.map((k: string, ki: number) => <Bar key={ki} dataKey={k} fill={COLORS[ki % COLORS.length]} radius={[4, 4, 0, 0]} />)}
            </BarChart>
          )}
        </ResponsiveContainer>
        {fig.description && <p className="dm-chart-note">{fig.description}</p>}
        {fig.apa_note && <p className="dm-chart-apa">{fig.apa_note}</p>}
      </div>
    );
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
            <p>Upload your research data and our AI performs comprehensive statistical analysis — producing publication-ready tables, charts, interpretations, and downloadable reports.</p>
            <button className="dm-btn dm-btn-primary" onClick={() => (setStep as any)(1.5)}>Start Analysis<span style={{ marginLeft: 8 }}>&rarr;</span></button>
          </div>
        </div>
      )}

      {(step === 1.5 || (step === 1 && loading)) && (
        <div className="dm-form-container">
          <h1 className="dm-form-title">Research Setup</h1>
          <p className="dm-form-subtitle">Tell us about your research. Our AI will design the optimal analysis plan.</p>
          <div className="dm-card"><h3>Research Title <span className="required">*</span></h3><input className="dm-input" placeholder="e.g., Impact of Social Media on Academic Performance" value={title} onChange={e => setTitle(e.target.value)} /></div>
          <div className="dm-card"><h3>Research Objectives <span className="required">*</span></h3>
            {objectives.map((o, i) => (<div className="dm-list-item" key={i}><input className="dm-input" placeholder={`Objective ${i + 1}`} value={o} onChange={e => { const n = [...objectives]; n[i] = e.target.value; setObjectives(n); }} />{objectives.length > 1 && <button className="dm-remove-btn" onClick={() => setObjectives(objectives.filter((_, j) => j !== i))}>x</button>}</div>))}
            <button className="dm-add-btn" onClick={() => setObjectives([...objectives, ''])}>+ Add Objective</button>
          </div>
          <div className="dm-card"><h3>Research Methodology</h3><textarea className="dm-textarea" placeholder="e.g., Quantitative survey-based study using Likert scale questionnaire..." value={methodology} onChange={e => setMethodology(e.target.value)} /></div>
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
          <p className="dm-form-subtitle">Review AI suggestions, upload data, and configure the analysis modules.</p>
          {aiSetup?.methodology_assessment && (<div className="dm-ai-card"><h4>AI Assessment</h4><p style={{ color: 'rgba(255,255,255,0.7)', fontSize: 14 }}>Design: {aiSetup.methodology_assessment.design_type}</p>{aiSetup.methodology_assessment.suggestions?.map((s: string, i: number) => <div className="dm-ai-tag recommended" key={i}>{s}</div>)}</div>)}
          {aiSetup?.recommended_tests && (<div className="dm-ai-card"><h4>Recommended Tests</h4><div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>{(aiSetup.recommended_tests as RecommendedTest[]).map((t, i) => <div className={`dm-ai-tag ${t.priority}`} key={i} title={t.reason}>{t.test_name}</div>)}</div></div>)}
          <div className="dm-card"><h3>Hypotheses</h3>
            {hypotheses.map((h, i) => (<div className="dm-list-item" key={i}><input className="dm-input" value={h.text} onChange={e => { const n = [...hypotheses]; n[i] = { ...n[i], text: e.target.value }; setHypotheses(n); }} /><button className="dm-remove-btn" onClick={() => setHypotheses(hypotheses.filter((_, j) => j !== i))}>x</button></div>))}
            <button className="dm-add-btn" onClick={() => setHypotheses([...hypotheses, { id: `H${hypotheses.length + 1}`, text: '', type: 'directional', test_suggestion: '' }])}>+ Add Hypothesis</button>
          </div>
          <div className="dm-card"><h3>Variables</h3>
            {variables.map((v, i) => (<div className="dm-var-row" key={i}><input className="dm-input" placeholder="Variable name" value={v.name} onChange={e => { const n = [...variables]; n[i] = { ...n[i], name: e.target.value }; setVariables(n); }} /><select className="dm-select" value={v.type} onChange={e => { const n = [...variables]; n[i] = { ...n[i], type: e.target.value }; setVariables(n); }}><option value="nominal">Nominal</option><option value="ordinal">Ordinal</option><option value="interval">Interval</option><option value="ratio">Ratio</option></select><select className="dm-select" value={v.role} onChange={e => { const n = [...variables]; n[i] = { ...n[i], role: e.target.value }; setVariables(n); }}><option value="iv">Independent</option><option value="dv">Dependent</option><option value="covariate">Covariate</option><option value="mediator">Mediator</option><option value="moderator">Moderator</option></select><button className="dm-remove-btn" onClick={() => setVariables(variables.filter((_, j) => j !== i))}>x</button></div>))}
            <button className="dm-add-btn" onClick={() => setVariables([...variables, { name: '', type: 'nominal', role: 'iv', description: '' }])}>+ Add Variable</button>
          </div>
          <div className="dm-card"><h3>Upload Dataset</h3><p className="dm-label">CSV, Excel, PDF, DOCX, or text file</p>
            <div className={`dm-upload-zone ${isDragging ? 'dragging' : ''}`} onClick={() => fileInputRef.current?.click()} onDragOver={e => { e.preventDefault(); setIsDragging(true); }} onDragLeave={() => setIsDragging(false)} onDrop={onDrop}>
              <p style={{ fontSize: 32, marginBottom: 8 }}>+</p><p><strong>Drop your file here</strong> or click to browse</p><p style={{ fontSize: 13 }}>Supports CSV, XLS, XLSX, PDF, DOCX, TXT, JSON</p>
            </div>
            <input ref={fileInputRef} type="file" accept=".csv,.xls,.xlsx,.tsv,.txt,.json,.pdf,.docx" style={{ display: 'none' }} onChange={e => { if (e.target.files?.[0]) handleFile(e.target.files[0]); }} />
            {uploadedFile && <div className="dm-file-info"><span className="file-name">{uploadedFile.name}</span><span className="file-size">{(uploadedFile.size / 1024).toFixed(1)} KB</span>{parsedData?.n_rows > 0 && <span style={{ color: '#86efac', fontSize: 13 }}>{parsedData.n_rows} rows, {parsedData.n_columns} columns</span>}</div>}
          </div>
          <div className="dm-card"><h3>Questionnaire / Instrument</h3><textarea className="dm-textarea" placeholder="Paste questionnaire items, scales, or instrument description..." value={questionnaireText} onChange={e => setQuestionnaireText(e.target.value)} style={{ minHeight: 80 }} /></div>
          <div className="dm-card"><h3>Analysis Modules</h3>
            <div className="dm-toggle-row"><span style={{ fontSize: 14, color: 'rgba(255,255,255,0.6)' }}>Auto-select optimal tests</span><button className={`dm-toggle ${autoSelect ? 'on' : ''}`} onClick={() => setAutoSelect(!autoSelect)} /></div>
            {!autoSelect && <div className="dm-module-grid">{ANALYSIS_MODULES.map(m => <div key={m.id} className={`dm-module-chip ${selectedModules.includes(m.id) ? 'selected' : ''}`} onClick={() => setSelectedModules(prev => prev.includes(m.id) ? prev.filter(x => x !== m.id) : [...prev, m.id])}><div className="dm-module-check">{selectedModules.includes(m.id) ? '✓' : ''}</div><span>{m.label}</span></div>)}</div>}
          </div>
          <div className="dm-footer-actions"><button className="dm-btn dm-btn-secondary" onClick={() => (setStep as any)(1.5)}>Back</button><button className="dm-btn dm-btn-primary" onClick={handleAnalyze}>Run Analysis</button></div>
        </div>
      )}

      {step === 3 && <div className="dm-loading"><div className="dm-loading-ring" /><h2>Performing Analysis</h2><p>{progressMsg}</p><div className="dm-progress-bar"><div className="dm-progress-fill" style={{ width: `${progress}%` }} /></div></div>}

      {step === 4 && analysisResult && (
        <div className="dm-results">
          <div className="dm-results-header">
            <div><h1>Analysis Results</h1><p style={{ color: 'rgba(255,255,255,0.5)', fontSize: 14, margin: 0 }}>{title}</p></div>
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="dm-btn dm-btn-secondary dm-btn-sm" onClick={() => downloadReport('results')}>Download Results Report</button>
              <button className="dm-btn dm-btn-secondary dm-btn-sm" onClick={() => downloadReport('methodology')}>Download Methods Report</button>
              <button className="dm-btn dm-btn-secondary dm-btn-sm" onClick={() => { setStep(1); setAnalysisResult(null); }}>New Analysis</button>
            </div>
          </div>

          {/* Executive Summary */}
          {analysisResult.executive_summary && <div className="dm-section"><h2>Executive Summary</h2><p>{analysisResult.executive_summary}</p></div>}

          {/* Descriptive Statistics */}
          {analysisResult.descriptive_statistics?.table_html && <div className="dm-section"><h2>Descriptive Statistics</h2>{analysisResult.descriptive_statistics.summary && <p>{analysisResult.descriptive_statistics.summary}</p>}<div dangerouslySetInnerHTML={{ __html: analysisResult.descriptive_statistics.table_html }} /></div>}

          {/* Results Chapter */}
          {analysisResult.results_chapter?.introduction && <div className="dm-section"><h2>Results</h2><p>{analysisResult.results_chapter.introduction}</p></div>}
          {analysisResult.results_chapter?.sections?.map((s: any, si: number) => (
            <div className="dm-section" key={si}>
              <h2>{s.heading}</h2>
              <div dangerouslySetInnerHTML={{ __html: s.content || '' }} />
              {s.tables?.map((t: string, ti: number) => <div key={ti} dangerouslySetInnerHTML={{ __html: t }} />)}
              {s.figures?.map((f: any, fi: number) => renderChart(f, fi))}
            </div>
          ))}

          {/* Detailed Analyses */}
          {analysisResult.analyses_performed?.map((a: any, i: number) => (
            <div className="dm-section" key={`analysis-${i}`}>
              <h2>{a.test_name}</h2>
              <p style={{ fontSize: 13, color: '#a5b4fc', marginBottom: 12 }}>{a.category}{a.why_this_test ? ` — ${a.why_this_test}` : ''}</p>
              {a.assumptions_checked?.length > 0 && <div style={{ marginBottom: 12 }}><strong style={{ fontSize: 13, color: 'rgba(255,255,255,0.5)' }}>Assumptions:</strong>{a.assumptions_checked.map((ac: any, ai: number) => <span key={ai} style={{ display: 'inline-block', margin: '2px 4px', padding: '3px 10px', background: ac.result?.toLowerCase().includes('met') ? 'rgba(34,197,94,0.1)' : 'rgba(239,68,68,0.1)', border: `1px solid ${ac.result?.toLowerCase().includes('met') ? 'rgba(34,197,94,0.2)' : 'rgba(239,68,68,0.2)'}`, borderRadius: 6, fontSize: 12 }}>{ac.assumption}: {ac.result}</span>)}</div>}
              {a.result_table_html && <div dangerouslySetInnerHTML={{ __html: a.result_table_html }} />}
              {a.interpretation && <div className="dm-interpretation"><strong>Interpretation:</strong> {a.interpretation}</div>}
              {a.finding_paragraph && <div className="dm-finding"><p>{a.finding_paragraph}</p></div>}
            </div>
          ))}

          {/* Hypothesis Results */}
          {analysisResult.hypothesis_results?.length > 0 && (
            <div className="dm-section">
              <h2>Hypothesis Testing Summary</h2>
              {analysisResult.hypothesis_results.map((h: any, i: number) => (
                <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '14px 0', borderBottom: '1px solid rgba(255,255,255,0.04)' }}>
                  <span className={`dm-hypothesis-badge ${h.status?.toLowerCase().includes('not') ? 'not-supported' : h.status?.toLowerCase().includes('partial') ? 'partial' : 'supported'}`}>{h.status}</span>
                  <span style={{ flex: 1, fontSize: 14 }}>{h.hypothesis}</span>
                  {h.p_value != null && <span style={{ fontSize: 13, color: 'rgba(255,255,255,0.4)', fontFamily: 'monospace' }}>p = {Number(h.p_value).toFixed(3)}</span>}
                </div>
              ))}
            </div>
          )}

          {/* Methodology WHY/HOW/WHAT Table */}
          {analysisResult.methodology_report?.analysis_justification_table?.length > 0 && (
            <div className="dm-section">
              <h2>Analysis Justification</h2>
              <p style={{ marginBottom: 16, fontSize: 14, color: 'rgba(255,255,255,0.6)' }}>A clear explanation of every analysis performed — what it does, why it was selected, and how it was conducted.</p>
              <table className="dm-result-table dm-justify-table">
                <thead><tr><th>Analysis</th><th>What</th><th>Why</th><th>How</th></tr></thead>
                <tbody>{analysisResult.methodology_report.analysis_justification_table.map((row: any, i: number) => (
                  <tr key={i}><td><strong>{row.analysis_name}</strong></td><td>{row.what}</td><td>{row.why}</td><td>{row.how}</td></tr>
                ))}</tbody>
              </table>
            </div>
          )}

          {/* Key Findings */}
          {analysisResult.findings_summary && (
            <div className="dm-section">
              <h2>Key Findings</h2>
              {analysisResult.findings_summary.key_findings?.map((f: string, i: number) => <p key={i} style={{ paddingLeft: 16, borderLeft: '3px solid #2563eb', marginBottom: 12 }}>{f}</p>)}
              {analysisResult.findings_summary.practical_implications?.length > 0 && <><h3 style={{ fontSize: 16, fontWeight: 600, margin: '20px 0 8px', color: 'rgba(255,255,255,0.8)' }}>Practical Implications</h3>{analysisResult.findings_summary.practical_implications.map((f: string, i: number) => <p key={i}>• {f}</p>)}</>}
              {analysisResult.findings_summary.limitations?.length > 0 && <><h3 style={{ fontSize: 16, fontWeight: 600, margin: '20px 0 8px', color: 'rgba(255,255,255,0.8)' }}>Limitations</h3>{analysisResult.findings_summary.limitations.map((f: string, i: number) => <p key={i}>• {f}</p>)}</>}
            </div>
          )}

          {/* Results Chapter Conclusion */}
          {analysisResult.results_chapter?.conclusion && <div className="dm-section"><h2>Conclusion</h2><p>{analysisResult.results_chapter.conclusion}</p></div>}

          {/* Chat with Gaply */}
          <div className="dm-section" style={{ marginTop: 40 }}>
            <h2>Chat with Gaply</h2>
            <p style={{ fontSize: 14, color: 'rgba(255,255,255,0.5)', marginBottom: 16 }}>Ask questions about your analysis, request clarifications, or explore findings further.</p>
            <div className="dm-chat">
              <div className="dm-chat-messages">
                {chatMessages.length === 0 && <div style={{ textAlign: 'center', padding: 32, color: 'rgba(255,255,255,0.25)' }}><p>Ask anything about your analysis results</p></div>}
                {chatMessages.map((m, i) => <div key={i} className={`dm-chat-msg ${m.role}`}>{m.role === 'assistant' ? <div dangerouslySetInnerHTML={{ __html: m.content.replace(/\n/g, '<br/>') }} /> : m.content}</div>)}
                {chatLoading && <div className="dm-chat-msg assistant" style={{ opacity: 0.5 }}>Thinking...</div>}
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
