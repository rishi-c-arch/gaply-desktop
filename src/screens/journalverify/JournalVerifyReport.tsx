// Gaply — Journal Verification evidence report (Set 5). Makes VERIFIED (grounded
// registry facts) vs CLAIMED (self-reported, from the journal's own site) vs
// UNCHECKED (sources with no free API / couldn't-verify) VISUALLY unmistakable.
// Evidence + signals, NEVER a verdict — the researcher decides. The un-strippable
// disclosure is rendered structurally on every report.
import React from 'react';
import { Badge } from '../../design-system';
import { JournalVerificationResult, JournalRegistryFacts, SiteSummary } from './journalVerifyTypes';
import './journalverify.css';

/** Structural, un-strippable — rendered on EVERY report (a required constant,
 *  never a dismissible toast). */
export const DISCLOSURE =
  'This is EVIDENCE, not a verdict. Gaply gathers signals from public registries and the ' +
  'journal’s own website — you decide. Details marked “the journal claims” are self-reported and ' +
  'unverified. “Not found” is a warning sign, not proof that a journal is predatory.';

const FIELD_LABEL: Record<string, string> = {
  apc: 'APC / fees',
  review_timeline: 'Review timeline',
  guidelines_link: 'Submission guidelines',
  contact: 'Contact',
};

const tri = (v: boolean | null, yes: string, no: string) =>
  v === true ? yes : v === false ? no : 'Couldn’t verify';

/** GROUNDED registry facts — VERIFIED treatment, each with its source registry. */
const RegistrySection: React.FC<{ r: JournalRegistryFacts }> = ({ r }) => (
  <section className="gds-jv-verified" data-testid="jv-registry">
    <div className="gds-jv-sec__head">
      <Badge status="certain">✓ Verified from public registries</Badge>
      <span className="gds-jv-sec__sub">facts retrieved from the registries named — not opinions</span>
    </div>

    <div className="gds-jv-facts">
      <Fact label="DOAJ (open-access directory)" value={tri(r.doaj_registered, 'Registered', 'Not in DOAJ')} source="DOAJ" srcUrl="https://doaj.org" show={r.doaj_registered !== null} testid="jv-fact-doaj" />
      <Fact label="PubMed / NLM Catalog" value={tri(r.pubmed_indexed, 'Indexed', 'Not in the NLM Catalog')} source="NLM Catalog (PubMed)" srcUrl="https://www.ncbi.nlm.nih.gov/nlmcatalog" show={r.pubmed_indexed !== null} testid="jv-fact-pubmed" />
      <Fact label="Recent publishing activity" value={tri(r.recent_activity, 'Active recently', 'No recent activity')} source="OpenAlex" srcUrl="https://openalex.org" show={r.recent_activity !== null} testid="jv-fact-activity" />
      {r.scope.length > 0 && (
        <div className="gds-jv-fact" data-testid="jv-fact-scope">
          <div className="gds-jv-fact__label">Scope</div>
          <div className="gds-jv-fact__value">{r.scope.join(' · ')}</div>
          <span className="gds-jv-fact__src">✓ from OpenAlex</span>
        </div>
      )}
    </div>

    {r.warning && (
      <p className="gds-jv-warning" data-testid="jv-warning">{r.warning}</p>
    )}
    {r.unverified.length > 0 && (
      <div className="gds-jv-unverified" data-testid="jv-unverified">
        <strong>Couldn’t verify:</strong>
        <ul>{r.unverified.map((u, i) => <li key={i}>{u}</li>)}</ul>
      </div>
    )}
    {r.sources_not_checked.length > 0 && (
      <p className="gds-jv-unchecked" data-testid="jv-sources-not-checked">
        Not checked: {r.sources_not_checked.join(' · ')}
      </p>
    )}
  </section>
);

