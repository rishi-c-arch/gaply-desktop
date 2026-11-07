// Gaply — the Statistics window's menus, as one table (Research OS, Phase 8P,
// decision 451; ADR-012 Part 5). Pure: the state goes in and the design
// system's menus come out, every item in decision 441's words and order. Live
// in this step: File's Open and Close, Gaply's menu and Help. Every other item
// is shown, disabled. The screen never decides what may run: Rust's capability
// list does. "Coming later" is a closed list of plain labels, with no id, no
// handler, no date and no sentence promising anything.
import type { MenuDefinition, MenuItem } from '../../../design-system';
import { METHODS } from '../methods';
import type { CapabilityView } from '../ttestTypes';
import { FILE_BUSY, LATER_STEP, MENU_WORDS, NOT_RUNNABLE, NO_FILE, NO_PROJECT_OPEN, NO_SCORE, OUTPUT_FULL, RUN_GOING, WINDOW_TITLE } from './windowWording';

/** The scale score is no METHODS row, since it has no run kind (decision 451). */
export const SCALE_SCORE_CAPABILITY = 'stats.prepare.scale_score';

export const COMING_LATER = {
  analyze: ['Crosstabs and chi-square', 'Regression', 'Factor analysis', 'Nonparametric tests', 'CFA and SEM'],
  transform: ['Recode values…', 'Compute from a formula…', 'Select cases…'],
};

export type HelpTopic = 'methods' | 'file' | 'keyboard';

export interface WindowMenuActions {
  open: () => void;
  close: () => void;
  go: (path: string) => void;
  switchWorkspace: () => void;
  help: (topic: HelpTopic) => void;
  /** Opens a method's dialog (decision 471). */
  method?: (kind: string) => void;
  /** Opens the scale score's dialog (decision 506). */
  score?: () => void;
  /** Asks first, then exports the prepared data (decision 514). */
  exportPrepared?: () => void;
  /** Opens the Open a project dialog (decision 525). */
  openProject?: () => void;
  /** Opens the open project's report dialog (decision 526). */
  exportReport?: () => void;
}

export interface WindowMenuContext {
  fileOpen: boolean;
  /** Rust says a run is queued or running. */
  runGoing: boolean;
  /** A picker, an open or a close of this window's is going. */
  busy: boolean;
  /** The Output holds its 32 (decision 471). */
  outputFull?: boolean;
  /** The open file holds a scale score: its view has a preparation (decision 514). */
  prepared?: boolean;
  /** Rust holds a project open, as the line reads it (decision 526). */
  projectOpen?: boolean;
  /** Rust's list, or null while unread or when it could not be read. */
  capabilities: CapabilityView[] | null;
  projects: boolean;
  statsCheck: boolean;
  workspaces: boolean;
  actions: WindowMenuActions;
}

const live = (id: string, label: string, onSelect: () => void): MenuItem => ({ id, label, onSelect });
const disabled = (id: string, label: string, description: string): MenuItem => ({ id, label, disabled: true, description });
const comingLater = (labels: string[]) => ({ heading: MENU_WORDS.comingLater, items: labels.map((label): MenuItem => ({ label, disabled: true })) });

/** Why Open cannot be pressed now; undefined when it can. The file's Open
 *  button and the menu's item ask the same question. */
export function openBlocked(c: Pick<WindowMenuContext, 'runGoing' | 'busy'>): string | undefined {
  if (c.runGoing) return RUN_GOING;
  return c.busy ? FILE_BUSY : undefined;
}

function closeBlocked(c: WindowMenuContext): string | undefined {
  if (!c.fileOpen) return NO_FILE;
  return openBlocked(c);
}

/** Rust's word first: an item it does not call runnable says so, and that wins
 *  over the later-step sentence. A list unread has called nothing runnable. */
function capabilityItem(c: WindowMenuContext, id: string, label: string, capabilityId: string): MenuItem {
  const runnable = c.capabilities !== null && c.capabilities.some((x) => x.id === capabilityId && x.runnable === true);
  return disabled(id, label, runnable ? LATER_STEP : NOT_RUNNABLE);
}

/** The methods with a dialog (decisions 473 and 485), each still reading Rust's list. */
export const LIVE_METHODS: readonly string[] = ['describe', 'ttest', 'one_sample', 'paired', 'anova', 'correlations', 'cronbach_alpha'];

/** A method's item: Rust's word first, then whether this build has its dialog,
 *  then why it cannot be pressed now (decision 471). */
function methodItem(c: WindowMenuContext, id: string, label: string, kind: string, capabilityId: string): MenuItem {
  const generic = capabilityItem(c, id, label, capabilityId);
  if (generic.description === NOT_RUNNABLE || !LIVE_METHODS.includes(kind)) return generic;
  const why = !c.fileOpen ? NO_FILE : c.runGoing ? RUN_GOING : c.busy ? FILE_BUSY : c.outputFull ? OUTPUT_FULL : undefined;
  return why ? disabled(id, label, why) : live(id, label, () => c.actions.method?.(kind));
}

