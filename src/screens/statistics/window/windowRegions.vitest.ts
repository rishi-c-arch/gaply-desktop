// The window's regions at 1024 x 700, by CSS rule (Research OS, Phase 8P,
// decision 454; ADR-012 Part 1's table). jsdom has no layout, so this reads the
// rules that make the layout; the real-app check reads what they do.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const css = fs.readFileSync(path.join(__dirname, 'StatisticsWindow.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');

function rule(selector: string): string {
  const at = css.indexOf(`${selector} {`);
  expect(at, `no rule for ${selector}`).toBeGreaterThanOrEqual(0);
  return css.slice(at, css.indexOf('}', at));
}

describe("the window's regions", () => {
  it('fills the window and never scrolls as a whole', () => {
    const w = rule('.gds-root.sw-window');
    expect(w).toMatch(/height: 100vh;/);
    expect(w).toMatch(/overflow: hidden;/);
    expect(w).toMatch(/grid-template-rows: 40px minmax\(0, 1fr\) minmax\(24px, auto\);/);
  });

  it('sizes the tabs at 32 px and the line above the grid at 44 px, leaving 560 px at the minimum', () => {
    expect(rule('.gds-root .sw-main .gds-tabs')).toMatch(/grid-template-rows: 32px minmax\(0, 1fr\);/);
    expect(rule('.gds-root .sw-main .gds-tabs__panel')).toMatch(/grid-template-rows: 44px minmax\(0, 1fr\);/);
    expect(700 - 40 - 32 - 44 - 24).toBe(560);
  });

  it('lets only the tab\'s body scroll', () => {
    expect(rule('.gds-root .sw-body')).toMatch(/overflow: auto;/);
    expect(rule('.gds-root .sw-body')).toMatch(/min-height: 0;/);
    expect(rule('.gds-root .sw-main')).toMatch(/overflow: hidden;/);
    expect(rule('.gds-root .sw-line')).toMatch(/overflow: hidden;/);
    const scrollers = css.match(/overflow(-[xy])?: (auto|scroll)/g) ?? [];
    // The body, and the Help dialog's own content inside its dialog.
    expect(scrollers).toHaveLength(2);
    expect(rule('.gds-root .sw-help')).toMatch(/overflow: auto;/);
  });

  it('never wraps the menus, and clips the project and the file name first', () => {
    expect(rule('.gds-root .sw-top')).toMatch(/white-space: nowrap;/);
    const project = rule('.gds-root .sw-project');
    expect(project).toMatch(/min-width: 0;/);
    expect(project).toMatch(/text-overflow: ellipsis;/);
    expect(rule('.gds-root .sw-line__file .gds-mono')).toMatch(/text-overflow: ellipsis;/);
  });

  it('draws the footer at 12 px, wrapping only if it must', () => {
    const foot = rule('.gds-root .sw-foot');
    expect(foot).toMatch(/font-size: 12px;/);
    expect(foot).toMatch(/flex-wrap: wrap;/);
  });

  it('uses the design system\'s tokens for every colour', () => {
    expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    expect(css).not.toMatch(/rgba?\(/);
  });
});
