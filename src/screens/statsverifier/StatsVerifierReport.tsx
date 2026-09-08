// Gaply — the Statistical Analysis Verifier report. TWO structurally distinct
// lanes, rendered so a user can never confuse them:
//   · VERIFIED (🟢 match / 🔴 mismatch) — deterministic recomputations. A
//     mismatch shows BOTH the reported and the recomputed value + the delta +
//     the honest evidence line. The recomputed ground truth is ALWAYS shown.
//   · ADVISORY (🟡) — methodology notes, each labelled "advice, not a
//     verification," carrying its disclaimer.
// The lane disclosure renders from the report's REQUIRED `disclosure` field
// (like AiCheckReport's disclaimer) — always present, never strippable.
import React from 'react';
import { Badge, BadgeStatus } from '../../design-system';
import { AdvisoryNote, VerificationReport, VerifiedResult } from './statsVerifierTypes';

const fmt = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(4));

/** Empty-guard backstop for the un-strippable lane disclosure. The wire normally
 *  carries `report.disclosure` VERBATIM from the Rust core's `LANE_DISCLOSURE`
 *  (stats_verdict.rs); this hardcoded copy renders ONLY if that field ever
 *  regresses to empty, so the verified-vs-advisory distinction can never vanish. */
const LANE_DISCLOSURE_FALLBACK =
  'VERIFIED results are deterministic recomputations from your data. ADVISORY ' +
  'notes are methodology observations, NOT verifications — consult a statistician.';

