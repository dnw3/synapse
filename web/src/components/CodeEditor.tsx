import { useCallback, useState } from 'react';
import Editor from '@monaco-editor/react';

interface CodeEditorProps {
  /** Initial file path to load */
  filePath?: string;
  /** Language mode */
  language?: string;
  /** Initial content */
  initialContent?: string;
  /** Called when user saves (Ctrl+S) */
  onSave?: (content: string) => void;
}

export function CodeEditor({ filePath, language, initialContent = '', onSave }: CodeEditorProps) {
  const [content, setContent] = useState(initialContent);
  const [isDirty, setIsDirty] = useState(false);
  const isDark = document.documentElement.getAttribute("data-theme") !== "light";

  const handleChange = useCallback((value: string | undefined) => {
    if (value !== undefined) {
      setContent(value);
      setIsDirty(true);
    }
  }, []);

  const handleSave = useCallback(() => {
    if (onSave) {
      onSave(content);
      setIsDirty(false);
    }
  }, [content, onSave]);

  // Detect language from file extension
  const detectLanguage = (path?: string): string => {
    if (language) return language;
    if (!path) return 'plaintext';
    const ext = path.split('.').pop()?.toLowerCase();
    const langMap: Record<string, string> = {
      rs: 'rust',
      ts: 'typescript',
      tsx: 'typescript',
      js: 'javascript',
      jsx: 'javascript',
      py: 'python',
      json: 'json',
      toml: 'toml',
      yaml: 'yaml',
      yml: 'yaml',
      md: 'markdown',
      css: 'css',
      html: 'html',
      sql: 'sql',
      sh: 'shell',
      bash: 'shell',
      dockerfile: 'dockerfile',
    };
    return langMap[ext || ''] || 'plaintext';
  };

  return (
    <div className="flex flex-col h-full bg-[var(--bg-content)] border border-[var(--separator)] rounded-[var(--radius-md)] overflow-hidden">
      <div className="flex items-center justify-between px-3 py-1.5 bg-[var(--bg-grouped)] border-b border-[var(--border-subtle)] text-sm flex-shrink-0">
        <span className="text-[var(--text-secondary)] font-mono text-xs font-medium">
          {filePath || 'untitled'}
          {isDirty && <span className="text-[var(--warning)] ml-1">{'\u25cf'}</span>}
        </span>
        <button
          onClick={handleSave}
          disabled={!isDirty}
          className="px-2 py-0.5 text-xs rounded-[var(--radius-sm)] bg-[var(--accent)] text-white disabled:opacity-40 hover:opacity-90 transition-colors"
        >
          Save
        </button>
      </div>
      <div className="flex-1 min-h-0">
        <Editor
          height="100%"
          language={detectLanguage(filePath)}
          value={content}
          onChange={handleChange}
          theme={isDark ? "vs-dark" : "light"}
          options={{
            minimap: { enabled: false },
            fontSize: 13,
            lineNumbers: 'on',
            wordWrap: 'on',
            scrollBeyondLastLine: false,
            automaticLayout: true,
            tabSize: 2,
          }}
          onMount={(editor) => {
            // Register Ctrl+S / Cmd+S keybinding
            editor.addCommand(
              // Monaco.KeyMod.CtrlCmd | Monaco.KeyCode.KeyS
              // eslint-disable-next-line no-bitwise
              2048 | 49,
              () => handleSave()
            );
          }}
        />
      </div>
      <div className="flex items-center px-3 h-6 bg-[var(--bg-grouped)] border-t border-[var(--border-subtle)] flex-shrink-0">
        <span className="text-[10px] text-[var(--text-tertiary)] font-mono">
          {detectLanguage(filePath)}
        </span>
      </div>
    </div>
  );
}

export default CodeEditor;
