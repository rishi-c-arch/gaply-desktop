// Gaply — Note Creator recommended-tools panel (Set 5). PURELY STATIC: two
// honest external links + a third-party disclaimer + privacy labels. There is
// NO fetch, NO call, NO data sent to these services — just links the researcher
// can choose to open, with eyes open (both are cloud tools, unlike Gaply's
// on-device work). Gaply earns nothing from them.
import React from 'react';
import { Card } from '../../design-system';
import './notes.css';

const TOOLS = [
  {
    key: 'notebooklm',
    name: 'NotebookLM',
    url: 'https://notebooklm.google/',
    desc: 'Google’s AI notebook — grounds its answers in sources you upload.',
  },
  {
    key: 'gpai',
    name: 'GPAI',
    url: 'https://gpai.app/',
    desc: 'An AI study & research assistant.',
  },
];

const RecommendedToolsPanel: React.FC = () => (
  <Card title="Tools researchers may find useful" data-testid="recommended-tools">
    <div className="gds-tools">
      {TOOLS.map((t) => (
        <a
          key={t.key}
          className="gds-tools__item"
          href={t.url}
          target="_blank"
          rel="noopener noreferrer"
          data-testid={`tool-${t.key}`}
        >
          <div className="gds-tools__name">
            {t.name} <span className="gds-tools__ext" aria-hidden="true">↗</span>
          </div>
          <div className="gds-tools__desc">{t.desc}</div>
          <div className="gds-tools__cloud" data-testid={`tool-cloud-${t.key}`}>
            ☁ processes your text in the cloud — unlike Gaply’s on-device work.
          </div>
        </a>
      ))}
    </div>
    <p className="gds-jc__disclaimer" data-testid="tools-disclaimer">
      These are third-party tools — not affiliated with Gaply, and we’re not responsible for them.
      Do your own diligence. We earn no money from these; they’re shared purely to help researchers.
    </p>
  </Card>
);

export default RecommendedToolsPanel;
