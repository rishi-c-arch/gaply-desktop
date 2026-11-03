// The design system's menu bar (Research OS, Phase 8P, decision 451; ADR-012
// Part 5): its roles and its keys. Tab reaches the bar; Left and Right change
// menu; Up and Down change item; Enter opens and chooses; Escape closes and
// returns the focus; a letter jumps. A disabled item stays focusable, with its
// sentence as its description. No key does anything outside the bar.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { Menu, type MenuDefinition } from './Menu';

afterEach(cleanup);

function setUp() {
  const open = vi.fn();
  const help = vi.fn();
  // A disabled item that HAS a handler: otherwise the test cannot reach the
  // guard it names, since a missing handler returns first.
  const closed = vi.fn();
  const menus: MenuDefinition[] = [
    { id: 'gaply', label: 'Gaply', after: 'Statistics', groups: [{ items: [{ id: 'home', label: 'Home', onSelect: vi.fn() }] }] },
    {
      id: 'file',
      label: 'File',
      groups: [
        {
          items: [
            { id: 'open', label: 'Open a data file…', onSelect: open },
            { id: 'close', label: 'Close this file', disabled: true, description: 'Open a data file first.', onSelect: closed },
            { id: 'export', label: 'Export the prepared data (CSV)…', disabled: true, description: 'Later.' },
          ],
        },
      ],
    },
    {
      id: 'analyze',
      label: 'Analyze',
      groups: [
        { heading: 'Compare means', items: [{ id: 't', label: 'T-test…', onSelect: vi.fn() }] },
        { heading: 'Coming later', items: [{ label: 'Regression', disabled: true }] },
      ],
    },
    { id: 'help', label: 'Help', groups: [{ items: [{ id: 'keys', label: 'Keyboard', onSelect: help }] }] },
  ];
  render(
    <div className="gds-root">
      <button data-testid="before">before</button>
      <Menu label="Statistics" menus={menus} />
      <button data-testid="after">after</button>
    </div>
  );
  return { open, help, closed };
}

const trigger = (name: string) => screen.getByRole('menuitem', { name });
const key = (k: string, init: object = {}) => fireEvent.keyDown(document.activeElement as Element, { key: k, ...init });
const focused = () => (document.activeElement as HTMLElement).textContent;

function openFile() {
  trigger('File').focus();
  key('Enter');
}

describe('the roles', () => {
  it('is a menubar of menus, closed at first', () => {
    setUp();
    const bar = screen.getByRole('menubar', { name: 'Statistics' });
    expect(within(bar).getAllByRole('menuitem').map((m) => m.textContent)).toEqual(['Gaply', 'File', 'Analyze', 'Help']);
    expect(screen.queryByRole('menu')).toBeNull();
    expect(trigger('File').getAttribute('aria-haspopup')).toBe('menu');
    expect(trigger('File').getAttribute('aria-expanded')).toBe('false');
  });

  it('only one of the bar\'s buttons is a Tab stop', () => {
    setUp();
    const stops = screen.getAllByRole('menuitem').filter((m) => m.tabIndex === 0);
    expect(stops.map((m) => m.textContent)).toEqual(['Gaply']);
  });

  it('shows what stands beside a menu without making it an item', () => {
    setUp();
    expect(screen.getByText('Statistics')).toBeTruthy();
    expect(within(screen.getByRole('menubar')).queryByRole('menuitem', { name: 'Statistics' })).toBeNull();
  });

  it('a group has its heading, and the heading is not an item', () => {
    setUp();
    trigger('Analyze').focus();
    key('Enter');
    const menu = screen.getByRole('menu');
    expect(within(menu).getByRole('group', { name: 'Coming later' })).toBeTruthy();
    expect(within(menu).getAllByRole('menuitem').map((m) => m.textContent)).toEqual(['T-test…', 'Regression']);
  });
});

