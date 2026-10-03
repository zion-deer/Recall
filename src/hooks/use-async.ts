import { useEffect, useState, type DependencyList } from "react";
import { toRecallError } from "@/lib/api";

export type AsyncState<T> =
  | { status: "loading"; data?: T }
  | { status: "error"; error: string; data?: T }
  | { status: "ready"; data: T };

/**
 * Runs `fn` whenever `deps` change. Keeps the previous data while reloading so
 * live updates don't flash a loading state.
 */
export function useAsync<T>(fn: () => Promise<T>, deps: DependencyList): AsyncState<T> {
  const [state, setState] = useState<AsyncState<T>>({ status: "loading" });

  // eslint-disable-next-line react-hooks/exhaustive-deps -- callers pass the deps that `fn` closes over
  useEffect(() => {
    let cancelled = false;
    setState((s) => ({ status: "loading", data: s.data }));
    fn().then(
      (data) => !cancelled && setState({ status: "ready", data }),
      (e) =>
        !cancelled &&
        setState((s) => ({ status: "error", error: toRecallError(e).message, data: s.data })),
    );
    return () => {
      cancelled = true;
    };
  }, deps);

  return state;
}
