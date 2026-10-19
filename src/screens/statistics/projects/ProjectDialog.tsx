// Gaply — the project screens' dialog (Research OS, Phase 5B-3a; decision
// 139). The shell's switch dialog uses the design system's Modal, which
// closes on Escape and puts the focus on its first button, but keeps no
// focus inside it and returns none when it closes. This one does both, with
// Modal's look, and stays inside the project screens; Modal is unchanged.
//
// * Focus moves to `initialFocus` (or the first control) when it opens.
// * Tab and Shift+Tab stay inside it.
// * Escape closes it, as Cancel does.
// * When it closes, focus goes to what `returnFocus` names: the control that
//   opened it, recorded from its click (WebKit, which the macOS app runs on,
//   does not focus a button on a click, so the focused element at opening is
//   only the last resort), or after a delete the panel's heading.
// * Its title is fixed text, never a project's name: the title is the
//   dialog's accessible name, and a name must stay inside <bdi> as text.
import React, { useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import '../../../design-system/primitives.css';

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export interface ProjectDialogProps {
  /** Fixed text from the wording table. */
  title: string;
  onClose: () => void;
  initialFocus?: React.RefObject<HTMLElement | null>;
  /** Where focus goes when the dialog closes; the opener by default. */
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
  testId: string;
  children: React.ReactNode;
}

let ids = 0;

export const ProjectDialog: React.FC<ProjectDialogProps> = ({ title, onClose, initialFocus, returnFocus, testId, children }) => {
  const box = useRef<HTMLDivElement>(null);
  const headingId = useRef(`project-dialog-${++ids}`).current;
  const close = useRef(onClose);
  close.current = onClose;
  // Read once, when it opens and when it closes.
  const focusAtOpen = useRef(initialFocus);
  const focusAtClose = useRef(returnFocus);

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const first = focusAtOpen.current?.current ?? box.current?.querySelector<HTMLElement>(FOCUSABLE) ?? box.current;
    first?.focus();
    const target = focusAtClose.current;
    return () => {
      const to = target?.current?.() ?? opener;
      if (to && document.contains(to)) to.focus();
    };
  }, []);

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      close.current();
      return;
    }
    if (e.key !== 'Tab' || !box.current) return;
    const items = Array.prototype.slice.call(box.current.querySelectorAll(FOCUSABLE)) as HTMLElement[];
    if (items.length === 0) {
      e.preventDefault();
      return;
    }
    const firstItem = items[0];
    const lastItem = items[items.length - 1];
    const at = document.activeElement;
    if (e.shiftKey && (at === firstItem || !box.current.contains(at))) {
      e.preventDefault();
      lastItem.focus();
    } else if (!e.shiftKey && (at === lastItem || !box.current.contains(at))) {
      e.preventDefault();
      firstItem.focus();
    }
  };

  return createPortal(
    <div className="gds-modal-overlay" data-testid={`${testId}-overlay`}>
      <div
        ref={box}
        className="gds-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={headingId}
        tabIndex={-1}
        onKeyDown={onKeyDown}
        data-testid={testId}
      >
        <div className="gds-modal__header">
          <h2 id={headingId} style={{ font: 'inherit', margin: 0 }}>
            {title}
          </h2>
        </div>
        <div className="gds-modal__body">{children}</div>
      </div>
    </div>,
    document.body
  );
};

export default ProjectDialog;