/** Transform, Scale score… (decision 506): methodItem's order without the
 *  Output's cap, since a score adds no entry. */
function scoreItem(c: WindowMenuContext): MenuItem {
  const id = 'transform-scale-score';
  const label = 'Scale score…';
  const generic = capabilityItem(c, id, label, SCALE_SCORE_CAPABILITY);
  if (generic.description === NOT_RUNNABLE) return generic;
  const why = !c.fileOpen ? NO_FILE : c.runGoing ? RUN_GOING : c.busy ? FILE_BUSY : undefined;
  return why ? disabled(id, label, why) : live(id, label, () => c.actions.score?.());
}

/** File, Export the prepared data (CSV)… (decision 514): the scale score's
 *  capability first, as scoreItem reads it; then no file; then no score, which
 *  also keeps csv's run.unknown_input for a slot with no recipe from happening;
 *  then busy. A run going does not stop it: stats_prep_csv only reads the slot. */
function exportItem(c: WindowMenuContext): MenuItem {
  const id = 'file-export';
  const label = 'Export the prepared data (CSV)…';
  const generic = capabilityItem(c, id, label, SCALE_SCORE_CAPABILITY);
  if (generic.description === NOT_RUNNABLE) return generic;
  const why = !c.fileOpen ? NO_FILE : c.prepared !== true ? NO_SCORE : c.busy ? FILE_BUSY : undefined;
  return why ? disabled(id, label, why) : live(id, label, () => c.actions.exportPrepared?.());
}

/** METHODS' rows, in order, under each group's heading. */
function analyzeGroups(c: WindowMenuContext): MenuDefinition['groups'] {
  const groups: MenuDefinition['groups'] = [];
  for (const m of METHODS) {
    let group = groups.find((g) => g.heading === m.menuGroup);
    if (!group) {
      group = { heading: m.menuGroup, items: [] };
      groups.push(group);
    }
    group.items.push(methodItem(c, `analyze-${m.kind}`, m.menuLabel, m.kind, m.capabilityId));
  }
  return groups;
}

export function windowMenus(c: WindowMenuContext): MenuDefinition[] {
  const { actions } = c;
  const openWhy = openBlocked(c);
  const closeWhy = closeBlocked(c);
  const menus: MenuDefinition[] = [
    {
      id: 'gaply',
      label: MENU_WORDS.gaply,
      after: WINDOW_TITLE,
      groups: [
        {
          items: [
            live('gaply-home', 'Home', () => actions.go('/app')),
            ...(c.statsCheck ? [live('gaply-check', 'Statistical Analysis Check', () => actions.go('/app/check/stats'))] : []),
            live('gaply-verifier', 'Statistical Analysis Verifier', () => actions.go('/app/statsverifier')),
            live('gaply-settings', 'Settings', () => actions.go('/app/settings')),
            ...(c.workspaces ? [live('gaply-switch', 'Switch workspace', actions.switchWorkspace)] : []),
          ],
        },
      ],
    },
    {
      id: 'file',
      label: MENU_WORDS.file,
      groups: [
        {
          items: [
            openWhy ? disabled('file-open', 'Open a data file…', openWhy) : live('file-open', 'Open a data file…', actions.open),
            closeWhy ? disabled('file-close', 'Close this file', closeWhy) : live('file-close', 'Close this file', actions.close),
            exportItem(c),
          ],
        },
      ],
    },
    {
      id: 'transform',
      label: MENU_WORDS.transform,
      groups: [{ items: [scoreItem(c)] }, comingLater(COMING_LATER.transform)],
    },
    {
      id: 'analyze',
      label: MENU_WORDS.analyze,
      groups: [...analyzeGroups(c), comingLater(COMING_LATER.analyze)],
    },
  ];
  if (c.projects) {
    menus.push({
      id: 'project',
      label: MENU_WORDS.project,
      groups: [
        {
          items: [
            live('project-open', 'Open a project…', () => actions.openProject?.()),
            disabled('project-rerun-all', 'Run every saved result again', LATER_STEP),
            c.projectOpen === true
              ? live('project-report', 'Export the project report…', () => actions.exportReport?.())
              : disabled('project-report', 'Export the project report…', NO_PROJECT_OPEN),
          ],
        },
      ],
    });
  }
  menus.push({
    id: 'help',
    label: MENU_WORDS.help,
    groups: [
      {
        items: [
          live('help-methods', 'Methods and limits', () => actions.help('methods')),
          live('help-file', 'How Gaply reads a file', () => actions.help('file')),
          live('help-keyboard', 'Keyboard', () => actions.help('keyboard')),
        ],
      },
    ],
  });
  return menus;
}
