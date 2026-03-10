import { useEffect, useRef } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

interface Props {
  conversationId: string | null;
}

export default function Terminal({ conversationId }: Props) {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    if (!terminalRef.current) return;

    const style = getComputedStyle(document.documentElement);
    const bgColor = style.getPropertyValue("--bg-primary").trim() || "#0a0a0f";
    const fgColor = style.getPropertyValue("--text-primary").trim() || "#e4e4ef";
    const cursorColor = style.getPropertyValue("--accent").trim() || "#6366f1";
    const selectionBg = (style.getPropertyValue("--accent").trim() || "#6366f1") + "40";

    const term = new XTerm({
      cursorBlink: true,
      fontSize: 12,
      fontFamily: '"JetBrains Mono", "Fira Code", monospace',
      theme: {
        background: bgColor,
        foreground: fgColor,
        cursor: cursorColor,
        selectionBackground: selectionBg,
      },
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(terminalRef.current);
    fitAddon.fit();

    xtermRef.current = term;

    // Build WebSocket URL
    const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl = `${proto}//${window.location.host}/ws/terminal`;

    term.writeln("Connecting to terminal...");

    const ws = new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      term.clear();
      term.focus();
    };

    ws.onmessage = (event) => {
      term.write(event.data);
    };

    ws.onerror = () => {
      term.writeln("\r\n\x1b[31mWebSocket error — could not connect to terminal backend.\x1b[0m");
    };

    ws.onclose = () => {
      term.writeln("\r\n\x1b[33mTerminal session closed.\x1b[0m");
    };

    // Send keystrokes over WebSocket
    term.onData((data) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(data);
      }
    });

    // Handle resize
    const observer = new ResizeObserver(() => {
      fitAddon.fit();
    });
    observer.observe(terminalRef.current);

    return () => {
      observer.disconnect();
      ws.close();
      term.dispose();
      wsRef.current = null;
      xtermRef.current = null;
    };
  }, [conversationId]);

  return (
    <div className="h-full bg-[var(--bg-primary)]">
      <div ref={terminalRef} className="h-full" />
    </div>
  );
}
