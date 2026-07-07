// Gaply DS — primitive components (UI-shell only, no features).
// Everything renders inside a `.gds-root` scope; see tokens.css.
import React, { useId, useState } from 'react';
import './tokens.css';
import './primitives.css';

/* ------------------------------- Button --------------------------------- */

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
}

export const Button: React.FC<ButtonProps> = ({ variant = 'primary', className = '', ...rest }) => (
  <button type="button" className={`gds-btn gds-btn--${variant} ${className}`} {...rest} />
);

/* -------------------------------- Card ---------------------------------- */

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  title?: string;
  glass?: boolean;
}

export const Card: React.FC<CardProps> = ({ title, glass = false, className = '', children, ...rest }) => (
  <div className={`gds-card ${glass ? 'gds-card--glass' : ''} ${className}`} {...rest}>
    {title && <h3 className="gds-card__title">{title}</h3>}
    {children}
  </div>
);

/* -------------------------------- Badge --------------------------------- */

/** The four report statuses: certain (green) / assessed (amber) / flagged (red)
 *  / neutral (accent violet). Mirrors the report compiler's certainty tiers. */
export type BadgeStatus = 'certain' | 'assessed' | 'flagged' | 'neutral';

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  status: BadgeStatus;
}

export const Badge: React.FC<BadgeProps> = ({ status, className = '', children, ...rest }) => (
  <span className={`gds-badge gds-badge--${status} ${className}`} {...rest}>
    {children}
  </span>
);

/* ------------------------------ ScoreRing ------------------------------- */

export interface ScoreRingProps {
  /** 0–100 */
  score: number;
  size?: number;
  strokeWidth?: number;
  status?: BadgeStatus;
  label?: string;
}

const STATUS_VAR: Record<BadgeStatus, string> = {
  certain: 'var(--g-certain)',
  assessed: 'var(--g-assessed)',
  flagged: 'var(--g-flagged)',
  neutral: 'var(--g-accent)',
};

export const ScoreRing: React.FC<ScoreRingProps> = ({
  score,
  size = 72,
  strokeWidth = 6,
  status = 'neutral',
  label,
}) => {
  const clamped = Math.max(0, Math.min(100, score));
  const r = (size - strokeWidth) / 2;
  const c = 2 * Math.PI * r;
  return (
    <div
      className="gds-scorering"
      role="meter"
      aria-valuenow={clamped}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-label={label ?? `score ${clamped}`}
      style={{ width: size, height: size }}
    >
      <svg width={size} height={size}>
        <circle className="gds-scorering__track" cx={size / 2} cy={size / 2} r={r} fill="none" strokeWidth={strokeWidth} />
        <circle
          className="gds-scorering__value"
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          strokeWidth={strokeWidth}
          stroke={STATUS_VAR[status]}
          strokeDasharray={c}
          strokeDashoffset={c * (1 - clamped / 100)}
        />
      </svg>
      <span className="gds-scorering__label" style={{ fontSize: size * 0.26 }}>
        {Math.round(clamped)}
      </span>
    </div>
  );
};

/* -------------------------------- Panel --------------------------------- */

export interface PanelProps extends React.HTMLAttributes<HTMLElement> {
  title?: string;
  actions?: React.ReactNode;
}

export const Panel: React.FC<PanelProps> = ({ title, actions, className = '', children, ...rest }) => (
  <section className={`gds-panel ${className}`} {...rest}>
    {(title || actions) && (
      <header className="gds-panel__header">
        <span>{title}</span>
        {actions}
      </header>
    )}
    <div className="gds-panel__body">{children}</div>
  </section>
);

/* --------------------------------- Tabs --------------------------------- */

export interface TabItem {
  id: string;
  label: string;
  content: React.ReactNode;
}

export interface TabsProps {
  tabs: TabItem[];
  defaultTabId?: string;
}

export const Tabs: React.FC<TabsProps> = ({ tabs, defaultTabId }) => {
  const [active, setActive] = useState(defaultTabId ?? tabs[0]?.id);
  const baseId = useId();
  const current = tabs.find((t) => t.id === active) ?? tabs[0];
  return (
    <div className="gds-tabs">
      <div className="gds-tabs__list" role="tablist">
        {tabs.map((t) => (
          <button
            key={t.id}
            role="tab"
            id={`${baseId}-tab-${t.id}`}
            aria-selected={t.id === current?.id}
            aria-controls={`${baseId}-panel-${t.id}`}
            className="gds-tabs__tab"
            onClick={() => setActive(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>
      {current && (
        <div
          className="gds-tabs__panel"
          role="tabpanel"
          id={`${baseId}-panel-${current.id}`}
          aria-labelledby={`${baseId}-tab-${current.id}`}
        >
          {current.content}
        </div>
      )}
    </div>
  );
};

/* ------------------------------ UsageMeter ------------------------------ */

export interface UsageMeterProps {
  label: string;
  used: number;
  limit: number;
  unit?: string;
}

export const UsageMeter: React.FC<UsageMeterProps> = ({ label, used, limit, unit = '' }) => {
  const pct = limit > 0 ? Math.min(100, (used / limit) * 100) : 0;
  const state = pct >= 100 ? 'gds-usagemeter--over' : pct >= 80 ? 'gds-usagemeter--warn' : '';
  return (
    <div
      className={`gds-usagemeter ${state}`}
      role="meter"
      aria-valuenow={used}
      aria-valuemin={0}
      aria-valuemax={limit}
      aria-label={label}
    >
      <div className="gds-usagemeter__row">
        <span>{label}</span>
        <span className="gds-usagemeter__value">
          {used}
          {unit} / {limit}
          {unit}
        </span>
      </div>
      <div className="gds-usagemeter__track">
        <div className="gds-usagemeter__fill" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
};