describe('the keys', () => {
  it('Enter opens a menu and moves to its first item', () => {
    setUp();
    openFile();
    expect(trigger('File').getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByRole('menu')).toBeTruthy();
    expect(focused()).toBe('Open a data file…');
  });

  it('Left and Right move along the bar, and wrap, when the menu is closed', () => {
    setUp();
    trigger('File').focus();
    key('ArrowRight');
    expect(focused()).toBe('Analyze');
    key('ArrowLeft');
    key('ArrowLeft');
    key('ArrowLeft');
    expect(focused()).toBe('Help');
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('Left and Right change the open menu', () => {
    setUp();
    openFile();
    key('ArrowRight');
    expect(trigger('Analyze').getAttribute('aria-expanded')).toBe('true');
    expect(trigger('File').getAttribute('aria-expanded')).toBe('false');
    expect(focused()).toBe('T-test…');
    key('ArrowLeft');
    expect(focused()).toBe('Open a data file…');
  });

  it('Up and Down move between items, and wrap, through disabled items too', () => {
    setUp();
    openFile();
    key('ArrowDown');
    expect(focused()).toContain('Close this file');
    key('ArrowDown');
    expect(focused()).toContain('Export the prepared data');
    key('ArrowDown');
    expect(focused()).toBe('Open a data file…');
    key('ArrowUp');
    expect(focused()).toContain('Export the prepared data');
  });

  it('Escape closes the menu and returns the focus to its button', () => {
    setUp();
    openFile();
    key('ArrowDown');
    key('Escape');
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(trigger('File'));
  });

  it('Enter chooses an item, closes the menu and returns the focus', () => {
    const { open } = setUp();
    openFile();
    key('Enter');
    expect(open).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(trigger('File'));
  });

  it('a letter jumps to the next item that starts with it, in a menu', () => {
    setUp();
    openFile();
    key('e');
    expect(focused()).toContain('Export the prepared data');
    key('C');
    expect(focused()).toContain('Close this file');
    key('z');
    expect(focused()).toContain('Close this file');
  });

  it('a letter jumps to a menu on the bar', () => {
    setUp();
    trigger('Gaply').focus();
    key('h');
    expect(focused()).toBe('Help');
  });

  it('a letter with Cmd or Ctrl is not the menu\'s', () => {
    setUp();
    trigger('Gaply').focus();
    key('h', { metaKey: true });
    expect(focused()).toBe('Gaply');
    key('h', { ctrlKey: true });
    expect(focused()).toBe('Gaply');
  });

  it('Tab from an open menu closes it, and leaves the focus on its button for the browser to move on', () => {
    setUp();
    openFile();
    key('Tab');
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(trigger('File'));
  });
});

describe('a disabled item', () => {
  it('is focusable, says aria-disabled, and has its sentence as its description', () => {
    setUp();
    openFile();
    key('ArrowDown');
    const item = document.activeElement as HTMLElement;
    expect(item.getAttribute('aria-disabled')).toBe('true');
    expect(item.hasAttribute('disabled')).toBe(false);
    const described = document.getElementById(item.getAttribute('aria-describedby') as string);
    expect(described?.textContent).toBe('Open a data file first.');
  });

  it('does nothing on Enter or a click, and the menu stays open', () => {
    const { open, closed } = setUp();
    openFile();
    key('ArrowDown');
    key('Enter');
    fireEvent.click(document.activeElement as Element);
    expect(closed).not.toHaveBeenCalled();
    expect(open).not.toHaveBeenCalled();
    expect(screen.getByRole('menu')).toBeTruthy();
  });

  it('with no sentence has no description', () => {
    setUp();
    trigger('Analyze').focus();
    key('Enter');
    key('ArrowDown');
    const item = document.activeElement as HTMLElement;
    expect(item.textContent).toBe('Regression');
    expect(item.getAttribute('aria-disabled')).toBe('true');
    expect(item.hasAttribute('aria-describedby')).toBe(false);
  });

  it('shows the focused item\'s sentence at the foot of the menu, for sighted users', () => {
    setUp();
    openFile();
    expect(screen.getByTestId('gds-menu-foot').textContent).toBe('');
    key('ArrowDown');
    expect(screen.getByTestId('gds-menu-foot').textContent).toBe('Open a data file first.');
  });
});

describe('the pointer', () => {
  it('a click opens and closes a menu, and a click outside closes it', () => {
    const { open } = setUp();
    fireEvent.click(trigger('File'));
    expect(screen.getByRole('menu')).toBeTruthy();
    fireEvent.click(trigger('File'));
    expect(screen.queryByRole('menu')).toBeNull();
    fireEvent.click(trigger('File'));
    fireEvent.mouseDown(screen.getByTestId('after'));
    expect(screen.queryByRole('menu')).toBeNull();
    fireEvent.click(trigger('File'));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Open a data file…' }));
    expect(open).toHaveBeenCalledTimes(1);
  });
});

describe('no global shortcut', () => {
  it('a key outside the bar opens nothing and moves nothing', () => {
    const { open, help } = setUp();
    screen.getByTestId('before').focus();
    fireEvent.keyDown(screen.getByTestId('before'), { key: 'h' });
    fireEvent.keyDown(screen.getByTestId('before'), { key: 'o', metaKey: true });
    fireEvent.keyDown(document.body, { key: 'Enter' });
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(screen.getByTestId('before'));
    expect(open).not.toHaveBeenCalled();
    expect(help).not.toHaveBeenCalled();
  });

  it('its source listens for no key on the window or the document', () => {
    const src = fs.readFileSync(path.join(__dirname, 'Menu.tsx'), 'utf8');
    expect(src).not.toMatch(/addEventListener\(\s*['"]key/);
  });

  it('its CSS is its own, so primitives.css stays as dialogTheme pins it', () => {
    expect(fs.readFileSync(path.join(__dirname, 'Menu.tsx'), 'utf8')).toMatch(/import '\.\/Menu\.css';/);
    expect(fs.readFileSync(path.join(__dirname, 'primitives.css'), 'utf8')).not.toContain('gds-menu');
  });
});
