import React, { useState, useRef, useCallback } from 'react';
import SEO from './SEO';
import { apiFetch } from '../api/config';
import * as XLSX from 'xlsx';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, PieChart, Pie, Cell, CartesianGrid, Legend } from 'recharts';
import './DataMaestroProPage.css';

type Variable = { name: string; type: string; role: string; description: string };
type Hypothesis = { id: string; text: string; type: string; test_suggestion: string };
type RecommendedTest = { test_id: string; test_name: string; category: string; reason: string; priority: string };

const ANALYSIS_MODULES = [
  { id: 'data_preparation', label: 'Data Preparation', icon: '🔧' },
  { id: 'descriptive_stats', label: 'Descriptive Statistics', icon: '📊' },
  { id: 'normality_tests', label: 'Normality & Assumptions', icon: '📐' },
  { id: 'parametric_tests', label: 'Parametric Tests', icon: '📈' },
  { id: 'non_parametric_tests', label: 'Non-Parametric Tests', icon: '📉' },
  { id: 'correlation', label: 'Correlation Analysis', icon: '🔗' },
  { id: 'regression', label: 'Regression Analysis', icon: '📏' },
  { id: 'anova', label: 'ANOVA / MANOVA', icon: '⚖️' },
  { id: 'chi_square', label: 'Chi-Square Tests', icon: '🎲' },
  { id: 'reliability', label: 'Reliability (Cronbach\'s α)', icon: '🔒' },
  { id: 'validity', label: 'Validity Analysis', icon: '✅' },
  { id: 'factor_analysis', label: 'Factor Analysis / PCA', icon: '🧩' },
  { id: 'mediation', label: 'Mediation / Moderation', icon: '🔄' },
  { id: 'sem', label: 'SEM / Path Analysis', icon: '🌐' },
  { id: 'time_series', label: 'Time Series Analysis', icon: '⏳' },
  { id: 'cluster_analysis', label: 'Cluster Analysis', icon: '🎯' },
];

const CHART_COLORS = ['#3b82f6', '#8b5cf6', '#22c55e', '#f59e0b', '#ef4444', '#06b6d4', '#ec4899', '#f97316', '#14b8a6', '#a855f7'];

