import React from 'react';

export default function SectionLabel({
  children,
  className = '',
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <span
      className={className}
      style={{
        textTransform: 'uppercase',
        letterSpacing: '0.15em',
        fontSize: '0.75rem',
        color: 'var(--accent-purple)',
        fontFamily: 'var(--font-heading)',
        fontWeight: 600,
      }}
    >
      {children}
    </span>
  );
}
