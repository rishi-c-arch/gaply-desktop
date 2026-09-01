// Gaply — every frontend AI call must match its Rust command signature.
//
// This is the same class of defect as the serde wire drift (§11 D53, where the
// backend renamed event VARIANTS but not their FIELDS): two sides of one
// contract, edited independently, with nothing checking they still agree. That
// one cost a panel that showed a single static line for a whole run. This one
// cost "invalid args `section` for command `ai_citation_need`: missing required
// key `section`" — a button that could never work.
//
// A key-presence check would NOT have caught it. The bridge passed
// `{ sentence, section }` with `section?: string`; the key is right there in the
// source, and an undefined value serializes to a MISSING KEY at runtime. So the
// rule enforced here is the one that actually holds:
//
//   a REQUIRED Rust parameter (not Option<…>) must be fed by a TS argument that
//   cannot be undefined — a non-optional parameter, a defaulted one, or a
//   literal expression.
//
// Read from source rather than exercised through IPC on purpose: there is no
// Tauri host in vitest, and the drift is a static fact about two files.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

/** Rust source with `//` line comments stripped.
 *
 *  A parameter may carry a comment above it, and the comment's own colons and
 *  commas would otherwise be parsed as part of the parameter list — which
 *  reported a real, correctly-declared parameter as an unknown key. */
const RUST = fs
  .readFileSync(path.resolve(__dirname, '../../../src-tauri/src/commands.rs'), 'utf8')
  .replace(/^[ \t]*\/\/.*$/gm, '');
const BRIDGE = fs.readFileSync(path.resolve(__dirname, 'aiBridge.ts'), 'utf8');

/** Params Tauri injects; never sent from JS. */
const INJECTED = /^(state|app|window|webview|on_event)$/;

interface RustParam {
  name: string;
  optional: boolean;
}

/** Split a parameter list on top-level commas (generics contain commas too). */
function splitParams(src: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let cur = '';
  for (const ch of src) {
    if (ch === '<' || ch === '(' || ch === '[') depth++;
    if (ch === '>' || ch === ')' || ch === ']') depth--;
    if (ch === ',' && depth === 0) {
      out.push(cur);
      cur = '';
      continue;
    }
    cur += ch;
  }
  if (cur.trim()) out.push(cur);
  return out;
}

function parseRustCommands(): Map<string, RustParam[]> {
  const cmds = new Map<string, RustParam[]>();
  const re = /#\[tauri::command\]([\s\S]*?)\bfn\s+(\w+)\s*\(([\s\S]*?)\)\s*(?:->|\{)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(RUST)) !== null) {
    const [, between, name, params] = m;
    // Only attributes may sit between the marker and the fn; anything else
    // means the regex ran past the command it matched.
    if (/\bfn\b/.test(between)) continue;
    const parsed: RustParam[] = [];
    for (const raw of splitParams(params)) {
      const p = raw.trim();
      if (!p) continue;
      const colon = p.indexOf(':');
      if (colon < 0) continue;
      const pname = p.slice(0, colon).trim().replace(/^mut\s+/, '');
      const ty = p.slice(colon + 1).trim();
      if (INJECTED.test(pname)) continue;
      if (/^tauri::ipc::Channel|^Channel</.test(ty)) continue;
      if (/^State</.test(ty)) continue;
      parsed.push({ name: pname, optional: /^Option</.test(ty) });
    }
    cmds.set(name, parsed);
  }
  return cmds;
}

const snakeToCamel = (s: string) => s.replace(/_([a-z])/g, (_, c) => c.toUpperCase());

interface Call {
  method: string;
  command: string;
  /** arg-object key -> the expression it is bound to */
  args: Map<string, string>;
  /** TS parameter name -> may it be undefined at runtime */
  tsParams: Map<string, boolean>;
}