const DataMaestroProPage: React.FC = () => {
  const [step, setStep] = useState(1);
  const [sessionId, setSessionId] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [progressMsg, setProgressMsg] = useState('');

  // Step 1: Research Setup
  const [title, setTitle] = useState('');
  const [objectives, setObjectives] = useState(['']);
  const [methodology, setMethodology] = useState('');
  const [researchArea, setResearchArea] = useState('');

  // Step 2: AI Config
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

  // Step 4: Results
  const [analysisResult, setAnalysisResult] = useState<any>(null);
  const [resultTab, setResultTab] = useState<'summary' | 'results' | 'methodology' | 'findings' | 'chat'>('summary');

  // Chat
  const [chatMessages, setChatMessages] = useState<{ role: 'user' | 'assistant'; content: string }[]>([]);
  const [chatInput, setChatInput] = useState('');
  const [chatLoading, setChatLoading] = useState(false);

  // --- Step 1: Setup submission ---
  const handleSetup = async () => {
    if (!title.trim()) { setError('Please enter a research title'); return; }
    const validObj = objectives.filter(o => o.trim());
    if (validObj.length === 0) { setError('Please enter at least one objective'); return; }

    setLoading(true);
    setError(null);
    setProgress(0);
    setProgressMsg('Analyzing your research context...');

    const interval = setInterval(() => {
      setProgress(p => Math.min(p + Math.random() * 8, 90));
    }, 400);

    try {
      const res = await apiFetch('/api/datamaestro/setup', {
        method: 'POST',
        body: JSON.stringify({ title: title.trim(), objectives: validObj, methodology, research_area: researchArea }),
      });
      if (!res.ok) { const e = await res.json().catch(() => ({})); throw new Error(e.error || 'Setup failed'); }
      const data = await res.json();
      setSessionId(data.session_id);
      const setup = data.setup || {};

      if (setup.hypotheses) setHypotheses(setup.hypotheses);
      if (setup.variables) setVariables(setup.variables);
      if (setup.recommended_tests) {
        const autoMods = (setup.recommended_tests as RecommendedTest[])
          .filter(t => t.priority === 'required' || t.priority === 'recommended')
          .map(t => t.category)
          .filter((v, i, a) => a.indexOf(v) === i);
        setSelectedModules(autoMods.length > 0 ? autoMods : ['descriptive_stats', 'parametric_tests']);
      }
      setAiSetup(setup);
      setStep(2);
    } catch (err: any) {
      setError(err.message || 'Failed to analyze research context');
    } finally {
      clearInterval(interval);
      setLoading(false);
      setProgress(0);
    }
  };

  // --- File upload ---
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
        setParsedData({
          file_name: file.name,
          n_rows: json.length,
          n_columns: cols.length,
          columns: cols.map(c => ({ name: c, detected_type: typeof (json[0] as any)?.[c] === 'number' ? 'numeric' : 'text' })),
          sample_rows: json.slice(0, 20),
        });
      } catch { setParsedData({ file_name: file.name, error: 'Could not parse file' }); }
    } else {
      setParsedData({ file_name: file.name, file_type: ext, n_rows: 0 });
    }
  }, []);

  const onDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    if (e.dataTransfer.files[0]) handleFile(e.dataTransfer.files[0]);
  }, [handleFile]);

  // --- Step 2: Run analysis ---
  const handleAnalyze = async () => {
    setLoading(true);
    setError(null);
    setProgress(0);
    setProgressMsg('Preparing your analysis...');
    setStep(3);

    const msgs = ['Processing dataset...', 'Running statistical tests...', 'Building tables and charts...', 'Generating interpretations...', 'Compiling findings...', 'Finalizing report...'];
    let mi = 0;
    const pInt = setInterval(() => setProgress(p => Math.min(p + Math.random() * 4, 92)), 600);
    const mInt = setInterval(() => { setProgressMsg(msgs[mi % msgs.length]); mi++; }, 3000);

    try {
      const payload: any = {
        session_id: sessionId,
        title,
        objectives: objectives.filter(o => o.trim()),
        hypotheses: hypotheses.map(h => h.text),
        research_questions: [],
        methodology: { design: methodology, notes: researchArea },
        variables: variables.map(v => ({ name: v.name, type: v.type, role: v.role })),
        selected_modules: selectedModules,
        auto_select_tests: autoSelect,
        questionnaire_text: questionnaireText,
        dataset_summary: parsedData ? { n_rows: parsedData.n_rows, n_columns: parsedData.n_columns, sample_rows: parsedData.sample_rows || [] } : {},
        parsed_tables: parsedData?.sample_rows ? [{ table_id: 'main', source_file: parsedData.file_name, n_rows: parsedData.n_rows, n_columns: parsedData.n_columns, columns: parsedData.columns || [], sample_rows: parsedData.sample_rows || [] }] : [],
      };

      const res = await apiFetch('/api/datamaestro/analyze', { method: 'POST', body: JSON.stringify(payload) });
      if (!res.ok) { const e = await res.json().catch(() => ({})); throw new Error(e.error || 'Analysis failed'); }
      const data = await res.json();
      setAnalysisResult(data.analysis || {});
      setProgress(100);
      setProgressMsg('Analysis complete!');
      setTimeout(() => setStep(4), 400);
    } catch (err: any) {
      setError(err.message || 'Analysis failed');
      setStep(2);
    } finally {
      clearInterval(pInt);
      clearInterval(mInt);
      setLoading(false);
    }
  };

  // --- Chat ---
  const sendChat = async () => {
    if (!chatInput.trim() || chatLoading) return;
    const msg = chatInput.trim();
    setChatInput('');
    setChatMessages(prev => [...prev, { role: 'user', content: msg }]);
    setChatLoading(true);
    try {
      const res = await apiFetch('/api/datamaestro/chat', {
        method: 'POST',
        body: JSON.stringify({
          session_id: sessionId,
          message: msg,
          context: { title, objectives: objectives.filter(o => o.trim()), analysis_summary: analysisResult?.executive_summary || '' },
        }),
      });
      const data = await res.json();
      setChatMessages(prev => [...prev, { role: 'assistant', content: data.reply || 'I could not generate a response.' }]);
    } catch {
      setChatMessages(prev => [...prev, { role: 'assistant', content: 'Sorry, there was an error. Please try again.' }]);
    } finally {
      setChatLoading(false);
    }
  };

  // --- Download report ---
  const downloadReport = (type: 'results' | 'methodology') => {
    const r = analysisResult;
    if (!r) return;
    let html = `<!DOCTYPE html><html><head><meta charset="utf-8"><title>${title} - ${type === 'results' ? 'Data Analysis Report' : 'Methodology Report'}</title>
<style>body{font-family:'Inter',system-ui,sans-serif;max-width:900px;margin:40px auto;padding:20px;color:#1a1a2e;line-height:1.7}
h1{font-size:28px;border-bottom:3px solid #3b82f6;padding-bottom:12px}h2{font-size:22px;color:#1e3a5f;margin-top:32px}h3{font-size:18px;color:#374151}
table{width:100%;border-collapse:collapse;margin:16px 0;font-size:14px}th{background:#f0f4ff;padding:12px;text-align:left;font-weight:600;border-bottom:2px solid #3b82f6}
td{padding:10px 12px;border-bottom:1px solid #e5e7eb}.badge{display:inline-block;padding:4px 12px;border-radius:6px;font-size:13px;font-weight:600}
.supported{background:#dcfce7;color:#166534}.not-supported{background:#fee2e2;color:#991b1b}.partial{background:#fef9c3;color:#854d0e}
p{margin:8px 0}ul{margin:8px 0;padding-left:20px}li{margin:4px 0}.footer{margin-top:40px;padding-top:16px;border-top:1px solid #e5e7eb;color:#6b7280;font-size:13px;text-align:center}</style></head><body>`;

    if (type === 'results') {
      html += `<h1>📊 Data Analysis & Results Report</h1><p><strong>Research:</strong> ${title}</p><p><strong>Date:</strong> ${new Date().toLocaleDateString()}</p>`;
      if (r.executive_summary) html += `<h2>Executive Summary</h2><p>${r.executive_summary}</p>`;
      if (r.results_chapter?.sections) {
        r.results_chapter.sections.forEach((s: any) => {
          html += `<h2>${s.heading}</h2><div>${s.content}</div>`;
          if (s.tables) s.tables.forEach((t: string) => { html += t; });
        });
      }
      if (r.analyses_performed) {
        html += `<h2>Detailed Statistical Results</h2>`;
        r.analyses_performed.forEach((a: any) => {
          html += `<h3>${a.test_name}</h3>`;
          if (a.result_table_html) html += a.result_table_html;
          if (a.interpretation) html += `<p><strong>Interpretation:</strong> ${a.interpretation}</p>`;
          if (a.finding_paragraph) html += `<p>${a.finding_paragraph}</p>`;
        });
      }
      if (r.hypothesis_results) {
        html += `<h2>Hypothesis Testing Results</h2><table><tr><th>Hypothesis</th><th>Status</th><th>Evidence</th></tr>`;
        r.hypothesis_results.forEach((h: any) => {
          const cls = h.status?.toLowerCase().includes('not') ? 'not-supported' : h.status?.toLowerCase().includes('partial') ? 'partial' : 'supported';
          html += `<tr><td>${h.hypothesis}</td><td><span class="badge ${cls}">${h.status}</span></td><td>${h.evidence || ''}</td></tr>`;
        });
        html += `</table>`;
      }
    } else {
      html += `<h1>📋 Methodology Report</h1><p><strong>Research:</strong> ${title}</p><p><strong>Date:</strong> ${new Date().toLocaleDateString()}</p>`;
      if (r.methodology_report?.sections) {
        r.methodology_report.sections.forEach((s: any) => {
          html += `<h2>${s.heading}</h2><div>${s.content}</div>`;
          if (s.table_html) html += s.table_html;
        });
      }
    }
    html += `<div class="footer">Generated by Gaply DataMaestro Pro — ${new Date().toLocaleString()}</div></body></html>`;

    const blob = new Blob([html], { type: 'text/html' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `${title.replace(/[^a-zA-Z0-9]/g, '_')}_${type}_report.html`;
    a.click();
    URL.revokeObjectURL(a.href);
  };

  // --- Render helpers ---
  const renderChart = (fig: any, idx: number) => {
    if (!fig?.data) return null;
    const { labels, datasets } = fig.data;
    if (!labels || !datasets?.[0]?.data) return null;
    const chartData = labels.map((l: string, i: number) => {
      const point: any = { name: l };
      datasets.forEach((ds: any, di: number) => { point[ds.label || `Series ${di + 1}`] = ds.data[i]; });
      return point;
    });

    if (fig.type === 'pie') {
      const pieData = labels.map((l: string, i: number) => ({ name: l, value: datasets[0].data[i] }));
      return (
        <div className="dm-chart-wrap" key={idx}>
          <div className="dm-chart-title">{fig.title}</div>
          <ResponsiveContainer width="100%" height={300}>
            <PieChart>
              <Pie data={pieData} dataKey="value" nameKey="name" cx="50%" cy="50%" outerRadius={100} label>
                {pieData.map((_: any, i: number) => <Cell key={i} fill={CHART_COLORS[i % CHART_COLORS.length]} />)}
              </Pie>
              <Tooltip contentStyle={{ background: '#1e1e2e', border: '1px solid #333', borderRadius: 8, color: '#fff' }} />
              <Legend />
            </PieChart>
          </ResponsiveContainer>
          {fig.description && <p style={{ fontSize: 13, color: 'rgba(255,255,255,0.5)', marginTop: 8 }}>{fig.description}</p>}
        </div>
      );
    }

    return (
      <div className="dm-chart-wrap" key={idx}>
        <div className="dm-chart-title">{fig.title}</div>
        <ResponsiveContainer width="100%" height={300}>
          <BarChart data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.06)" />
            <XAxis dataKey="name" tick={{ fill: 'rgba(255,255,255,0.5)', fontSize: 12 }} />
            <YAxis tick={{ fill: 'rgba(255,255,255,0.5)', fontSize: 12 }} />
            <Tooltip contentStyle={{ background: '#1e1e2e', border: '1px solid #333', borderRadius: 8, color: '#fff' }} />
            <Legend />
            {datasets.map((ds: any, di: number) => (
              <Bar key={di} dataKey={ds.label || `Series ${di + 1}`} fill={CHART_COLORS[di % CHART_COLORS.length]} radius={[4, 4, 0, 0]} />
            ))}
          </BarChart>
        </ResponsiveContainer>
        {fig.description && <p style={{ fontSize: 13, color: 'rgba(255,255,255,0.5)', marginTop: 8 }}>{fig.description}</p>}
      </div>
    );
  };

  const stepDots = [1, 2, 3, 4];

  return (
    <div className="dm-pro-page">
      <SEO title="DataMaestro Pro | Gaply" description="AI-powered statistical analysis for academic research" keywords="statistical analysis, data analysis, research, SPSS, DataMaestro" />

      <div className="dm-steps">
        {stepDots.map(s => (
          <div key={s} className={`dm-step-dot ${step === s ? 'active' : step > s ? 'done' : ''}`} />
        ))}
      </div>

      {error && <div className="dm-error" style={{ maxWidth: 960, margin: '16px auto' }}>⚠️ {error} <button onClick={() => setError(null)} style={{ marginLeft: 'auto', background: 'none', border: 'none', color: '#fca5a5', cursor: 'pointer', fontSize: 18 }}>×</button></div>}

      {/* === STEP 1: Hero + Research Setup === */}
      {step === 1 && !loading && (
        <div className="dm-hero">
          <div className="dm-hero-bg" />
          <div className="dm-hero-content">
            <div className="dm-badge">Pro Feature</div>
            <h1>DataMaestro<br />Pro</h1>
            <p>Upload your research data, and our AI will perform comprehensive statistical analysis — producing publication-ready tables, charts, interpretations, and downloadable reports.</p>
            <button className="dm-btn dm-btn-primary" onClick={() => setStep(1.5 as any)}>
              Start Analysis <span>→</span>
            </button>
          </div>
        </div>
      )}

      {(step === 1.5 || (step === 1 && loading)) && (
        <div className="dm-form-container">
          <h1 className="dm-form-title">Research Setup</h1>
          <p className="dm-form-subtitle">Tell us about your research. Our AI will design the optimal analysis plan.</p>

          <div className="dm-card">
            <h3><span className="icon">📝</span> Research Title</h3>
            <p className="dm-label">Enter your research title</p>
            <input className="dm-input" placeholder="e.g., Impact of Social Media on Student Academic Performance" value={title} onChange={e => setTitle(e.target.value)} />
          </div>

          <div className="dm-card">
            <h3><span className="icon">🎯</span> Research Objectives</h3>
            <p className="dm-label">What do you aim to achieve?</p>
            {objectives.map((o, i) => (
              <div className="dm-list-item" key={i}>
                <input className="dm-input" placeholder={`Objective ${i + 1}`} value={o} onChange={e => { const n = [...objectives]; n[i] = e.target.value; setObjectives(n); }} />
                {objectives.length > 1 && <button className="dm-remove-btn" onClick={() => setObjectives(objectives.filter((_, j) => j !== i))}>×</button>}
              </div>
            ))}
            <button className="dm-add-btn" onClick={() => setObjectives([...objectives, ''])}>+ Add Objective</button>
          </div>

          <div className="dm-card">
            <h3><span className="icon">🔬</span> Research Methodology</h3>
            <p className="dm-label">Describe your research design (optional)</p>
            <textarea className="dm-textarea" placeholder="e.g., Quantitative survey-based study with Likert scale questionnaire..." value={methodology} onChange={e => setMethodology(e.target.value)} />
          </div>

          <div className="dm-card">
            <h3><span className="icon">🏷️</span> Research Area</h3>
            <p className="dm-label">Field of study (optional)</p>
            <input className="dm-input" placeholder="e.g., Education, Psychology, Business..." value={researchArea} onChange={e => setResearchArea(e.target.value)} />
          </div>

          <div className="dm-footer-actions">
            <button className="dm-btn dm-btn-secondary" onClick={() => setStep(1)}>← Back</button>
            <button className="dm-btn dm-btn-primary" onClick={handleSetup} disabled={loading}>
              {loading ? 'Analyzing...' : 'Generate Analysis Plan →'}
            </button>
          </div>
        </div>
      )}

      {/* === STEP 1 Loading === */}
      {step === 1 && loading && (
        <div className="dm-loading">
          <div className="dm-loading-ring" />
          <h2>Analyzing Research Context</h2>
          <p>{progressMsg}</p>
          <div className="dm-progress-bar"><div className="dm-progress-fill" style={{ width: `${progress}%` }} /></div>
        </div>
      )}

      {/* === STEP 2: AI Config + Upload === */}
      {step === 2 && (
        <div className="dm-form-container">
          <h1 className="dm-form-title">Analysis Configuration</h1>
          <p className="dm-form-subtitle">AI has analyzed your research. Review the suggestions, upload your data, and configure the analysis.</p>

          {/* AI Suggestions */}
          {aiSetup && (
            <>
              {aiSetup.methodology_assessment && (
                <div className="dm-ai-card">
                  <h4>🤖 AI Assessment</h4>
                  <p style={{ color: 'rgba(255,255,255,0.7)', fontSize: 14 }}>
                    <strong>Design:</strong> {aiSetup.methodology_assessment.design_type}
                  </p>
                  {aiSetup.methodology_assessment.suggestions?.map((s: string, i: number) => (
                    <div className="dm-ai-tag recommended" key={i}>{s}</div>
                  ))}
                </div>
              )}

              {aiSetup.recommended_tests && (
                <div className="dm-ai-card">
                  <h4>📋 Recommended Tests</h4>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                    {(aiSetup.recommended_tests as RecommendedTest[]).map((t, i) => (
                      <div className={`dm-ai-tag ${t.priority}`} key={i} title={t.reason}>{t.test_name}</div>
                    ))}
                  </div>
                </div>
              )}
            </>
          )}

          {/* Hypotheses */}
          <div className="dm-card">
            <h3><span className="icon">💡</span> Hypotheses</h3>
            <p className="dm-label">AI-suggested hypotheses. Edit or add your own.</p>
            {hypotheses.map((h, i) => (
              <div className="dm-list-item" key={i}>
                <input className="dm-input" value={h.text} onChange={e => { const n = [...hypotheses]; n[i] = { ...n[i], text: e.target.value }; setHypotheses(n); }} />
                <button className="dm-remove-btn" onClick={() => setHypotheses(hypotheses.filter((_, j) => j !== i))}>×</button>
              </div>
            ))}
            <button className="dm-add-btn" onClick={() => setHypotheses([...hypotheses, { id: `H${hypotheses.length + 1}`, text: '', type: 'directional', test_suggestion: '' }])}>+ Add Hypothesis</button>
          </div>

          {/* Variables */}
          <div className="dm-card">
            <h3><span className="icon">📊</span> Variables</h3>
            <p className="dm-label">Define your research variables</p>
            {variables.map((v, i) => (
              <div className="dm-var-row" key={i}>
                <input className="dm-input" placeholder="Variable name" value={v.name} onChange={e => { const n = [...variables]; n[i] = { ...n[i], name: e.target.value }; setVariables(n); }} />
                <select className="dm-select" value={v.type} onChange={e => { const n = [...variables]; n[i] = { ...n[i], type: e.target.value }; setVariables(n); }}>
                  <option value="nominal">Nominal</option>
                  <option value="ordinal">Ordinal</option>
                  <option value="interval">Interval</option>
                  <option value="ratio">Ratio</option>
                </select>
                <select className="dm-select" value={v.role} onChange={e => { const n = [...variables]; n[i] = { ...n[i], role: e.target.value }; setVariables(n); }}>
                  <option value="iv">Independent</option>
                  <option value="dv">Dependent</option>
                  <option value="covariate">Covariate</option>
                  <option value="mediator">Mediator</option>
                  <option value="moderator">Moderator</option>
                </select>
                <button className="dm-remove-btn" onClick={() => setVariables(variables.filter((_, j) => j !== i))}>×</button>
              </div>
            ))}
            <button className="dm-add-btn" onClick={() => setVariables([...variables, { name: '', type: 'nominal', role: 'iv', description: '' }])}>+ Add Variable</button>
          </div>

          {/* Data Upload */}
          <div className="dm-card">
            <h3><span className="icon">📂</span> Upload Dataset</h3>
            <p className="dm-label">CSV, Excel, PDF, DOCX, or text file</p>
            <div className={`dm-upload-zone ${isDragging ? 'dragging' : ''}`}
              onClick={() => fileInputRef.current?.click()}
              onDragOver={e => { e.preventDefault(); setIsDragging(true); }}
              onDragLeave={() => setIsDragging(false)}
              onDrop={onDrop}>
              <div className="upload-icon">📤</div>
              <p><strong>Drop your file here</strong> or click to browse</p>
              <p style={{ fontSize: 13 }}>Supports CSV, XLS, XLSX, PDF, DOCX, TXT, JSON</p>
            </div>
            <input ref={fileInputRef} type="file" accept=".csv,.xls,.xlsx,.tsv,.txt,.json,.pdf,.docx" style={{ display: 'none' }}
              onChange={e => { if (e.target.files?.[0]) handleFile(e.target.files[0]); }} />
            {uploadedFile && (
              <div className="dm-file-info">
                <span className="file-icon">📄</span>
                <span className="file-name">{uploadedFile.name}</span>
                <span className="file-size">{(uploadedFile.size / 1024).toFixed(1)} KB</span>
                {parsedData?.n_rows > 0 && <span style={{ color: '#86efac', fontSize: 13 }}>✓ {parsedData.n_rows} rows × {parsedData.n_columns} cols</span>}
              </div>
            )}
          </div>

          {/* Questionnaire */}
          <div className="dm-card">
            <h3><span className="icon">📋</span> Questionnaire / Instrument</h3>
            <p className="dm-label">Paste your questionnaire or survey instrument (optional)</p>
            <textarea className="dm-textarea" placeholder="Paste questionnaire items, scales, or instrument description..." value={questionnaireText} onChange={e => setQuestionnaireText(e.target.value)} style={{ minHeight: 80 }} />
          </div>

          {/* Test Modules */}
          <div className="dm-card">
            <h3><span className="icon">🧪</span> Analysis Modules</h3>
            <div className="dm-toggle-row">
              <span style={{ fontSize: 14, color: 'rgba(255,255,255,0.6)' }}>Auto-select optimal tests based on your data</span>
              <button className={`dm-toggle ${autoSelect ? 'on' : ''}`} onClick={() => setAutoSelect(!autoSelect)} />
            </div>
            {!autoSelect && (
              <div className="dm-module-grid">
                {ANALYSIS_MODULES.map(m => (
                  <div key={m.id} className={`dm-module-chip ${selectedModules.includes(m.id) ? 'selected' : ''}`}
                    onClick={() => setSelectedModules(prev => prev.includes(m.id) ? prev.filter(x => x !== m.id) : [...prev, m.id])}>
                    <div className="dm-module-check">{selectedModules.includes(m.id) ? '✓' : ''}</div>
                    <span>{m.icon} {m.label}</span>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="dm-footer-actions">
            <button className="dm-btn dm-btn-secondary" onClick={() => setStep(1.5 as any)}>← Back to Setup</button>
            <button className="dm-btn dm-btn-primary" onClick={handleAnalyze}>
              Run Analysis →
            </button>
          </div>
        </div>
      )}

      {/* === STEP 3: Loading === */}
      {step === 3 && (
        <div className="dm-loading">
          <div className="dm-loading-ring" />
          <h2>Performing Analysis</h2>
          <p>{progressMsg}</p>
          <div className="dm-progress-bar"><div className="dm-progress-fill" style={{ width: `${progress}%` }} /></div>
        </div>
      )}

      {/* === STEP 4: Results === */}
      {step === 4 && analysisResult && (
        <div className="dm-results">
          <div className="dm-results-header">
            <div>
              <h1>Analysis Results</h1>
              <p style={{ color: 'rgba(255,255,255,0.5)', fontSize: 14, margin: 0 }}>{title}</p>
            </div>
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="dm-btn dm-btn-secondary dm-btn-sm" onClick={() => downloadReport('results')}>📥 Results Report</button>
              <button className="dm-btn dm-btn-secondary dm-btn-sm" onClick={() => downloadReport('methodology')}>📋 Methods Report</button>
              <button className="dm-btn dm-btn-secondary dm-btn-sm" onClick={() => { setStep(1); setAnalysisResult(null); }}>🔄 New Analysis</button>
            </div>
          </div>

          <div className="dm-tabs">
            {(['summary', 'results', 'methodology', 'findings', 'chat'] as const).map(t => (
              <button key={t} className={`dm-tab ${resultTab === t ? 'active' : ''}`} onClick={() => setResultTab(t)}>
                {t === 'summary' ? '📊 Summary' : t === 'results' ? '📈 Results' : t === 'methodology' ? '📋 Methods' : t === 'findings' ? '💡 Findings' : '💬 Chat'}
              </button>
            ))}
          </div>

          {/* Summary Tab */}
          {resultTab === 'summary' && (
            <>
              {analysisResult.executive_summary && (
                <div className="dm-section">
                  <h2>📊 Executive Summary</h2>
                  <p>{analysisResult.executive_summary}</p>
                </div>
              )}
              {analysisResult.hypothesis_results && (
                <div className="dm-section">
                  <h2>🔬 Hypothesis Results</h2>
                  {analysisResult.hypothesis_results.map((h: any, i: number) => (
                    <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 0', borderBottom: '1px solid rgba(255,255,255,0.04)' }}>
                      <span className={`dm-hypothesis-badge ${h.status?.toLowerCase().includes('not') ? 'not-supported' : h.status?.toLowerCase().includes('partial') ? 'partial' : 'supported'}`}>
                        {h.status?.toLowerCase().includes('not') ? '✗' : h.status?.toLowerCase().includes('partial') ? '~' : '✓'} {h.status}
                      </span>
                      <span style={{ flex: 1, fontSize: 14 }}>{h.hypothesis}</span>
                      {h.p_value != null && <span style={{ fontSize: 13, color: 'rgba(255,255,255,0.4)' }}>p = {Number(h.p_value).toFixed(3)}</span>}
                    </div>
                  ))}
                </div>
              )}
              {analysisResult.descriptive_statistics?.table_html && (
                <div className="dm-section">
                  <h2>📋 Descriptive Statistics</h2>
                  <div dangerouslySetInnerHTML={{ __html: analysisResult.descriptive_statistics.table_html }} />
                </div>
              )}
            </>
          )}

          {/* Results Tab */}
          {resultTab === 'results' && (
            <>
              {analysisResult.results_chapter?.introduction && (
                <div className="dm-section">
                  <h2>📝 Results</h2>
                  <p>{analysisResult.results_chapter.introduction}</p>
                </div>
              )}
              {analysisResult.results_chapter?.sections?.map((s: any, si: number) => (
                <div className="dm-section" key={si}>
                  <h2>{s.heading}</h2>
                  <div dangerouslySetInnerHTML={{ __html: s.content }} />
                  {s.tables?.map((t: string, ti: number) => <div key={ti} dangerouslySetInnerHTML={{ __html: t }} />)}
                  {s.figures?.map((f: any, fi: number) => renderChart(f, fi))}
                </div>
              ))}
              {analysisResult.analyses_performed?.map((a: any, i: number) => (
                <div className="dm-section" key={i}>
                  <h2>{a.test_name}</h2>
                  <p style={{ fontSize: 13, color: '#a5b4fc' }}>{a.category} • {a.why_this_test}</p>
                  {a.result_table_html && <div dangerouslySetInnerHTML={{ __html: a.result_table_html }} />}
                  {a.interpretation && <p><strong>Interpretation:</strong> {a.interpretation}</p>}
                  {a.finding_paragraph && <div style={{ background: 'rgba(59,130,246,0.06)', padding: 16, borderRadius: 12, borderLeft: '3px solid #3b82f6', marginTop: 12 }}><p style={{ margin: 0 }}>{a.finding_paragraph}</p></div>}
                </div>
              ))}
              {analysisResult.results_chapter?.conclusion && (
                <div className="dm-section">
                  <h2>Conclusion</h2>
                  <p>{analysisResult.results_chapter.conclusion}</p>
                </div>
              )}
            </>
          )}

          {/* Methodology Tab */}
          {resultTab === 'methodology' && analysisResult.methodology_report?.sections?.map((s: any, i: number) => (
            <div className="dm-section" key={i}>
              <h2>{s.heading}</h2>
              <div dangerouslySetInnerHTML={{ __html: s.content }} />
              {s.table_html && <div dangerouslySetInnerHTML={{ __html: s.table_html }} />}
            </div>
          ))}

          {/* Findings Tab */}
          {resultTab === 'findings' && analysisResult.findings_summary && (
            <>
              <div className="dm-section">
                <h2>💡 Key Findings</h2>
                <ul>{analysisResult.findings_summary.key_findings?.map((f: string, i: number) => <li key={i}>{f}</li>)}</ul>
              </div>
              <div className="dm-section">
                <h2>🎯 Practical Implications</h2>
                <ul>{analysisResult.findings_summary.practical_implications?.map((f: string, i: number) => <li key={i}>{f}</li>)}</ul>
              </div>
              <div className="dm-section">
                <h2>⚠️ Limitations</h2>
                <ul>{analysisResult.findings_summary.limitations?.map((f: string, i: number) => <li key={i}>{f}</li>)}</ul>
              </div>
              <div className="dm-section">
                <h2>🔮 Future Research</h2>
                <ul>{analysisResult.findings_summary.future_research?.map((f: string, i: number) => <li key={i}>{f}</li>)}</ul>
              </div>
            </>
          )}

          {/* Chat Tab */}
          {resultTab === 'chat' && (
            <div className="dm-chat">
              <div className="dm-chat-messages">
                {chatMessages.length === 0 && (
                  <div style={{ textAlign: 'center', padding: 40, color: 'rgba(255,255,255,0.3)' }}>
                    <div style={{ fontSize: 48, marginBottom: 12 }}>💬</div>
                    <p>Ask Gaply anything about your analysis results</p>
                  </div>
                )}
                {chatMessages.map((m, i) => (
                  <div key={i} className={`dm-chat-msg ${m.role}`}>
                    {m.role === 'assistant' ? <div dangerouslySetInnerHTML={{ __html: m.content.replace(/\n/g, '<br/>') }} /> : m.content}
                  </div>
                ))}
                {chatLoading && <div className="dm-chat-msg assistant" style={{ opacity: 0.6 }}>Thinking...</div>}
              </div>
              <div className="dm-chat-input-row">
                <input placeholder="Ask about your analysis..." value={chatInput}
                  onChange={e => setChatInput(e.target.value)}
                  onKeyDown={e => { if (e.key === 'Enter') sendChat(); }} />
                <button className="dm-btn dm-btn-primary dm-btn-sm" onClick={sendChat} disabled={chatLoading}>Send</button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default DataMaestroProPage;
