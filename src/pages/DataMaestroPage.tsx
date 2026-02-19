import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  BarChart2,
  Upload,
  TestTube,
  FileText,
  Sparkles,
} from 'lucide-react';
import DashboardLayout from '../components/Layout/DashboardLayout';
import { fetchDashboardOverview } from '../services/dashboardService';
import { useAuth } from '../contexts/AuthContext';
import './DataMaestroPage.css';

const FEATURES = [
  {
    icon: Upload,
    title: 'Dataset Upload',
    description: 'Upload CSV, Excel, PDF, or DOCX. Multiple file support for comprehensive analysis.',
  },
  {
    icon: TestTube,
    title: 'Test Recommendations',
    description: 'AI-powered statistical test suggestions based on your research design, variables, and objectives.',
  },
  {
    icon: FileText,
    title: 'HTML Reports',
    description: 'Publication-ready statistical analysis reports with tables, interpretations, and methodology notes.',
  },
  {
    icon: BarChart2,
    title: 'Interpretations',
    description: 'Clear explanations of test results, assumptions, and how to report findings in your paper.',
  },
];

const DataMaestroPage: React.FC = () => {
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
        setUsage(s ? `${s.datamaestro_used}/${s.datamaestro_total}` : '0/0');
      })
      .catch(() => setUsage('0/0'))
      .finally(() => setLoadingUsage(false));
  }, [token]);

  if (!isAuthenticated || !user) {
    return null;
  }

  return (
    <DashboardLayout pageTitle="DataMaestro Pro">
      <div className="datamaestro-page">
        {/* Hero */}
        <section className="datamaestro-hero">
          <div className="datamaestro-hero-badge">
            <Sparkles size={16} />
            <span>Pro</span>
          </div>
          <h1 className="datamaestro-hero-title">DataMaestro Pro</h1>
          <p className="datamaestro-hero-tagline">
            Conduct any statistical test, hypothesis or model — orchestrated smartly. Upload datasets, get recommendations, and generate publication-ready reports.
          </p>
          <div className="datamaestro-hero-actions">
            <button
              type="button"
              className="datamaestro-cta"
              onClick={() => navigate('/statistical-research')}
            >
              Start analysis
            </button>
            <div className="datamaestro-usage">
              {loadingUsage ? (
                <span className="datamaestro-usage-muted">Loading usage…</span>
              ) : (
                <>
                  <span className="datamaestro-usage-label">DataMaestro uses</span>
                  <span className="datamaestro-usage-value">{usage}</span>
                </>
              )}
            </div>
          </div>
        </section>

        {/* Features grid */}
        <section className="datamaestro-section">
          <h2 className="datamaestro-section-title">Features</h2>
          <p className="datamaestro-section-desc">
            Statistical Research Orchestrator — expert recommendations and report generation for your data.
          </p>
          <div className="datamaestro-features-grid">
            {FEATURES.map((f) => {
              const Icon = f.icon;
              return (
                <div key={f.title} className="datamaestro-feature-card">
                  <div className="datamaestro-feature-icon">
                    <Icon size={22} />
                  </div>
                  <h3 className="datamaestro-feature-title">{f.title}</h3>
                  <p className="datamaestro-feature-desc">{f.description}</p>
                </div>
              );
            })}
          </div>
        </section>

        {/* CTA bottom */}
        <section className="datamaestro-section datamaestro-cta-section">
          <p className="datamaestro-cta-text">Upload your dataset and get test recommendations plus an HTML report.</p>
          <button
            type="button"
            className="datamaestro-cta datamaestro-cta-secondary"
            onClick={() => navigate('/statistical-research')}
          >
            Open DataMaestro
          </button>
        </section>
      </div>
    </DashboardLayout>
  );
};

export default DataMaestroPage;