/** The verified lane — the engine's ground truth, match or mismatch. */
const VerifiedCard: React.FC<{ v: VerifiedResult }> = ({ v }) => {
  const isMatch = v.verdict === 'match';
  // §11 D109. Annotated rather than cast at the call site: `BadgeStatus` is a
  // union of literals and this ternary already produces two of them, so the
  // `as any` was never buying anything — it was only stopping the checker from
  // confirming that.
  const status: BadgeStatus = isMatch ? 'certain' : 'flagged'; // 🟢 vs 🔴 — by verdict, not tier
  const name = v.recomputed.statistic_name;
  return (
    <div
      data-testid="sv-verified"
      data-verdict={v.verdict}
      style={{
        border: `2px solid var(--g-${status}, ${isMatch ? '#2a7' : '#e93'})`,
        borderRadius: 10,
        padding: 14,
        background: 'var(--g-bg-layer1)',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <Badge status={status}>{isMatch ? '🟢 VERIFIED — match' : '🔴 VERIFIED — mismatch'}</Badge>
        <strong data-testid="sv-verdict">{v.verdict.toUpperCase()}</strong>
        <span className="gds-mono" style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--g-text-3)' }}>
          {v.recomputed.test_kind}
        </span>
      </div>

      {/* The recomputed ground truth — ALWAYS shown. */}
      <p style={{ marginTop: 10 }} data-testid="sv-recomputed">
        Recomputed from your data: <strong>{name} = {fmt(v.recomputed.statistic)}</strong>, p ={' '}
        {fmt(v.recomputed.p_value)}
      </p>

      {!isMatch && (
        <div
          data-testid="sv-mismatch-evidence"
          style={{ marginTop: 8, padding: 10, borderRadius: 8, background: 'var(--g-flagged-bg, rgba(238,153,51,0.08))' }}
        >
          <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap' }}>
            {v.reported.statistic != null && (
              <span data-testid="sv-reported">
                You reported: <strong>{name} = {fmt(v.reported.statistic)}</strong>
              </span>
            )}
            {v.reported.p_value != null && (
              <span data-testid="sv-reported-p">You reported p = <strong>{fmt(v.reported.p_value)}</strong></span>
            )}
            {v.statistic_delta != null && (
              <span data-testid="sv-delta">Δ = {fmt(v.statistic_delta)}</span>
            )}
          </div>
        </div>
      )}

      <p className="gds-jc__disclaimer" style={{ marginTop: 8 }} data-testid="sv-evidence">
        {v.explanation}
      </p>
      {v.recomputed.detail && (
        <p className="gds-mono" style={{ fontSize: 11, color: 'var(--g-text-3)', margin: '6px 0 0' }} data-testid="sv-detail">
          {v.recomputed.detail}
        </p>
      )}
      {v.recomputed.rows_dropped > 0 && (
        <p className="gds-jc__disclaimer" data-testid="sv-dropped">
          {v.recomputed.rows_dropped} incomplete row(s) were dropped (missing values).
        </p>
      )}
    </div>
  );
};

/** One advisory note (🟡) — clearly "advice, not verification." */
const AdvisoryCard: React.FC<{ n: AdvisoryNote; i: number }> = ({ n, i }) => (
  <div
    data-testid={`sv-advisory-${i}`}
    style={{ border: '1px dashed var(--g-assessed, #e93)', borderRadius: 8, padding: 12, background: 'var(--g-bg-layer1)' }}
  >
    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
      <Badge status="assessed">🟡 advisory</Badge>
      <span className="gds-mono" style={{ fontSize: 12 }}>{n.rule}</span>
      <span style={{ fontSize: 11, color: 'var(--g-text-3)', marginLeft: 'auto' }}>{n.severity}</span>
    </div>
    <p style={{ margin: '8px 0 4px' }}>{n.observation}</p>
    <p className="gds-jc__disclaimer" data-testid={`sv-advisory-disclaimer-${i}`}>{n.disclaimer}</p>
  </div>
);

export interface StatsVerifierReportProps {
  report: VerificationReport;
}

const StatsVerifierReport: React.FC<StatsVerifierReportProps> = ({ report }) => (
  <div className="gds-report" data-testid="statsverifier-report" style={{ display: 'grid', gap: 16 }}>
    {/* VERIFIED lane */}
    <section data-testid="sv-verified-lane">
      <h3 style={{ margin: '0 0 8px' }}>
        Verified <Badge status="certain">deterministic recomputation</Badge>
      </h3>
      {report.verified ? (
        <VerifiedCard v={report.verified} />
      ) : report.recompute_error ? (
        <div
          data-testid="sv-recompute-error"
          style={{ border: '1px solid var(--g-flagged, #e93)', borderRadius: 8, padding: 12 }}
        >
          <Badge status="flagged">couldn’t recompute</Badge>
          <p style={{ marginTop: 6 }}>{report.recompute_error}</p>
          <p className="gds-jc__disclaimer">
            No result is shown rather than a fake one — fix the data or the column mapping and try again.
          </p>
        </div>
      ) : (
        <p className="gds-jc__disclaimer" data-testid="sv-verified-empty">Nothing to verify yet.</p>
      )}
    </section>

    {/* ADVISORY lane — visually distinct (dashed, amber), clearly separated. */}
    <section data-testid="sv-advisory-lane" style={{ borderTop: '1px solid var(--g-border)', paddingTop: 12 }}>
      <h3 style={{ margin: '0 0 4px' }}>
        Advisory <Badge status="assessed">methodology — advice, not verification</Badge>
      </h3>
      <p className="gds-jc__disclaimer" style={{ marginTop: 0 }}>
        These are methodology observations about the reported analysis. They are NOT verifications.
      </p>
      {report.advisory.length > 0 ? (
        <div style={{ display: 'grid', gap: 8, marginTop: 8 }}>
          {report.advisory.map((n, i) => (
            <AdvisoryCard key={i} n={n} i={i} />
          ))}
        </div>
      ) : (
        <p className="gds-jc__disclaimer" data-testid="sv-advisory-empty">No methodology notes.</p>
      )}
    </section>

    {/* The un-strippable lane disclosure — rendered from the required field. */}
    <p
      className="gds-report__disclaimer"
      data-testid="sv-disclosure"
      style={{ borderTop: '1px solid var(--g-border)', paddingTop: 10, fontSize: 12 }}
    >
      {report.disclosure && report.disclosure.trim()
        ? report.disclosure
        : LANE_DISCLOSURE_FALLBACK}
    </p>
  </div>
);

export default StatsVerifierReport;
