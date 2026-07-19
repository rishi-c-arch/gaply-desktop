// Gaply — Note Creator icon set (Stitch "Academic Focus" redesign). Inline SVG
// stand-ins for the mock's Material Symbols: the CDN icon font is not loadable
// (strict CSP + the offline promise), so each glyph used by the design is a
// small bundled outline path. Stroke-based, 24px box, currentColor.
import React from 'react';

type P = { size?: number; filled?: boolean };
const S: React.FC<P & { children: React.ReactNode; viewBox?: string }> = ({ size = 20, children }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
    strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" style={{ flex: 'none' }}>
    {children}
  </svg>
);

export const IcDoc: React.FC<P> = (p) => (
  <S {...p}><path d="M7 3h7l4 4v14H7z" /><path d="M14 3v4h4" /><path d="M9.5 12h5M9.5 15.5h5" /></S>
);
export const IcBulb: React.FC<P> = (p) => (
  <S {...p}><path d="M12 3a6 6 0 0 1 3.5 10.9c-.8.6-1 1.4-1 2.1h-5c0-.7-.2-1.5-1-2.1A6 6 0 0 1 12 3z" /><path d="M10 19h4M10.8 21.5h2.4" /></S>
);
export const IcArticle: React.FC<P> = (p) => (
  <S {...p}><rect x="4.5" y="4" width="15" height="16" rx="2" /><path d="M8 8.5h8M8 12h8M8 15.5h5" /></S>
);
export const IcQuote: React.FC<P> = (p) => (
  <S {...p}><path d="M5 7h5v5H7a3 3 0 0 0 3 3v2a6 6 0 0 1-5-6V7z" fill="currentColor" stroke="none" opacity="0.9" /><path d="M14 7h5v5h-3a3 3 0 0 0 3 3v2a6 6 0 0 1-5-6V7z" fill="currentColor" stroke="none" opacity="0.9" /></S>
);
export const IcTag: React.FC<P> = (p) => (
  <S {...p}><path d="M3.5 12.2V5a1.5 1.5 0 0 1 1.5-1.5h7.2a2 2 0 0 1 1.4.6l6.9 6.9a2 2 0 0 1 0 2.8l-6 6a2 2 0 0 1-2.8 0l-7.6-7.6a2 2 0 0 1-.6-1z" /><circle cx="8.2" cy="8.2" r="1.3" fill="currentColor" stroke="none" /></S>
);
export const IcGear: React.FC<P> = (p) => (
  <S {...p}><circle cx="12" cy="12" r="3" /><path d="M12 3.5l1 2.2a6.6 6.6 0 0 1 2.3 1.3l2.4-.6 1.5 2.6-1.6 1.8a6.7 6.7 0 0 1 0 2.4l1.6 1.8-1.5 2.6-2.4-.6a6.6 6.6 0 0 1-2.3 1.3l-1 2.2h-3l-1-2.2a6.6 6.6 0 0 1-2.3-1.3l-2.4.6-1.5-2.6 1.6-1.8a6.7 6.7 0 0 1 0-2.4L3.8 9l1.5-2.6 2.4.6a6.6 6.6 0 0 1 2.3-1.3l1-2.2z" /></S>
);
export const IcSearch: React.FC<P> = (p) => (
  <S {...p}><circle cx="11" cy="11" r="6" /><path d="M15.5 15.5L20 20" /></S>
);
export const IcAdd: React.FC<P> = (p) => (
  <S {...p}><path d="M12 5v14M5 12h14" /></S>
);
export const IcEditNote: React.FC<P> = (p) => (
  <S {...p}><path d="M4 6h12M4 10h8M4 14h6" /><path d="M14.5 17.5l5.2-5.2 2 2-5.2 5.2H14.5v-2z" /></S>
);
export const IcBook: React.FC<P> = (p) => (
  <S {...p}><path d="M12 6c-1.5-1.3-3.6-2-6-2v14c2.4 0 4.5.7 6 2 1.5-1.3 3.6-2 6-2V4c-2.4 0-4.5.7-6 2z" /><path d="M12 6v14" /></S>
);
export const IcFilter: React.FC<P> = (p) => (
  <S {...p}><path d="M5 7h14M7.5 12h9M10 17h4" /></S>
);
export const IcExport: React.FC<P> = (p) => (
  <S {...p}><path d="M12 4v10M12 14l-3.5-3.5M12 14l3.5-3.5" /><path d="M5 17v2.5h14V17" /></S>
);
export const IcEye: React.FC<P> = (p) => (
  <S {...p}><path d="M3 12s3.5-6 9-6 9 6 9 6-3.5 6-9 6-9-6-9-6z" /><circle cx="12" cy="12" r="2.6" /></S>
);
export const IcTrash: React.FC<P> = (p) => (
  <S {...p}><path d="M5.5 7h13M9 7V5h6v2M7 7l1 13h8l1-13" /><path d="M10.5 11v5.5M13.5 11v5.5" /></S>
);
export const IcSave: React.FC<P> = (p) => (
  <S {...p}><path d="M5 5h11l3 3v11H5z" /><path d="M8 5v4h7V5" /><rect x="8" y="13" width="8" height="6" /></S>
);
export const IcChevron: React.FC<P> = (p) => (
  <S {...p} size={p.size ?? 16}><path d="M9.5 6l6 6-6 6" /></S>
);
export const IcBack: React.FC<P> = (p) => (
  <S {...p}><path d="M20 12H4M4 12l6-6M4 12l6 6" /></S>
);
export const IcOpenNew: React.FC<P> = (p) => (
  <S {...p} size={p.size ?? 18}><path d="M9 5H5v14h14v-4" /><path d="M13 5h6v6" /><path d="M19 5l-8 8" /></S>
);
export const IcHistory: React.FC<P> = (p) => (
  <S {...p} size={p.size ?? 18}><path d="M4.5 5v4h4" /><path d="M4.8 9A8 8 0 1 1 4 12" /><path d="M12 8v4l2.8 2.8" /></S>
);
