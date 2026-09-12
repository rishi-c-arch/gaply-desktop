// Gaply — the Thesis Audit route's page wrapper.
//
// Thin, matching the other check pages: the shell and the readiness probe live
// here, the screen itself stays presentational and testable without a session.
import React, { useEffect, useState } from 'react';
import { AppShell, NavRail, HeaderBar } from '../../design-system/shell';
import { GaplyGlobe } from '../../design-system/GaplyGlobe';
import { useNavigate } from 'react-router-dom';
import { ThesisAuditScreen } from './ThesisAuditScreen';
import { aiBridge, isAiReady } from './aiReady';

export const ThesisAuditPage: React.FC = () => {
  const navigate = useNavigate();
  const [aiInstalled, setAiInstalled] = useState(false);
  // `setResumable` IS NEVER CALLED, and that is a recorded gap rather than dead
  // code. ThesisAuditScreen has a complete resume path — it renders the offer
  // and calls bridge.resumeJob(resumableJob.jobId) — but it can never fire,
  // because this page always hands it null. The missing piece is DISCOVERY:
  // aiBridge can resume a job by id and has no way to find an unfinished one,
  // so wiring this needs a new command (an ai_job_resumable that returns the
  // latest incomplete job), not a change here. Kept rather than deleted so the
  // dead feature stays visible; deleting the setter would make it permanent and
  // silent, which is how it got here.
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const [resumable, setResumable] = useState<{ jobId: number; completed: number; total: number } | null>(
    null,
  );

  useEffect(() => {
    let alive = true;
    isAiReady(aiBridge).then((r) => alive && setAiInstalled(r)).catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  const pickManuscript = async (): Promise<string | null> => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked = await open({
      multiple: false,
      filters: [{ name: 'Manuscript', extensions: ['pdf', 'docx', 'txt', 'md'] }],
    });
    return typeof picked === 'string' ? picked : null;
  };

  return (
    <AppShell
      rail={
        <NavRail
          items={[
            { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
            { id: 'citations', label: 'Citation Audit', icon: '❝', onSelect: () => navigate('/app/check/citations') },
          ]}
          activeId="citations"
          brand={<GaplyGlobe scale="mark" />}
        />
      }
      header={<HeaderBar title="Thesis citation audit" />}
    >
      <div className="gds-root">
        <ThesisAuditScreen
          aiInstalled={aiInstalled}
          resumableJob={resumable}
          pickManuscript={pickManuscript}
          // Attaching a PDF by hand belongs to the citation's Document card;
          // this screen sends the user there rather than growing a second,
          // competing file-picker for the same job.
          onOpenCitation={() => navigate('/app/citations')}
        />
      </div>
    </AppShell>
  );
};

export default ThesisAuditPage;
