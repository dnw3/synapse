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

export async function fetchJSON<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE}${path}`);
  if (!res.ok) throw new ApiError(res.status, res.statusText);
  return res.json();
}

export async function mutateJSON<T>(
  method: string,
  path: string,
  body?: unknown,
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
  return res.json();
}

export const postJSON = <T,>(path: string, body?: unknown) =>
  mutateJSON<T>("POST", path, body);
export const putJSON = <T,>(path: string, body: unknown) =>
  mutateJSON<T>("PUT", path, body);
export const patchJSON = <T,>(path: string, body: unknown) =>
  mutateJSON<T>("PATCH", path, body);
export const deleteJSON = (path: string) =>
  mutateJSON<void>("DELETE", path);

export async function fetchRaw(path: string): Promise<Response> {
  const res = await fetch(`${BASE}${path}`);
  if (!res.ok) throw new ApiError(res.status, res.statusText);
  return res;
}
