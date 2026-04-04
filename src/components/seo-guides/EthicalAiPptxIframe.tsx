import React, { useState } from 'react';
import { ETHICAL_AI_PPTX_PATH } from './ethicalAiGuideTopics';

type DeckViewer = 'office' | 'google';

export interface EthicalAiPptxIframeProps {
  absoluteUrl: string;
}

/** Fallback when no PDF is deployed: Microsoft / Google wrap the hosted .pptx (cannot drive slide-by-slide from our JS). */
const EthicalAiPptxIframe: React.FC<EthicalAiPptxIframeProps> = ({ absoluteUrl }) => {
  const [viewer, setViewer] = useState<DeckViewer>('office');

  const embedSrc =
    viewer === 'office'
      ? `https://view.officeapps.live.com/op/embed.aspx?src=${encodeURIComponent(absoluteUrl)}`
      : `https://docs.google.com/viewer?url=${encodeURIComponent(absoluteUrl)}&embedded=true`;

  const downloadLocal =
    typeof window !== 'undefined' ? `${window.location.origin}${ETHICAL_AI_PPTX_PATH}` : absoluteUrl;
  const isLocalhost =
    typeof window !== 'undefined' &&
    (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1');

  return (
    <div className="ethical-ai-pptx-wrap ethical-ai-pptx-wrap--fallback">
      <p className="ethical-ai-pptx-fallback-note">
        Add a PDF export of the same deck for slide-by-slide controls on this page. Until then, use the viewers below.
      </p>
      <div className="ethical-ai-viewer-tabs" role="tablist" aria-label="Presentation viewer">
        <button
          type="button"
          role="tab"
          aria-selected={viewer === 'office'}
          className={`ethical-ai-viewer-tab${viewer === 'office' ? ' ethical-ai-viewer-tab--active' : ''}`}
          onClick={() => setViewer('office')}
        >
          Microsoft viewer
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={viewer === 'google'}
          className={`ethical-ai-viewer-tab${viewer === 'google' ? ' ethical-ai-viewer-tab--active' : ''}`}
          onClick={() => setViewer('google')}
        >
          Google viewer
        </button>
        <a className="ethical-ai-viewer-open-tab" href={embedSrc} target="_blank" rel="noopener noreferrer">
          Open in new tab
        </a>
      </div>
      <div className="ethical-ai-pptx-frame">
        <iframe
          key={viewer}
          title="The Ethical Researcher’s Guide to AI — presentation"
          src={embedSrc}
          loading="lazy"
          referrerPolicy="no-referrer-when-downgrade"
          allowFullScreen
        />
      </div>
      <p className="ethical-ai-pptx-meta">
        <a href={downloadLocal} download className="ethical-ai-pptx-download">
          Download .pptx
        </a>
        {isLocalhost ? (
          <a href={absoluteUrl} className="ethical-ai-pptx-download" rel="noreferrer">
            Open production file
          </a>
        ) : null}
      </p>
    </div>
  );
};

export default EthicalAiPptxIframe;
