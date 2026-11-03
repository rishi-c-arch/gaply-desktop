// Gaply DS — Menu: a menu bar in a screen, with its menus (Research OS, Phase
// 8P, decision 451; ADR-012 Part 5). role="menubar" and role="menu". Tab
// reaches the bar and leaves it. Left and Right change menu, Up and Down change
// item, Enter opens a menu and chooses an item, Escape closes and returns the
// focus, and a letter jumps. Keys count only while the focus is in the bar:
// there is no global shortcut, so none can be pressed by accident and none
// clashes with a webview's.
//
// A disabled item stays focusable, with aria-disabled and its sentence as its
// description, so a keyboard user hears why. Its handler never runs.
import React, { useCallback, useEffect, useId, useRef, useState } from 'react';
import './Menu.css';

export interface MenuItem {
  /** Names its test id; an item without one is a plain label. */
  id?: string;
  label: string;
  disabled?: boolean;
  /** Why it is disabled: its accessible description. */
  description?: string;
  onSelect?: () => void;
}

export interface MenuGroup {
  /** Shown above the group; not an item. */
  heading?: string;
  items: MenuItem[];
}

export interface MenuDefinition {
  id: string;
  label: string;
  /** Plain words drawn after this menu's button, for the eye. */
  after?: string;
  groups: MenuGroup[];
}

export interface MenuProps {
  /** The menu bar's accessible name. */
  label: string;
  menus: MenuDefinition[];
}

const itemsOf = (m: MenuDefinition): MenuItem[] => m.groups.reduce<MenuItem[]>((out, g) => out.concat(g.items), []);

const isLetter = (e: React.KeyboardEvent) => e.key.length === 1 && /\S/.test(e.key) && !e.metaKey && !e.ctrlKey && !e.altKey;

/** The next label after `from` starting with `letter`, wrapping; -1 for none. */
function jump(labels: string[], from: number, letter: string): number {
  const n = labels.length;
  const want = letter.toLowerCase();
  for (let step = 1; step <= n; step++) {
    const i = (from + step + n) % n;
    if (labels[i].toLowerCase().startsWith(want)) return i;
  }
  return -1;
}

