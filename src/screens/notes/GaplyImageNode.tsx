// Gaply — the image node for the rich editor (Set 3). Extends TipTap's FREE
// Image extension with a React NodeView that RENDERS a gaply-image://<hash>
// ref by resolving it to a blob: object URL (via noteImages.resolveImageRef).
//
// The node's `src` attribute stays the CANONICAL ref, untouched — so
// tiptap-markdown serializes it back to ![](gaply-image://<hash>.<ext>) and the
// note's markdown-canonical body round-trips losslessly. The blob URL is a
// display-only concern that never touches the stored bytes.
import React, { useEffect, useState } from 'react';
import { Image } from '@tiptap/extension-image';
import { ReactNodeViewRenderer, NodeViewWrapper, NodeViewProps } from '@tiptap/react';
import { isImageRef, resolveImageRef } from './noteImages';

const GaplyImageView: React.FC<NodeViewProps> = ({ node }) => {
  const src = (node.attrs.src as string) ?? '';
  // A gaply-image ref must be resolved from disk to a blob URL; a plain URL
  // (http/data/blob, e.g. legacy content) is shown as-is.
  const [display, setDisplay] = useState<string>(isImageRef(src) ? '' : src);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let alive = true;
    if (isImageRef(src)) {
      setDisplay('');
      setFailed(false);
      resolveImageRef(src)
        .then((url) => { if (alive) setDisplay(url); })
        .catch(() => { if (alive) setFailed(true); });
    } else {
      setDisplay(src);
    }
    return () => { alive = false; };
  }, [src]);

  return (
    <NodeViewWrapper as="span" className="an-img" data-testid="an-image">
      {display ? (
        <img src={display} alt={(node.attrs.alt as string) || ''} draggable={false} />
      ) : (
        <span className="an-img-ph">{failed ? 'Image unavailable' : 'Loading image…'}</span>
      )}
    </NodeViewWrapper>
  );
};

/** The production image node: same schema/name ('image') as @tiptap/extension-image
 *  (so markdown parse/serialize is unchanged) with a blob-resolving NodeView.
 *
 *  `inline: true` IS THE FIX for images not holding their place. The extension
 *  defaults to `inline: false`, which makes the schema group 'block' — and a
 *  block node cannot live inside a paragraph, so ProseMirror split every
 *  paragraph around an image. "See ![](…) and more text." came back as three
 *  blocks with the spaces gone, and an image-only paragraph was welded to
 *  whatever followed it. Nothing in the serializer was wrong; the node was the
 *  wrong shape. The NodeView already renders `<NodeViewWrapper as="span">`, so
 *  inline is also what the rest of this file always assumed. */
export const GaplyImage = Image.extend({
  addNodeView() {
    return ReactNodeViewRenderer(GaplyImageView);
  },
}).configure({ inline: true });
