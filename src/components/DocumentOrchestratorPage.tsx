import React, { useMemo, useState } from 'react';
import SEO from './SEO';
import { apiFetch } from '../api/config';
import './DocumentOrchestratorPage.css';

const TASK_OPTIONS = [
  { id: 'guideline_check', label: 'Guideline Check' },
  { id: 'reference_check', label: 'Reference Check' },
  { id: 'novelty_check', label: 'Novelty Check' },
  { id: 'plagiarism_check', label: 'Plagiarism Check' },
  { id: 'ai_use_detection', label: 'AI-Use Detection' },
  { id: 'suggest_edits', label: 'Suggest Edits' },
  { id: 'extract_entities', label: 'Extract Entities' },
];

const DEFAULT_GUIDELINES = `{
  "title_requirements": "12-20 words",
  "abstract_max_words": 250,
  "structure_order": ["Abstract", "Introduction", "Methods", "Results", "Discussion"],
  "reference_style": "APA",
  "figure_table_rules": "300dpi",
  "ethics_disclosure": "IRB approval required",
  "supplementary_requirements": "Data availability statement"
}`;

const DEFAULT_RETRIEVED_DOCS = `[
  {
    "id": "doc-1",
    "source": "semantic_scholar",
    "title": "Clinical Summarization with Transformers",
    "snippet": "Transformer models improved ROUGE-L by 12% on discharge notes but struggled on cardiology notes due to domain shift.",
    "similarity_score": 0.81
  }
]`;

