import type { ZodType } from "zod";

const BASE = "/api/dashboard";

export class ApiError extends Error {
  constructor(
    public status: number,
    public statusText: string,
    public body?: unknown,
  ) {
    super(`${status} ${statusText}`);
  }
}

function parseWithSchema<T>(data: unknown, schema?: ZodType<T>): T {
  if (schema) return schema.parse(data);
  return data as T;
}

export async function fetchJSON<T>(path: string, schema?: ZodType<T>): Promise<T> {
  const res = await fetch(`${BASE}${path}`);
  if (!res.ok) throw new ApiError(res.status, res.statusText);
  const data: unknown = await res.json().catch(() => {
    throw new ApiError(res.status, "Invalid JSON response");
  });
  return parseWithSchema(data, schema);
}

export async function mutateJSON<T>(
  method: string,
  path: string,
  body?: unknown,
  schema?: ZodType<T>,
): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method,
    headers: { "Content-Type": "application/json" },
    ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
  });
  if (!res.ok) {
    throw new ApiError(
      res.status,
      res.statusText,
      await res.json().catch(() => null),
    );
  }
  const contentType = res.headers.get("content-type") ?? "";
  if (!contentType.includes("application/json")) {
    return undefined as unknown as T;
  }
  const data: unknown = await res.json();
  return parseWithSchema(data, schema);
}

export const postJSON = <T,>(path: string, body?: unknown, schema?: ZodType<T>) =>
  mutateJSON<T>("POST", path, body, schema);
export const putJSON = <T,>(path: string, body: unknown, schema?: ZodType<T>) =>
  mutateJSON<T>("PUT", path, body, schema);
export const patchJSON = <T,>(path: string, body: unknown, schema?: ZodType<T>) =>
  mutateJSON<T>("PATCH", path, body, schema);
export const deleteJSON = (path: string) =>
  mutateJSON<void>("DELETE", path);

export async function fetchRaw(path: string): Promise<Response> {
  const res = await fetch(`${BASE}${path}`);
  if (!res.ok) throw new ApiError(res.status, res.statusText);
  return res;
}
