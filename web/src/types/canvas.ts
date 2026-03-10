export interface CanvasBlock {
  id: string;
  type: 'code' | 'markdown' | 'chart' | 'form' | 'text';
  content: string;
  language?: string;  // for code blocks
  metadata?: Record<string, unknown>;
  timestamp: number;
}

/** JSON Schema subset used for form block field definitions */
export interface FormFieldSchema {
  name: string;
  label?: string;
  type: 'text' | 'number' | 'boolean' | 'select';
  required?: boolean;
  options?: string[];          // for 'select' type
  defaultValue?: string | number | boolean;
}

/** Metadata shape for 'form' blocks */
export interface FormBlockMeta {
  fields: FormFieldSchema[];
  submitLabel?: string;
}

/** Metadata shape for 'chart' blocks */
export interface ChartBlockMeta {
  chartType?: 'bar' | 'line' | 'pie' | 'scatter';
  title?: string;
  xLabel?: string;
  yLabel?: string;
}

/**
 * Directive marker syntax used inside agent messages:
 *   [canvas:code lang=typescript]...[/canvas]
 *   [canvas:markdown]...[/canvas]
 *   [canvas:chart]{"labels":[...],"data":[...]}[/canvas]
 *   [canvas:form]{"fields":[...]}[/canvas]
 */
export const CANVAS_OPEN_RE = /\[canvas:(\w+)([^\]]*)\]/g;
export const CANVAS_CLOSE = '[/canvas]';
