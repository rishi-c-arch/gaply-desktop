// Gaply — Research Paper Writer, Set B1: the citation context + the gaplyCite
// NodeView (live, style-aware in-text marker). The ManuscriptEditor provides the
// context (a signature-gated marker map + library search); the NodeView consumes
// it, so a style switch re-renders every marker without touching prose.
import React, { createContext, useContext } from 'react';
import { ReactNodeViewRenderer, NodeViewWrapper, NodeViewProps } from '@tiptap/react';
import { GaplyCite } from './GaplyCiteNode';
import { Marker } from './manuscriptCitations';

export interface CitationPickItem { id: string; title: string; authors: string; year: number | null }

export interface CitationCtx {
  /** The live marker for a refId (formatted in the manuscript's chosen style). */
  markerFor: (refId: string) => Marker;
  /** Search the user's citation_library for the insert picker. */
  search: (query: string) => Promise<CitationPickItem[]>;
}

// Default = no citations wired (project notes never provide this). markerFor
// returns a neutral placeholder so a stray node never crashes.
const CitationContext = createContext<CitationCtx>({
  markerFor: () => ({ marker: '', missing: false }),
  search: async () => [],
});

export const CitationProvider = CitationContext.Provider;
export const useCitationCtx = (): CitationCtx => useContext(CitationContext);

/** The in-text citation chip. Shows the formatted marker for the chosen style
 *  (e.g. "[1]" or "(Smith, 2020)"); a dangling ref (deleted from the library)
 *  shows an honest ⚠ "missing" state, never a crash, never a silent blank. */
const GaplyCiteView: React.FC<NodeViewProps> = ({ node }) => {
  const refId = (node.attrs.refId as string) ?? '';
  const { marker, missing } = useCitationCtx().markerFor(refId);
  return (
    <NodeViewWrapper as="span" className={`an-cite${missing ? ' an-cite--missing' : ''}`} data-testid={`cite-${refId}`} data-missing={missing ? '1' : '0'} contentEditable={false}>
      {missing ? '⚠ missing reference' : (marker || '…')}
    </NodeViewWrapper>
  );
};

/** The gaplyCite node WITH the live NodeView — used only in the manuscript
 *  writing surface. Inherits the markdown round-trip from the base node. */
export const GaplyCiteLive = GaplyCite.extend({
  addNodeView() {
    return ReactNodeViewRenderer(GaplyCiteView);
  },
});
