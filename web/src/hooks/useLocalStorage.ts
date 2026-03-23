// web/src/hooks/useLocalStorage.ts
import { useState, useCallback } from "react";

export function useLocalStorage<T>(
  key: string,
  defaultValue: T,
): [T, (v: T) => void] {
  const [value, setValue] = useState<T>(() => {
    try {
      const stored = localStorage.getItem(key);
      return stored ? JSON.parse(stored) : defaultValue;
    } catch {
      return defaultValue;
    }
  });

  const set = useCallback(
    (v: T) => {
      setValue(v);
      try {
        localStorage.setItem(key, JSON.stringify(v));
      } catch {
        // QuotaExceededError in private browsing — state still updates
      }
    },
    [key],
  );

  return [value, set];
}
