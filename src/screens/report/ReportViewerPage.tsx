// Gaply — report viewer (F6): renders gaply_core::compile_report() output.
// REUSED by the free checks and by paid PublishReady. Three-panel workspace:
// LEFT outline tree (per-section status dots) · CENTER manuscript with inline
// certainty-tier highlights · RIGHT inspector (selected finding: detail,
// provenance "click to see why", confidence). Export → local PDF.
import React, { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import {
  AppShell,
  Badge,
  BadgeStatus,
  Button,
  Card,
  GaplyGlobe,
  HeaderBar,
  NavRail,
  Panel,
  ScoreRing,
  ThreePanelWorkspace,
} from '../../design-system';
import { useToast, ToastProvider } from '../../design-system/Toast';
import { useGaplySession } from '../session/SessionProvider';
import {
  ChecklistItem,
  Finding,
  findingsForTab,
  PublishReadyReport,
  ReportTab,
  REPORT_TABS,
  sortFindings,
  tierStatus,
} from './reportTypes';
import { SAMPLE_MANUSCRIPT_SECTIONS, SAMPLE_REPORT } from './sampleReport';
import { downloadReportPdf } from './exportPdf';
import '../auth/auth.css';
import './report.css';

export interface ReportViewerPageProps {
  /** The compiled report. Defaults to the golden sample until a
   *  `compile_report` Tauri command feeds a live one. */
  report?: PublishReadyReport;
  manuscriptSections?: { section: string; text: string }[];
  /** Paid PublishReady also gets the Reviewer Letter tab (F10). */
  isPaid?: boolean;
  /** Restrict the tab set — the single-agent F7 screens scope the viewer to
   *  just their agent (e.g. ['Overview', 'Plagiarism']). */
  tabs?: ReportTab[];
  /** Optional header title override (e.g. "Plagiarism Check"). */
  title?: string;
  /** Render inside a caller-provided shell instead of the full AppShell. */
  bare?: boolean;
}

function statusForSection(findings: Finding[], section: string): BadgeStatus | 'neutral' {
  const inSection = findings.filter((f) => f.section === section);
  if (inSection.some((f) => tierStatus(f.tier) === 'flagged')) return 'flagged';
  if (inSection.some((f) => tierStatus(f.tier) === 'certain' && f.severity === 'critical')) return 'certain';
  if (inSection.some((f) => tierStatus(f.tier) === 'assessed')) return 'assessed';
  return 'neutral';
}

const ReportInner: React.FC<ReportViewerPageProps> = ({
  report = SAMPLE_REPORT,
  manuscriptSections = SAMPLE_MANUSCRIPT_SECTIONS,
  isPaid = false,
  tabs: tabsProp,
  title = 'Integrity report',
  bare = false,
}) => {
  const { session } = useGaplySession();
  const { toast } = useToast();
  const ordered = useMemo(() => sortFindings(report.findings), [report.findings]);
  const tabs = tabsProp ?? (isPaid ? [...REPORT_TABS, 'Reviewer Letter' as ReportTab] : REPORT_TABS);
  const [tab, setTab] = useState<ReportTab>(tabs[0] ?? 'Overview');
  const [selectedId, setSelectedId] = useState<number>(0); // index into `ordered`
  const [activeSection, setActiveSection] = useState<string>(manuscriptSections[0]?.section ?? '');

  const selected = ordered[selectedId];

  const selectFinding = (f: Finding) => {
    const idx = ordered.indexOf(f);
    setSelectedId(idx);
    if (f.section) setActiveSection(f.section);
  };

  const exportButton = (
    <Button
      variant="secondary"
      data-testid="export-pdf"
      onClick={() => {
        void downloadReportPdf(report, 'gaply-integrity-report.pdf')
          .then(() => toast('Report exported as PDF', 'certain'))
          .catch(() => toast('PDF export failed', 'flagged'));
      }}
    >
      Export PDF
    </Button>
  );

  const reportBody = (
        <div className="gds-report">
          {/* top tabs */}
          <div className="gds-report__tabs" role="tablist" data-testid="report-tabs">
            {tabs.map((t) => (
              <button
                key={t}
                role="tab"
                aria-selected={t === tab}
                className="gds-report__tab"
                data-testid={`tab-${t}`}
                onClick={() => setTab(t)}
              >
                {t}
              </button>
            ))}
          </div>

          <ThreePanelWorkspace
            outline={
              <Panel title="Outline">
                <div className="gds-outline" data-testid="outline">
                  {manuscriptSections.map((s) => (
                    <button
                      key={s.section}
                      className="gds-outline__item"
                      aria-current={s.section === activeSection}
                      data-testid={`outline-${s.section}`}
                      onClick={() => setActiveSection(s.section)}
                    >
                      <span
                        className="gds-outline__dot"
                        data-status={statusForSection(ordered, s.section)}
                      />
                      {s.section}
                    </button>
                  ))}
                </div>
              </Panel>
            }
            inspector={
              <Panel title="Inspector">
                <div className="gds-inspector" data-testid="inspector">
                  {selected ? (
                    <>
                      <div>
                        <span className="gds-inspector__label">Finding</span>
                        <div style={{ fontWeight: 600 }}>{selected.title}</div>
                      </div>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                        <ScoreRing
                          size={54}
                          strokeWidth={5}
                          score={Math.round(selected.confidence * 100)}
                          status={tierStatus(selected.tier)}
                        />
                        <Badge status={tierStatus(selected.tier)} data-testid="inspector-tier">
                          {selected.certainty_label}
                        </Badge>
                      </div>
                      <p className="gds-finding__detail">{selected.detail}</p>
                      {selected.reconsidered && (
                        <p className="gds-finding__reconsidered" data-testid="inspector-reconsidered">
                          Reconsidered after peer review: <s>{selected.reconsidered.from}</s> →{' '}
                          {selected.reconsidered.to}
                        </p>
                      )}
                      <div>
                        <span className="gds-inspector__label">Provenance · click to see why</span>
                        <div className="gds-provenance" data-testid="inspector-provenance">
                          {selected.provenance.map((p, i) => (
                            <div key={i} className="gds-provenance__item">{p}</div>
                          ))}
                        </div>
                      </div>
                    </>
                  ) : (
                    <p className="gds-finding__detail">Select a finding to see its provenance.</p>
                  )}
                </div>
              </Panel>
            }
          >
            <Panel title={tab}>
              {tab === 'Checklist' ? (
                <ChecklistView items={report.checklist} />
              ) : tab === 'Overview' ? (
                <OverviewView
                  report={report}
                  sections={manuscriptSections}
                  ordered={ordered}
                  activeSection={activeSection}
                  selected={selected}
                  onSelectFinding={selectFinding}
                />
              ) : (tab as string) === 'Reviewer Letter' ? (
                <p className="gds-finding__detail" data-testid="reviewer-letter">
                  The full reviewer letter is generated in PublishReady (F10).
                </p>
              ) : (
                <FindingsList
                  findings={findingsForTab(ordered, tab)}
                  selected={selected}
                  onSelect={selectFinding}
                />
              )}

              {/* mandatory disclaimer — always visible */}
              <p className="gds-report__disclaimer" data-testid="report-disclaimer" style={{ marginTop: 16 }}>
                {report.disclaimer}
              </p>
            </Panel>
          </ThreePanelWorkspace>
        </div>
  );

  // bare: the caller supplies the gds-root + shell (F7 single-agent screens
  // embed the scoped viewer inside their own CheckScreen scaffold).
  if (bare) {
    return (
      <>
        <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: 8 }}>{exportButton}</div>
        {reportBody}
        {!session && <span style={{ display: 'none' }} data-testid="offline-ok" />}
      </>
    );
  }

  return (
    <div className="gds-root" style={{ height: '100vh' }} data-testid="report-viewer">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => (window.location.hash = '#/app') },
              { id: 'report', label: 'Report', icon: '✓' },
            ]}
            activeId="report"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={
          <HeaderBar title={title}>
            <Badge status={report.verdict === 'pass' ? 'certain' : 'flagged'}>{report.verdict}</Badge>
            {exportButton}
          </HeaderBar>
        }
      >
        {reportBody}
      </AppShell>
      {!session && <span style={{ display: 'none' }} data-testid="offline-ok" />}
    </div>
  );
};