export const Menu: React.FC<MenuProps> = ({ label, menus }) => {
  const base = useId();
  const root = useRef<HTMLDivElement>(null);
  const triggers = useRef<Array<HTMLButtonElement | null>>([]);
  const entries = useRef<Array<HTMLButtonElement | null>>([]);
  /** The bar's one Tab stop. */
  const [stop, setStop] = useState(0);
  const [open, setOpen] = useState<number | null>(null);
  const [active, setActive] = useState(0);
  const shown = open !== null && open < menus.length ? open : null;

  // The focus follows the item, whichever way it moved.
  useEffect(() => {
    if (shown !== null) entries.current[active]?.focus();
  }, [shown, active]);

  const toTrigger = useCallback((i: number) => {
    setStop(i);
    triggers.current[i]?.focus();
  }, []);

  const openMenu = useCallback((i: number, item: number) => {
    setStop(i);
    setActive(item);
    setOpen(i);
  }, []);

  const closeTo = useCallback(
    (i: number) => {
      setOpen(null);
      toTrigger(i);
    },
    [toTrigger]
  );

  // A press anywhere else closes the menu. Only the pointer: a key outside the
  // bar is not this menu's.
  useEffect(() => {
    if (shown === null) return;
    const away = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(null);
    };
    document.addEventListener('mousedown', away);
    return () => document.removeEventListener('mousedown', away);
  }, [shown]);

  const along = (i: number, by: number) => (i + by + menus.length) % menus.length;

  const onTriggerKey = (e: React.KeyboardEvent, i: number) => {
    if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      e.preventDefault();
      toTrigger(along(i, e.key === 'ArrowRight' ? 1 : -1));
    } else if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown') {
      e.preventDefault();
      openMenu(i, 0);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      openMenu(i, itemsOf(menus[i]).length - 1);
    } else if (isLetter(e)) {
      const to = jump(menus.map((m) => m.label), i, e.key);
      if (to >= 0) {
        e.preventDefault();
        toTrigger(to);
      }
    }
  };

  const choose = (i: number, item: MenuItem) => {
    if (item.disabled || !item.onSelect) return;
    closeTo(i);
    item.onSelect();
  };

  const onItemKey = (e: React.KeyboardEvent, i: number, items: MenuItem[], at: number) => {
    const n = items.length;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      setActive((at + (e.key === 'ArrowDown' ? 1 : -1) + n) % n);
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      setActive(e.key === 'Home' ? 0 : n - 1);
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      e.preventDefault();
      openMenu(along(i, e.key === 'ArrowRight' ? 1 : -1), 0);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      closeTo(i);
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      choose(i, items[at]);
    } else if (e.key === 'Tab') {
      // No preventDefault: from its button, the browser moves on past the bar.
      closeTo(i);
    } else if (isLetter(e)) {
      const to = jump(items.map((x) => x.label), at, e.key);
      if (to >= 0) {
        e.preventDefault();
        setActive(to);
      }
    }
  };

  let running = -1;

  return (
    <div className="gds-menubar" role="menubar" aria-label={label} ref={root}>
      {menus.map((m, i) => {
        const items = itemsOf(m);
        const triggerId = `${base}-t-${i}`;
        const isOpen = shown === i;
        running = -1;
        const foot = isOpen ? items[active]?.description ?? '' : '';
        return (
          <div className="gds-menu" role="none" key={m.id}>
            <button
              type="button"
              id={triggerId}
              ref={(el) => {
                triggers.current[i] = el;
              }}
              className="gds-menu__trigger"
              role="menuitem"
              aria-haspopup="menu"
              aria-expanded={isOpen}
              tabIndex={stop === i ? 0 : -1}
              data-testid={`gds-menu-${m.id}`}
              onClick={() => (isOpen ? closeTo(i) : openMenu(i, 0))}
              onMouseEnter={() => {
                if (shown !== null && !isOpen) openMenu(i, 0);
              }}
              onKeyDown={(e) => onTriggerKey(e, i)}
            >
              {m.label}
            </button>
            {m.after && (
              <span className="gds-menu__after" role="none">
                {m.after}
              </span>
            )}
            {isOpen && (
              <div className="gds-menu__popup" role="menu" aria-labelledby={triggerId} data-testid={`gds-menu-popup-${m.id}`}>
                {m.groups.map((g, gi) => (
                  <div role="group" aria-label={g.heading} key={g.heading ?? `group-${gi}`}>
                    {g.heading && (
                      <div className="gds-menu__heading" aria-hidden="true">
                        {g.heading}
                      </div>
                    )}
                    {g.items.map((item) => {
                      running += 1;
                      const at = running;
                      const describedBy = item.description ? `${base}-d-${i}-${at}` : undefined;
                      return (
                        <React.Fragment key={item.id ?? item.label}>
                          <button
                            type="button"
                            ref={(el) => {
                              entries.current[at] = el;
                            }}
                            className="gds-menu__item"
                            role="menuitem"
                            tabIndex={-1}
                            aria-disabled={item.disabled ? true : undefined}
                            aria-describedby={describedBy}
                            data-testid={item.id ? `gds-menu-item-${item.id}` : undefined}
                            onClick={() => choose(i, item)}
                            onMouseEnter={() => setActive(at)}
                            onKeyDown={(e) => onItemKey(e, i, items, at)}
                          >
                            {item.label}
                          </button>
                          {describedBy && (
                            <span id={describedBy} className="gds-menu__description">
                              {item.description}
                            </span>
                          )}
                        </React.Fragment>
                      );
                    })}
                  </div>
                ))}
                <div className="gds-menu__foot" aria-hidden="true" data-testid="gds-menu-foot">
                  {foot}
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
};
