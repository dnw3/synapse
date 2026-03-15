import { useState, useEffect } from "react";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import { oneLight } from "react-syntax-highlighter/dist/esm/styles/prism";

/** Detect dark/light mode from data-theme attribute. */
function useIsDarkMode() {
  const [isDark, setIsDark] = useState(
    () => document.documentElement.getAttribute("data-theme") !== "light"
  );
  useEffect(() => {
    const observer = new MutationObserver(() => {
      setIsDark(document.documentElement.getAttribute("data-theme") !== "light");
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);
  return isDark;
}

/** Returns the appropriate syntax highlighter theme based on current color mode. */
export function useCodeTheme() {
  const isDark = useIsDarkMode();
  return isDark ? oneDark : oneLight;
}