/* ------------------------------- overview ------------------------------- */

const OverviewView: React.FC<{
  report: PublishReadyReport;
  sections: { section: string; text: string }[];
  ordered: Finding[];
  activeSection: string;
  selected?: Finding;
  onSelectFinding: (f: Finding) => void;
}> = ({ report, sections, ordered, activeSection, selected, onSelectFinding }) => (
  <div style={{ display: 'grid', gap: 16 }}>
    <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
      <ScoreRing
        score={Math.round(report.combined_confidence * 100)}
        status={report.verdict === 'pass' ? 'certain' : 'flagged'}
      />
      <div>
        <div style={{ fontWeight: 700 }}>Verdict: {report.verdict.toUpperCase()}</div>
        <div className="gds-finding__detail">
          {ordered.length} findings · {report.debate.rounds_run} debate round(s)
          {report.debate.overridden_by_constraint && ' · hard constraint applied'}
        </div>
      </div>
    </div>

    {/* center manuscript with inline tier highlights */}
    <Card title="Manuscript">
      <div className="gds-manuscript" data-testid="manuscript">
        {sections.map((s) => {
          const f = ordered.find((x) => x.section === s.section);
          return (
            <div
              key={s.section}
              className="gds-ms-section"
              data-active={s.section === activeSection}
            >
              <span className="gds-ms-section__heading">{s.section}</span>
              <p className="gds-ms-section__body">
                {f ? (
                  <button
                    className="gds-highlight"
                    data-tier={tierStatus(f.tier)}
                    data-testid={`highlight-${s.section}`}
                    aria-current={selected === f}
                    onClick={() => onSelectFinding(f)}
                    title="click to see why"
                  >
                    {s.text}
                  </button>
                ) : (
                  s.text
                )}
              </p>
            </div>
          );
        })}
      </div>
    </Card>

    <FindingsList findings={ordered} selected={selected} onSelect={onSelectFinding} />
  </div>
);

