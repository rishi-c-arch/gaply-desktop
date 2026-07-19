// Gaply — Note Creator recommended-tools panel (Set 5), restyled for the
// "Academic Focus" design. PURELY STATIC: two honest external links + a
// third-party disclaimer + privacy labels. There is NO fetch, NO call, NO data
// sent to these services — just links the researcher can choose to open, with
// eyes open (both are cloud tools, unlike Gaply's on-device work). Gaply earns
// nothing from them.
import React from 'react';
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
  <section data-testid="recommended-tools">
    <div className="an-section-head"><h3>Tools researchers may find useful</h3></div>
    <div className="an-tools">
      {TOOLS.map((t) => (
        <a key={t.key} href={t.url} target="_blank" rel="noopener noreferrer" data-testid={`tool-${t.key}`}>
          <div className="an-tools-name">{t.name} <span aria-hidden="true">↗</span></div>
          <div className="an-tools-desc">{t.desc}</div>
          <div className="an-tools-cloud" data-testid={`tool-cloud-${t.key}`}>
            ☁ processes your text in the cloud — unlike Gaply’s on-device work.
          </div>
        </a>
      ))}
    </div>
    <p className="an-hint" data-testid="tools-disclaimer">
      These are third-party tools — not affiliated with Gaply, and we’re not responsible for them.
      Do your own diligence. We earn no money from these; they’re shared purely to help researchers.
    </p>
  </section>
);

export default RecommendedToolsPanel;