const DocumentOrchestratorPage: React.FC = () => {
  const [jobId, setJobId] = useState('job-001');
  const [chunkId, setChunkId] = useState('chunk-001');
  const [chunkPosition, setChunkPosition] = useState(1);
  const [chunkTokenEstimate, setChunkTokenEstimate] = useState(450);
  const [chunkText, setChunkText] = useState(
    'The study evaluates transformer-based summarization for clinical notes across 12 hospitals. We report ROUGE-L improvements of 14% but note inconsistent performance across specialties, suggesting a domain adaptation gap.'
  );
  const [journalGuidelines, setJournalGuidelines] = useState(DEFAULT_GUIDELINES);
  const [retrievedDocs, setRetrievedDocs] = useState(DEFAULT_RETRIEVED_DOCS);
  const [selectedTasks, setSelectedTasks] = useState<string[]>(
    TASK_OPTIONS.filter((task) => task.id !== 'reference_check' && task.id !== 'extract_entities').map((task) => task.id)
  );
  const [maxTokens, setMaxTokens] = useState(1200);

  const [email, setEmail] = useState('demo@gaply.in');
  const [journalName, setJournalName] = useState('Target Journal');
  const [journalUrl, setJournalUrl] = useState('https://example.com/journal');
  const [manuscriptSummary, setManuscriptSummary] = useState('Concise summary of the manuscript for publication chance estimation.');
  const [useChunkAsSummary, setUseChunkAsSummary] = useState(true);

  const [status, setStatus] = useState<'idle' | 'running' | 'error' | 'success'>('idle');
  const [statusMeta, setStatusMeta] = useState('Awaiting input.');
  const [analysisResponse, setAnalysisResponse] = useState<string>('Awaiting request...');
  const [publicationResponse, setPublicationResponse] = useState<string>('Not requested yet.');

  const derivedSummary = useMemo(() => {
    return useChunkAsSummary ? chunkText : manuscriptSummary;
  }, [useChunkAsSummary, chunkText, manuscriptSummary]);

  const toggleTask = (taskId: string) => {
    setSelectedTasks((prev) =>
      prev.includes(taskId) ? prev.filter((id) => id !== taskId) : [...prev, taskId]
    );
  };

  const parseJson = (value: string, fallback: any, label: string) => {
    const trimmed = value.trim();
    if (!trimmed) return fallback;
    try {
      return JSON.parse(trimmed);
    } catch (err) {
      throw new Error(`${label} JSON is invalid.`);
    }
  };

  const handleRun = async () => {
    setStatus('running');
    setStatusMeta('Sending chunk to orchestrator...');
    setAnalysisResponse('Running analysis...');
    setPublicationResponse('Waiting for main analysis...');

    const start = performance.now();

    try {
      const guidelinesObj = parseJson(journalGuidelines, null, 'Journal guidelines');
      const retrievedDocsObj = parseJson(retrievedDocs, [], 'Retrieved docs');

      const payload = {
        job_id: jobId,
        chunk_id: chunkId,
        chunk_text: chunkText,
        chunk_position: Number(chunkPosition),
        chunk_token_estimate: Number(chunkTokenEstimate),
        journal_guidelines: guidelinesObj,
        retrieved_docs: retrievedDocsObj,
        tasks: selectedTasks,
        max_tokens_for_response: Number(maxTokens),
      };

      const orchestratorRes = await apiFetch('/api/ai/document-orchestrator', {
        method: 'POST',
        body: JSON.stringify(payload),
      });
      const orchestratorText = await orchestratorRes.text();

      let orchestratorJson: any;
      try {
        orchestratorJson = JSON.parse(orchestratorText);
        setAnalysisResponse(JSON.stringify(orchestratorJson, null, 2));
      } catch (err) {
        setAnalysisResponse(orchestratorText);
        throw new Error('Orchestrator response is not valid JSON.');
      }

      setStatusMeta('Estimating publication chance...');

      const publicationPayload = {
        email,
        journal_url: journalUrl,
        journal_name: journalName,
        manuscript_summary: derivedSummary,
        journal_guidelines: guidelinesObj,
      };

      const publicationRes = await apiFetch('/api/ai/publication-chance', {
        method: 'POST',
        body: JSON.stringify(publicationPayload),
      });
      const publicationText = await publicationRes.text();

      try {
        const publicationJson = JSON.parse(publicationText);
        setPublicationResponse(JSON.stringify(publicationJson, null, 2));
      } catch (err) {
        setPublicationResponse(publicationText);
      }

      const elapsed = Math.round(performance.now() - start);
      setStatus('success');
      setStatusMeta(`Completed in ${elapsed}ms · HTTP ${orchestratorRes.status}`);
    } catch (err: any) {
      setStatus('error');
      setStatusMeta(err?.message || 'Request failed.');
    }
  };

  return (
    <div className="orchestrator-page">
      <SEO
        title="Document Analysis Orchestrator | Gaply"
        description="Test the Document Analysis Orchestrator with journal guidelines, retrieval evidence, and publication chance estimation."
        keywords="document analysis orchestrator, journal guideline compliance, publication chance estimator, academic AI analysis"
      />

      <section className="orchestrator-hero">
        <div className="hero-content">
          <p className="eyebrow">Final Analysis Suite</p>
          <h1>Document Analysis Orchestrator</h1>
          <p className="hero-subtitle">
            Run chunk-level analysis with strict JSON output and immediately estimate the chance of
            publication for your target journal.
          </p>
          <div className="hero-actions">
            <button className="primary" onClick={handleRun} disabled={status === 'running'}>
              {status === 'running' ? 'Running...' : 'Run Full Analysis'}
            </button>
            <button
              className="secondary"
              onClick={() => {
                setJournalGuidelines(DEFAULT_GUIDELINES);
                setRetrievedDocs(DEFAULT_RETRIEVED_DOCS);
                setStatus('idle');
                setStatusMeta('Sample data loaded.');
              }}
            >
              Load Sample Data
            </button>
          </div>
          <div className={`status-pill ${status}`}>
            <span>{status.toUpperCase()}</span>
            <small>{statusMeta}</small>
          </div>
        </div>
        <div className="hero-glow" />
      </section>

      <section className="orchestrator-grid">
        <div className="panel">
          <h2>Chunk Request Builder</h2>
          <div className="grid-two">
            <div className="field">
              <label>Job ID</label>
              <input value={jobId} onChange={(e) => setJobId(e.target.value)} />
            </div>
            <div className="field">
              <label>Chunk ID</label>
              <input value={chunkId} onChange={(e) => setChunkId(e.target.value)} />
            </div>
          </div>
          <div className="grid-two">
            <div className="field">
              <label>Chunk Position</label>
              <input type="number" value={chunkPosition} onChange={(e) => setChunkPosition(Number(e.target.value))} />
            </div>
            <div className="field">
              <label>Chunk Token Estimate</label>
              <input type="number" value={chunkTokenEstimate} onChange={(e) => setChunkTokenEstimate(Number(e.target.value))} />
            </div>
          </div>
          <div className="field">
            <label>Chunk Text</label>
            <textarea value={chunkText} onChange={(e) => setChunkText(e.target.value)} />
          </div>
          <div className="field">
            <label>Journal Guidelines (JSON or empty)</label>
            <textarea value={journalGuidelines} onChange={(e) => setJournalGuidelines(e.target.value)} />
          </div>
          <div className="field">
            <label>Retrieved Docs (JSON array)</label>
            <textarea value={retrievedDocs} onChange={(e) => setRetrievedDocs(e.target.value)} />
          </div>
          <div className="field">
            <label>Tasks</label>
            <div className="task-grid">
              {TASK_OPTIONS.map((task) => (
                <button
                  key={task.id}
                  type="button"
                  className={`task-chip ${selectedTasks.includes(task.id) ? 'selected' : ''}`}
                  onClick={() => toggleTask(task.id)}
                >
                  {task.label}
                </button>
              ))}
            </div>
          </div>
          <div className="field">
            <label>Max Tokens for Response</label>
            <input type="number" value={maxTokens} onChange={(e) => setMaxTokens(Number(e.target.value))} />
          </div>
        </div>

        <div className="panel">
          <h2>Publication Chance</h2>
          <div className="grid-two">
            <div className="field">
              <label>Email (premium access)</label>
              <input value={email} onChange={(e) => setEmail(e.target.value)} />
            </div>
            <div className="field">
              <label>Journal Name</label>
              <input value={journalName} onChange={(e) => setJournalName(e.target.value)} />
            </div>
          </div>
          <div className="field">
            <label>Journal Website Link</label>
            <input value={journalUrl} onChange={(e) => setJournalUrl(e.target.value)} />
          </div>
          <div className="field">
            <label>Manuscript Summary</label>
            <textarea value={manuscriptSummary} onChange={(e) => setManuscriptSummary(e.target.value)} />
          </div>
          <div className="field inline">
            <input
              type="checkbox"
              checked={useChunkAsSummary}
              onChange={(e) => setUseChunkAsSummary(e.target.checked)}
            />
            <span>Use chunk text as summary</span>
          </div>
          <div className="hint">
            Summary sent: <strong>{derivedSummary.length} chars</strong>
          </div>
        </div>
      </section>

      <section className="orchestrator-grid">
        <div className="panel response">
          <h2>Model Response</h2>
          <p className="subtitle">Strict JSON output only. Parsed response will be formatted when valid.</p>
          <pre>{analysisResponse}</pre>
        </div>
        <div className="panel response">
          <h2>Publication Chance Output</h2>
          <p className="subtitle">Includes publication chance and the journal website link.</p>
          <pre>{publicationResponse}</pre>
        </div>
      </section>
    </div>
  );
};

export default DocumentOrchestratorPage;
