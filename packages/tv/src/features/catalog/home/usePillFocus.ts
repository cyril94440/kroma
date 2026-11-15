import { useCallback, useEffect, useLayoutEffect, useRef } from 'react';

interface Focusable {
  focus(): void;
}

const RESTORE_DELAY_MS = 150;

/** Keeps the remote on the same item of the top bar when the bar is rebuilt
 *  because `order` changed (an entry arrived after the first paint): the
 *  rebuilt pill would otherwise walk the focus off to a neighbour. The item is
 *  read before the rebuild starts, because the rebuild itself moves the focus. */
export function usePillFocus(order: string) {
  const held = useRef<string | null>(null);
  const restore = useRef<string | null>(null);
  const handles = useRef(new Map<string, Focusable>());
  useLayoutEffect(
    () => () => {
      restore.current = held.current;
    },
    [order],
  );
  useEffect(() => {
    const key = restore.current;
    if (key === null) return;
    const timer = setTimeout(() => {
      handles.current.get(key)?.focus();
      restore.current = null;
    }, RESTORE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [order]);
  return useCallback(
    (key: string) => ({
      onFocus: () => {
        held.current = key;
      },
      onBlur: () => {
        held.current = null;
      },
      focusHandle: (handle: Focusable | null) => {
        if (handle) handles.current.set(key, handle);
        else handles.current.delete(key);
      },
    }),
    [],
  );
}