function parseBridgeCalls(): Call[] {
  const calls: Call[] = [];
  // Each `async name(params) { … invoke…('cmd', { … }) … }`
  const methodRe = /\basync\s+(\w+)\s*\(([\s\S]*?)\)\s*:?[^{]*\{/g;
  let m: RegExpExecArray | null;
  while ((m = methodRe.exec(BRIDGE)) !== null) {
    const [, method, paramSrc] = m;
    const body = BRIDGE.slice(m.index, m.index + 1400);
    const invoke = body.match(
      /(?:this\.invoke|this\.channelInvoke)<[\s\S]*?>\(\s*'([\w]+)'\s*,\s*\{([\s\S]*?)\}/,
    );
    if (!invoke) continue;
    const [, command, argSrc] = invoke;

    const tsParams = new Map<string, boolean>();
    for (const raw of splitParams(paramSrc)) {
      const p = raw.trim();
      if (!p) continue;
      const nameMatch = p.match(/^(\w+)(\?)?\s*:/);
      if (!nameMatch) continue;
      const [, pname, question] = nameMatch;
      // Optional AND undefaulted is the only shape that can be undefined.
      const hasDefault = /=/.test(p);
      tsParams.set(pname, Boolean(question) && !hasDefault);
    }

    const args = new Map<string, string>();
    for (const raw of splitParams(argSrc)) {
      const a = raw.trim();
      if (!a || a.startsWith('...')) continue;
      const colon = a.indexOf(':');
      if (colon < 0) args.set(a, a); // shorthand { foo }
      else args.set(a.slice(0, colon).trim(), a.slice(colon + 1).trim());
    }
    calls.push({ method, command, args, tsParams });
  }
  return calls;
}

const commands = parseRustCommands();
const calls = parseBridgeCalls();

describe('AI command contract: aiBridge.ts vs commands.rs', () => {
  it('parses both sides, or the guard is silently vacuous', () => {
    // A regex that stops matching turns every assertion below into a no-op.
    expect(commands.size).toBeGreaterThan(20);
    expect(calls.length).toBeGreaterThan(10);
    expect(commands.has('ai_citation_need')).toBe(true);
    expect(calls.some((c) => c.command === 'ai_citation_need')).toBe(true);
  });

  it('every command the bridge calls exists in Rust', () => {
    const missing = calls.filter((c) => !commands.has(c.command));
    expect(
      missing.map((c) => `${c.method} -> ${c.command}`),
      'bridge calls a command that no #[tauri::command] defines',
    ).toEqual([]);
  });

  it('every REQUIRED Rust parameter is passed a value that cannot be undefined', () => {
    const problems: string[] = [];
    for (const call of calls) {
      const params = commands.get(call.command);
      if (!params) continue;
      for (const p of params) {
        if (p.optional) continue;
        const key = snakeToCamel(p.name);
        const bound = call.args.get(key) ?? call.args.get(p.name);
        if (bound === undefined) {
          problems.push(
            `${call.command}: required \`${p.name}\` is never passed by ${call.method}()`,
          );
          continue;
        }
        // Shorthand `{ section }` forwards a TS parameter — which may itself be
        // optional and undefined. THIS is the case that shipped broken.
        if (call.tsParams.get(bound) === true) {
          problems.push(
            `${call.command}: required \`${p.name}\` is fed by optional TS param ` +
              `\`${bound}?\` in ${call.method}() — undefined serializes to a missing key`,
          );
        }
      }
    }
    expect(problems, problems.join('\n')).toEqual([]);
  });

  it('the bridge never passes a key the command does not declare', () => {
    // A stray key is silently ignored by Tauri, so a renamed parameter looks
    // like it works until the value turns out to be absent on the Rust side.
    const problems: string[] = [];
    for (const call of calls) {
      const params = commands.get(call.command);
      if (!params) continue;
      const known = new Set(params.flatMap((p) => [snakeToCamel(p.name), p.name]));
      for (const key of Array.from(call.args.keys())) {
        if (!known.has(key)) {
          problems.push(`${call.command}: ${call.method}() sends unknown key \`${key}\``);
        }
      }
    }
    expect(problems, problems.join('\n')).toEqual([]);
  });
});
