interface Props {
  screenshot: string | null;
}

export default function BrowserView({ screenshot }: Props) {
  if (!screenshot) {
    return (
      <div className="flex items-center justify-center h-full text-sm text-[var(--text-secondary)]">
        No browser session active.
        <br />
        The agent will show browser screenshots here when using Chrome DevTools.
      </div>
    );
  }

  return (
    <div className="h-full flex items-center justify-center bg-[var(--bg-primary)] p-2">
      <img
        src={`data:image/png;base64,${screenshot}`}
        alt="Browser screenshot"
        className="max-w-full max-h-full object-contain rounded border border-[var(--border)]"
      />
    </div>
  );
}
