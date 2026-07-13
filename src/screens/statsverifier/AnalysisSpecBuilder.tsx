// Gaply — the analysis-spec builder. The investigation's key finding: the app
// CANNOT know which columns produced a reported number unless the user says so.
// This is that user-confirmed mapping — pick the test, map the column roles,
// enter the value you reported (p pre-filled from an attached manuscript where
// extraction can). It emits a well-formed AnalysisSpec; it never recomputes.
import React, { useMemo, useState } from 'react';
import { Badge, Button, Card } from '../../design-system';
import {
  AnalysisSpec,
  ColumnRoles,
  DEFAULT_TOLERANCE,
  StatsPreview,
  TEST_KINDS,
  TestKind,
} from './statsVerifierTypes';

export interface AnalysisSpecBuilderProps {
  preview: StatsPreview;
  onVerify: (spec: AnalysisSpec) => void;
  busy?: boolean;
}

const infoFor = (k: TestKind) => TEST_KINDS.find((t) => t.kind === k)!;

const AnalysisSpecBuilder: React.FC<AnalysisSpecBuilderProps> = ({ preview, onVerify, busy }) => {
  const cols = preview.headers;
  const [testKind, setTestKind] = useState<TestKind>('t_test_welch');
  const layout = infoFor(testKind).layout;

  // role selections (only the ones the current layout needs are read)
  const [samples, setSamples] = useState<string[]>([]);
  const [pairX, setPairX] = useState('');
  const [pairY, setPairY] = useState('');
  const [countCols, setCountCols] = useState<string[]>([]);
  const [outcome, setOutcome] = useState('');
  const [predictors, setPredictors] = useState<string[]>([]);

  const [reportedStat, setReportedStat] = useState('');
  const [reportedP, setReportedP] = useState(
    preview.prefill_p_value != null ? String(preview.prefill_p_value) : ''
  );

  const toggle = (list: string[], set: (v: string[]) => void, c: string) =>
    set(list.includes(c) ? list.filter((x) => x !== c) : [...list, c]);

  const roles: ColumnRoles | null = useMemo(() => {
    switch (layout) {
      case 'samples':
        return samples.length >= 2 ? { layout: 'samples', columns: samples } : null;
      case 'paired':
        return pairX && pairY && pairX !== pairY ? { layout: 'paired', x: pairX, y: pairY } : null;
      case 'contingency':
        return countCols.length >= 2 ? { layout: 'contingency', count_columns: countCols } : null;
      case 'regression':
        return outcome && predictors.length >= 1 && !predictors.includes(outcome)
          ? { layout: 'regression', outcome, predictors }
          : null;
      default:
        return null;
    }
  }, [layout, samples, pairX, pairY, countCols, outcome, predictors]);

  const num = (s: string): number | null => {
    const t = s.trim();
    if (!t) return null;
    const v = Number(t);
    return Number.isFinite(v) ? v : null;
  };

  const valid = roles !== null && (reportedStat.trim() !== '' || reportedP.trim() !== '');

  const submit = () => {
    if (!roles) return;
    onVerify({
      test_kind: testKind,
      roles,
      reported: {
        statistic: num(reportedStat),
        p_value: num(reportedP),
        coefficient_index: null,
      },
      tolerance: DEFAULT_TOLERANCE,
    });
  };

  const ColumnChecklist: React.FC<{ list: string[]; set: (v: string[]) => void; testidPrefix: string }> = ({
    list,
    set,
    testidPrefix,
  }) => (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
      {cols.map((c) => (
        <button
          key={c}
          type="button"
          data-testid={`${testidPrefix}-${c}`}
          onClick={() => toggle(list, set, c)}
          className="gds-pr__alt"
          style={{ borderColor: list.includes(c) ? 'var(--g-certain, #2a7)' : 'var(--g-border)' }}
          aria-pressed={list.includes(c)}
        >
          {list.includes(c) ? '✓ ' : ''}
          {c}
        </button>
      ))}
    </div>
  );

  const Dropdown: React.FC<{ value: string; onChange: (v: string) => void; testid: string; placeholder: string }> = ({
    value,
    onChange,
    testid,
    placeholder,
  }) => (
    <select className="gds-jc__input" data-testid={testid} value={value} onChange={(e) => onChange(e.target.value)}>
      <option value="">{placeholder}</option>
      {cols.map((c) => (
        <option key={c} value={c}>
          {c}
        </option>
      ))}
    </select>
  );

  return (
    <div style={{ display: 'grid', gap: 12 }} data-testid="spec-builder">
      <Card title="1 · Which test did you run?">
        <select
          className="gds-jc__input"
          style={{ width: '100%' }}
          data-testid="spec-test-kind"
          value={testKind}
          onChange={(e) => setTestKind(e.target.value as TestKind)}
        >
          {TEST_KINDS.map((t) => (
            <option key={t.kind} value={t.kind}>
              {t.label}
            </option>
          ))}
        </select>
        <p className="gds-jc__disclaimer" style={{ marginTop: 6 }} data-testid="spec-columns">
          {cols.length} column(s) in your data: {cols.join(', ') || '—'} ({preview.row_count} rows).
        </p>
      </Card>

      <Card title="2 · Map your columns to the test's roles">
        {layout === 'samples' && (
          <div data-testid="roles-samples">
            <p className="gds-jc__disclaimer">
              Pick the {testKind === 'anova' ? 'group columns (2 or more)' : 'two group columns'} to compare — each
              column is one independent sample.
            </p>
            <ColumnChecklist list={samples} set={setSamples} testidPrefix="sample-col" />
          </div>
        )}
        {layout === 'paired' && (
          <div data-testid="roles-paired" style={{ display: 'grid', gap: 8 }}>
            <p className="gds-jc__disclaimer">Pick the two paired columns to correlate.</p>
            <Dropdown value={pairX} onChange={setPairX} testid="paired-x" placeholder="X column…" />
            <Dropdown value={pairY} onChange={setPairY} testid="paired-y" placeholder="Y column…" />
          </div>
        )}
        {layout === 'contingency' && (
          <div data-testid="roles-contingency">
            <p className="gds-jc__disclaimer">
              Pick the observed-count columns (each row is one category level).
            </p>
            <ColumnChecklist list={countCols} set={setCountCols} testidPrefix="count-col" />
          </div>
        )}
        {layout === 'regression' && (
          <div data-testid="roles-regression" style={{ display: 'grid', gap: 8 }}>
            <p className="gds-jc__disclaimer">Pick the outcome, then the predictor column(s).</p>
            <Dropdown value={outcome} onChange={setOutcome} testid="reg-outcome" placeholder="Outcome (y)…" />
            <span className="gds-jc__disclaimer">Predictors:</span>
            <ColumnChecklist list={predictors} set={setPredictors} testidPrefix="reg-predictor" />
          </div>
        )}
      </Card>

      <Card title="3 · What did you report?">
        <div style={{ display: 'grid', gap: 8 }}>
          <label style={{ display: 'grid', gap: 4 }}>
            <span className="gds-jc__disclaimer">
              Reported {infoFor(testKind).statistic} value (the statistic you wrote in your paper)
            </span>
            <input
              className="gds-jc__input"
              data-testid="reported-statistic"
              inputMode="decimal"
              placeholder={`e.g. reported ${infoFor(testKind).statistic}`}
              value={reportedStat}
              onChange={(e) => setReportedStat(e.target.value)}
            />
          </label>
          <label style={{ display: 'grid', gap: 4 }}>
            <span className="gds-jc__disclaimer">
              Reported p-value{' '}
              {preview.prefill_p_value != null && (
                <Badge status="neutral">pre-filled from your manuscript</Badge>
              )}
            </span>
            <input
              className="gds-jc__input"
              data-testid="reported-p"
              inputMode="decimal"
              placeholder="e.g. 0.03"
              value={reportedP}
              onChange={(e) => setReportedP(e.target.value)}
            />
          </label>
        </div>
      </Card>

      <Button onClick={submit} disabled={!valid || busy} data-testid="spec-verify">
        {busy ? 'Recomputing…' : 'Verify against my data'}
      </Button>
      {!valid && (
        <p className="gds-jc__disclaimer" data-testid="spec-hint">
          Map the required column roles and enter at least one reported value to verify.
        </p>
      )}
    </div>
  );
};

export default AnalysisSpecBuilder;