const Fact: React.FC<{ label: string; value: string; source: string; srcUrl: string; show: boolean; testid: string }> = ({ label, value, source, srcUrl, show, testid }) =>
  !show ? (
    <div className="gds-jv-fact gds-jv-fact--muted" data-testid={testid}>
      <div className="gds-jv-fact__label">{label}</div>
      <div className="gds-jv-fact__value">Couldn’t verify</div>
    </div>
  ) : (
    <div className="gds-jv-fact" data-testid={testid}>
      <div className="gds-jv-fact__label">{label}</div>
      <div className="gds-jv-fact__value">{value}</div>
      <a className="gds-jv-fact__src" href={srcUrl} target="_blank" rel="noopener noreferrer">✓ verified from {source} ↗</a>
    </div>
  );

/** "THE JOURNAL CLAIMS" — self-reported, visually DISTINCT (dashed/amber),
 *  prominently labeled unverified. Never blends with the verified facts. */
const ClaimsSection: React.FC<{ s: SiteSummary }> = ({ s }) => (
  <section className="gds-jv-claimed" data-testid="jv-claims">
    <div className="gds-jv-sec__head">
      <Badge status="assessed">“The journal claims” — self-reported</Badge>
    </div>
    <p className="gds-jv-claims__label" data-testid="jv-claims-label">
      {s.label} {/* = SELF_REPORTED_LABEL: "…self-reported, not independently verified." */}
    </p>

    {s.status !== 'summarized' ? (
      <p className="gds-jv-claims__notice" data-testid="jv-claims-notice">{s.notice}</p>
    ) : (
      <div className="gds-jv-facts">
        {s.details.map((d) => (
          <div className="gds-jv-claim" data-testid={`jv-claim-${d.field}`} key={d.field}>
            <div className="gds-jv-fact__label">{FIELD_LABEL[d.field] ?? d.field}</div>
            {d.value ? (
              d.field === 'guidelines_link' ? (
                <a className="gds-jv-claim__value" href={d.value} target="_blank" rel="noopener noreferrer" data-testid={`jv-claim-value-${d.field}`}>{d.value} ↗</a>
              ) : (
                <div className="gds-jv-claim__value" data-testid={`jv-claim-value-${d.field}`}>{d.value}</div>
              )
            ) : (
              <div className="gds-jv-claim__notstated" data-testid={`jv-claim-notstated-${d.field}`}>not stated</div>
            )}
            <span className="gds-jv-claim__src">claimed on {hostOf(d.source)}</span>
          </div>
        ))}
      </div>
    )}
  </section>
);

const hostOf = (url: string) => {
  try { return new URL(url).host; } catch { return url; }
};

export interface JournalVerifyReportProps {
  result: JournalVerificationResult;
}

const JournalVerifyReport: React.FC<JournalVerifyReportProps> = ({ result }) => (
  <div className="gds-jv-report" data-testid="jv-report">
    {/* NOT-FOUND — honest warning, explicitly NOT a verdict */}
    {result.not_found && (
      <div className="gds-jv-notfound" data-testid="jv-notfound">
        <Badge status="flagged">not found in the registries</Badge>
        <p>
          This journal wasn’t found in DOAJ, OpenAlex, or PubMed. That’s a warning sign — many
          predatory journals aren’t indexed — but <strong>not-found is not proof</strong>: some
          legitimate new or niche journals aren’t indexed either. Investigate carefully before
          submitting.
        </p>
      </div>
    )}

    {result.registry && <RegistrySection r={result.registry} />}
    {result.site_summary && <ClaimsSection s={result.site_summary} />}

    {result.notes.length > 0 && (
      <ul className="gds-jv-notes" data-testid="jv-notes">
        {result.notes.map((n, i) => <li key={i}>{n}</li>)}
      </ul>
    )}

    {/* The un-strippable disclosure — structural, on every report */}
    <p className="gds-jv-disclosure" data-testid="jv-disclosure">{DISCLOSURE}</p>
  </div>
);

export default JournalVerifyReport;