/* ----------------------------- findings list ---------------------------- */

const FindingsList: React.FC<{
  findings: Finding[];
  selected?: Finding;
  onSelect: (f: Finding) => void;
}> = ({ findings, selected, onSelect }) => (
  <div className="gds-findings" data-testid="findings">
    {findings.length === 0 ? (
      <p className="gds-finding__detail">No findings in this category.</p>
    ) : (
      findings.map((f, i) => (
        <button
          key={i}
          className="gds-finding"
          aria-current={selected === f}
          data-testid={`finding-${i}`}
          data-tier={f.tier}
          onClick={() => onSelect(f)}
        >
          <div className="gds-finding__head">
            <span className="gds-finding__title">{f.title}</span>
            <Badge status={tierStatus(f.tier)} data-testid={`finding-tier-${i}`}>
              {f.certainty_label}
            </Badge>
          </div>
          <span className="gds-finding__detail">{f.detail}</span>
          {f.reconsidered && (
            <span className="gds-finding__reconsidered">
              <s>{f.reconsidered.from}</s> → {f.reconsidered.to}
            </span>
          )}
          <span className="gds-finding__detail" style={{ color: 'var(--g-text-3)', fontSize: 11 }}>
            provenance: {f.provenance[0]}
            {f.provenance.length > 1 ? ` (+${f.provenance.length - 1})` : ''}
          </span>
        </button>
      ))
    )}
  </div>
);

/* ------------------------------- checklist ------------------------------ */

const ChecklistView: React.FC<{ items: ChecklistItem[] }> = ({ items }) => (
  <div className="gds-checklist" data-testid="checklist">
    {items.map((c, i) => (
      <div key={i} className="gds-checklist__item" data-testid={`check-${i}`}>
        <span className="gds-checklist__mark" data-pass={c.passed}>
          {c.passed ? '✓' : '✗'}
        </span>
        <div>
          <div>{c.requirement}</div>
          <div className="gds-finding__detail" style={{ fontSize: 12 }}>{c.detail}</div>
        </div>
        {c.guideline_source && (
          <span className="gds-checklist__src">
            <Badge status="neutral">source</Badge>
          </span>
        )}
      </div>
    ))}
  </div>
);

const ReportViewerPage: React.FC<ReportViewerPageProps> = (props) => (
  <ToastProvider>
    <ReportInner {...props} />
  </ToastProvider>
);

export default ReportViewerPage;
