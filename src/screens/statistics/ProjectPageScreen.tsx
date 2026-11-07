// Gaply — the page around an open project's page (Research OS, Phase 5B-3b;
// decision 149), at /app/statistics/projects/:projectId. It reads the
// project's id from the route and does the navigation the project screens
// never do themselves (projects/projectGuards.vitest.ts): Close, the rail's
// Statistics item and "Back to Statistics" go to projectPageHome(): the
// Statistics window while it is shown, the Statistics home otherwise, each a
// fixed path (Research OS, Phase 8U-1, decision 524). Every title here is
// fixed text; a project's name appears only inside the page, as text.
import React, { useCallback } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { AppShell, GaplyGlobe, HeaderBar, NavRail, Panel } from '../../design-system';
import { ProjectPage } from './projects/ProjectPage';
import { projectPageHome } from './statisticsRoute';

export const ProjectPageScreen: React.FC = () => {
  const navigate = useNavigate();
  const { projectId = '' } = useParams();
  const home = useCallback(() => navigate(projectPageHome()), [navigate]);
  return (
    <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="project-page-screen">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'statistics', label: 'Statistics', icon: 'Σ', onSelect: home },
            ]}
            activeId="statistics"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={<HeaderBar title="Statistics" />}
      >
        <Panel title="A saved project">
          <ProjectPage projectId={projectId} onClosed={home} onBack={home} />
        </Panel>
      </AppShell>
    </div>
  );
};

export default ProjectPageScreen;
