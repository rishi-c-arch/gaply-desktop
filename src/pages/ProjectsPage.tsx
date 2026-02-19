import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import DashboardLayout from '../components/Layout/DashboardLayout';
import ProjectsTable from '../components/Dashboard/ProjectsTable';
import { fetchDashboardOverview } from '../services/dashboardService';
import { useAuth } from '../contexts/AuthContext';

const ProjectsPage: React.FC = () => {
  const { isAuthenticated, user, token } = useAuth();
  const navigate = useNavigate();
  const [projects, setProjects] = useState<Awaited<ReturnType<typeof fetchDashboardOverview>>['projects']>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isAuthenticated || !user) {
      navigate('/login');
    }
  }, [isAuthenticated, user, navigate]);

  useEffect(() => {
    if (!token) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    fetchDashboardOverview(token)
      .then((o) => setProjects(o.projects ?? []))
      .catch((err) => setError(err instanceof Error ? err.message : 'Failed to load projects'))
      .finally(() => setLoading(false));
  }, [token]);

  if (!isAuthenticated || !user) {
    return null;
  }

  if (loading) {
    return (
      <DashboardLayout pageTitle="Projects">
        <div style={{ padding: 48, textAlign: 'center', color: 'var(--dashboard-text-muted)' }}>Loading projects…</div>
      </DashboardLayout>
    );
  }

  if (error) {
    return (
      <DashboardLayout pageTitle="Projects">
        <div style={{ padding: 48, textAlign: 'center', color: 'var(--dashboard-danger)' }}>{error}</div>
      </DashboardLayout>
    );
  }

  return (
    <DashboardLayout pageTitle="Projects">
      <div style={{ maxWidth: 900, margin: '0 auto' }}>
        <ProjectsTable projects={projects} />
      </div>
    </DashboardLayout>
  );
};

export default ProjectsPage;
