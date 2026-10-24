// Gaply — one generation counter per project screen (Research OS, Phase
// 5B-3b; decision 148). Every project read and every action on a screen takes
// the next number when it starts, and applies its response only while that
// number is still the newest and the screen is still shown. So a late answer,
// such as a project_current that lands after an Open, never overwrites the
// state the newer call made.
//
// Leaving the screen counts as newer than everything: its cleanup moves the
// counter on, so nothing applies after the screen has gone. Under React
// StrictMode the first, discarded mount's reads are dropped the same way.
import { useEffect, useMemo, useRef } from 'react';

export interface Generation {
  /** A read or an action starts: its number. */
  next(): number;
  /** Whether `n` is still the newest, on a screen still shown. */
  is(n: number): boolean;
}

export function useGeneration(): Generation {
  const n = useRef(0);
  useEffect(
    () => () => {
      n.current += 1;
    },
    []
  );
  return useMemo(
    () => ({
      next: () => {
        n.current += 1;
        return n.current;
      },
      is: (g: number) => g === n.current,
    }),
    []
  );
}
