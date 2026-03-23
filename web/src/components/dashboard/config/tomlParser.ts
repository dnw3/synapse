// Schema types matching backend
export interface ConfigFieldSchema {
  key: string;
  label: string;
  type: "string" | "number" | "boolean" | "enum" | "array" | "secret";
  description?: string;
  placeholder?: string;
  options?: string[];
  default_value?: string;
  sensitive: boolean;
}

export interface ConfigSectionSchema {
  key: string;
  label: string;
  description?: string;
  order: number;
  icon: string;
  fields: ConfigFieldSchema[];
}

export interface ConfigSchemaData {
  sections: ConfigSectionSchema[];
  sensitive_patterns: string[];
  provider_defaults?: Record<string, string>;
}

// TOML parsing types
export interface TomlField {
  key: string;
  value: string;
  line: number;
  // For multi-line values, endLine tracks the last line
  endLine: number;
}

export interface TomlSection {
  key: string;
  fields: TomlField[];
  isArrayTable?: boolean;
}

// --- TOML helpers ---

/** Parse TOML into sections. Handles multi-line arrays/strings. */
export function parseTomlSections(content: string): TomlSection[] {
  const lines = content.split("\n");
  const sections: TomlSection[] = [];
  let current: TomlSection | null = null;
  let multiLineKey: string | null = null;
  let multiLineValue = "";
  let multiLineStart = 0;
  let bracketDepth = 0;

  function pushField(section: TomlSection, key: string, value: string, startLine: number, endLine: number) {
    section.fields.push({ key, value: value.trim(), line: startLine, endLine });
  }

  function ensureSection(): TomlSection {
    if (current) return current;
    if (!sections.length || sections[0].key !== "(root)") {
      const root: TomlSection = { key: "(root)", fields: [] };
      sections.unshift(root);
      current = root;
    } else {
      current = sections[0];
    }
    return current;
  }

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();

    // If we're accumulating a multi-line value
    if (multiLineKey !== null) {
      multiLineValue += "\n" + lines[i];
      // Count brackets
      for (const ch of line) {
        if (ch === "[") bracketDepth++;
        if (ch === "]") bracketDepth--;
      }
      if (bracketDepth <= 0) {
        // Multi-line value complete
        const section = ensureSection();
        pushField(section, multiLineKey, multiLineValue.trim(), multiLineStart, i);
        multiLineKey = null;
        multiLineValue = "";
        bracketDepth = 0;
      }
      continue;
    }

    // Section header: [[array_table]] (TOML array tables)
    const arrayTableMatch = line.match(/^\[\[([^[\]]+)\]\]$/);
    if (arrayTableMatch) {
      current = { key: arrayTableMatch[1], fields: [], isArrayTable: true };
      sections.push(current);
      continue;
    }

    // Section header: [name] or [name.sub]
    const sectionMatch = line.match(/^\[([^[\]]+)\]$/);
    if (sectionMatch) {
      current = { key: sectionMatch[1], fields: [] };
      sections.push(current);
      continue;
    }

    // Skip blanks and comments
    if (!line || line.startsWith("#")) continue;

    // Key-value pair
    const kvMatch = line.match(/^([^=]+?)\s*=\s*(.*)$/);
    if (kvMatch) {
      const key = kvMatch[1].trim();
      const value = kvMatch[2].trim();

      // Check if value starts a multi-line array
      if (value.startsWith("[") && !value.endsWith("]")) {
        multiLineKey = key;
        multiLineValue = value;
        multiLineStart = i;
        bracketDepth = 0;
        for (const ch of value) {
          if (ch === "[") bracketDepth++;
          if (ch === "]") bracketDepth--;
        }
        const section = ensureSection();
        // Don't push yet — wait for closing bracket
        void section;
        continue;
      }

      // Check for multi-line basic string (triple quotes)
      if (value === '"""' || value.startsWith('"""')) {
        if (value !== '"""' && value.endsWith('"""') && value.length > 3) {
          // Single-line triple-quoted
          const section = ensureSection();
          pushField(section, key, value, i, i);
        } else {
          void key; // multiLineKey will be used when multi-line string support is completed
          multiLineValue = value;
          multiLineStart = i;
          // We'll look for closing """ later
          // For simplicity, just treat as single line
          const section = ensureSection();
          pushField(section, key, value, i, i);
          multiLineKey = null;
        }
        continue;
      }

      const section = ensureSection();
      pushField(section, key, value, i, i);
    }
  }

  // If we still have an unclosed multi-line value, push it as-is
  if (multiLineKey !== null) {
    const section = ensureSection();
    pushField(section, multiLineKey, multiLineValue.trim(), multiLineStart, lines.length - 1);
  }

  return sections;
}

export function unquote(v: string): string {
  if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) {
    return v.slice(1, -1);
  }
  return v;
}

/** Format a TOML array value for display. */
export function formatArrayValue(v: string): string {
  // Try to extract array items for readable display
  const match = v.match(/^\[([\s\S]*)\]$/);
  if (!match) return v;
  const inner = match[1].trim();
  if (!inner) return "[]";
  // Extract quoted strings
  const items: string[] = [];
  const re = /"([^"]+)"/g;
  let m;
  while ((m = re.exec(inner)) !== null) {
    items.push(m[1]);
  }
  if (items.length > 0) return items.join(", ");
  return inner;
}

export function requote(original: string, updated: string): string {
  if (original.startsWith('"') && original.endsWith('"')) return `"${updated}"`;
  if (original.startsWith("'") && original.endsWith("'")) return `'${updated}'`;
  return updated;
}

// Keys that should NOT be treated as sensitive despite matching patterns
const SENSITIVE_WHITELIST = new Set([
  "max_tokens", "max_output_tokens", "token_count", "total_tokens",
  "tokens_per_minute", "token_budget", "compact_threshold",
]);

export function isSensitiveKey(key: string, patterns: string[]): boolean {
  const lower = key.toLowerCase();
  if (SENSITIVE_WHITELIST.has(lower)) return false;
  return patterns.some((p) => lower.includes(p)) && !lower.endsWith("_env");
}
