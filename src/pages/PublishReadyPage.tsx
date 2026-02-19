import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  FileText,
  Layers,
  Scale,
  Percent,
  Edit3,
  BookOpen,
  Search,
  MessageCircle,
  CheckCircle2,
  Sparkles,
} from 'lucide-react';
import DashboardLayout from '../components/Layout/DashboardLayout';
import { fetchDashboardOverview } from '../services/dashboardService';
import { useAuth } from '../contexts/AuthContext';
import './PublishReadyPage.css';

const FEATURES = [
  {
    icon: FileText,
    title: 'Document Upload',
    description: 'Upload PDF, DOCX, or TXT. Paste text or provide a manuscript link. Full manuscript support up to 1,000,000 characters.',
  },
  {
    icon: Layers,
    title: 'Chunk-by-Chunk Analysis',
    description: 'Intelligent paragraph-first, sentence-fallback segmentation. Multi-task evaluation per chunk with guideline, novelty, plagiarism, and AI-use checks.',
  },
  {
    icon: Scale,
    title: 'Referee-Style Review',
    description: 'Methodology rubric, evidence table, cross-section consistency, citation freshness, section scores, and acceptance probability with detailed reasoning.',
  },
  {
    icon: Percent,
    title: 'Publication Chance',
    description: 'AI-powered probability estimation with what/why/how reasoning to gauge readiness for your target journal.',
  },
  {
    icon: Edit3,
    title: 'Line-by-Line Review',
    description: 'Precise edit suggestions with offsets and explanations. Segment-level summaries for actionable improvements.',
  },
  {
    icon: BookOpen,
    title: 'Journal Guidelines',
    description: 'Automatic extraction and structuring of journal guidelines from HTML/PDF. Reference style, abstract limits, structure, and table requirements.',
  },
  {
    icon: Search,
    title: 'Retrieved Documents',
    description: 'Integration with OpenAlex, CrossRef, and Semantic Scholar. Relevant literature retrieved automatically for novelty and plagiarism context.',
  },
  {
    icon: MessageCircle,
    title: 'Chat with AI',
    description: 'Interactive assistance with manuscript context. Ask questions and get guidance on methodology, clarity, and structure.',
  },
];

const ANALYSIS_TASKS = [
  { name: 'Guideline check', desc: 'Title/abstract, reference style, section structure, word limits' },
  { name: 'Novelty check', desc: 'Originality vs retrieved literature, semantic similarity' },
  { name: 'Plagiarism check', desc: 'Exact phrase matches, paraphrase detection, score 0–1' },
  { name: 'AI use detection', desc: 'Stylometric analysis, repetition, generic phrasing' },
  { name: 'Suggest edits', desc: 'Line-by-line improvements with location and effort estimates' },
];

const PublishReadyPage: React.FC = () => {
  const { isAuthenticated, user, token } = useAuth();
  const navigate = useNavigate();
  const [usage, setUsage] = useState<string>('0/0');
  const [loadingUsage, setLoadingUsage] = useState(true);

  useEffect(() => {
    if (!isAuthenticated || !user) {
      navigate('/login');
    }
  }, [isAuthenticated, user, navigate]);

  useEffect(() => {
    if (!token) {
      setLoadingUsage(false);
      return;
    }
    fetchDashboardOverview(token)
      .then((o) => {
        const s = o?.stats;
        setUsage(s ? `${s.publishready_used}/${s.publishready_total}` : '0/0');
      })
      .catch(() => setUsage('0/0'))
      .finally(() => setLoadingUsage(false));
  }, [token]);

  if (!isAuthenticated || !user) {
    return null;
  }

  return (
    <DashboardLayout pageTitle="PublishReady Pro">
      <div className="publishready-page">
        {/* Hero */}
        <section className="publishready-hero">
          <div className="publishready-hero-badge">
            <Sparkles size={16} />
            <span>Pro</span>
          </div>
          <h1 className="publishready-hero-title">PublishReady Pro</h1>
          <p className="publishready-hero-tagline">
            Advanced AI-powered manuscript analysis — referee-grade evaluation, publication probability, and structured feedback.
          </p>
          <div className="publishready-hero-actions">
            <button
              type="button"
              className="publishready-cta"
              onClick={() => navigate('/final-orchestrator')}
            >
              Analyze manuscript
            </button>
            <div className="publishready-usage">
              {loadingUsage ? (
                <span className="publishready-usage-muted">Loading usage…</span>
              ) : (
                <>
                  <span className="publishready-usage-label">PublishReady uses</span>
                  <span className="publishready-usage-value">{usage}</span>
                </>
              )}
            </div>
          </div>
        </section>

        {/* Core features grid */}
        <section className="publishready-section">
          <h2 className="publishready-section-title">Features</h2>
          <p className="publishready-section-desc">
            Powered by the Document Analysis Orchestrator — production-ready, ChatGPT-quality evaluation.
          </p>
          <div className="publishready-features-grid">
            {FEATURES.map((f) => {
              const Icon = f.icon;
              return (
                <div key={f.title} className="publishready-feature-card">
                  <div className="publishready-feature-icon">
                    <Icon size={22} />
                  </div>
                  <h3 className="publishready-feature-title">{f.title}</h3>
                  <p className="publishready-feature-desc">{f.description}</p>
                </div>
              );
            })}
          </div>
        </section>

        {/* Analysis tasks */}
        <section className="publishready-section">
          <h2 className="publishready-section-title">Analysis tasks</h2>
          <p className="publishready-section-desc">
            Each chunk is evaluated across these dimensions for consistent, schema-stable JSON output.
          </p>
          <ul className="publishready-tasks-list">
            {ANALYSIS_TASKS.map((t) => (
              <li key={t.name} className="publishready-task-item">
                <CheckCircle2 size={18} className="publishready-task-icon" />
                <div>
                  <strong>{t.name}</strong> — {t.desc}
                </div>
              </li>
            ))}
          </ul>
        </section>

        {/* CTA bottom */}
        <section className="publishready-section publishready-cta-section">
          <p className="publishready-cta-text">Upload your manuscript and get referee-style feedback in minutes.</p>
          <button
            type="button"
            className="publishready-cta publishready-cta-secondary"
            onClick={() => navigate('/final-orchestrator')}
          >
            Open PublishReady
          </button>
        </section>
      </div>
    </DashboardLayout>
  );
};

export default PublishReadyPage;
