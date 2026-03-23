import { describe, it, expect, vi, beforeEach } from "vitest";
import { z } from "zod";
import {
  fetchJSON,
  mutateJSON,
  postJSON,
  putJSON,
  patchJSON,
  deleteJSON,
  fetchRaw,
  ApiError,
} from "./api";

const TestSchema = z.object({ id: z.number(), name: z.string() });

function mockFetch(
  overrides: Partial<Response> & { json?: () => Promise<unknown> } = {},
) {
  const res = {
    ok: true,
    status: 200,
    statusText: "OK",
    headers: new Headers({ "content-type": "application/json" }),
    json: () => Promise.resolve({}),
    ...overrides,
  } as Response;
  globalThis.fetch = vi.fn().mockResolvedValue(res);
  return globalThis.fetch as ReturnType<typeof vi.fn>;
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("fetchJSON", () => {
  it("returns parsed JSON without schema", async () => {
    mockFetch({ json: () => Promise.resolve({ foo: "bar" }) });
    const result = await fetchJSON("/test");
    expect(result).toEqual({ foo: "bar" });
  });

  it("validates with schema when provided", async () => {
    mockFetch({ json: () => Promise.resolve({ id: 1, name: "alice" }) });
    const result = await fetchJSON("/test", TestSchema);
    expect(result).toEqual({ id: 1, name: "alice" });
  });

  it("throws ZodError on schema mismatch", async () => {
    mockFetch({ json: () => Promise.resolve({ id: "not-a-number" }) });
    await expect(fetchJSON("/test", TestSchema)).rejects.toThrow(z.ZodError);
  });

  it("throws ApiError on non-OK response", async () => {
    mockFetch({ ok: false, status: 404, statusText: "Not Found" });
    await expect(fetchJSON("/test")).rejects.toThrow(ApiError);
    await expect(fetchJSON("/test")).rejects.toThrow("404 Not Found");
  });

  it("wraps malformed JSON in ApiError", async () => {
    mockFetch({
      json: () => Promise.reject(new SyntaxError("Unexpected token")),
    });
    await expect(fetchJSON("/test")).rejects.toThrow(ApiError);
    await expect(fetchJSON("/test")).rejects.toThrow("Invalid JSON response");
  });

  it("prepends /api/dashboard to path", async () => {
    const mock = mockFetch({});
    await fetchJSON("/stats");
    expect(mock).toHaveBeenCalledWith("/api/dashboard/stats");
  });
});

describe("mutateJSON", () => {
  it("sends correct method and body", async () => {
    const mock = mockFetch({
      json: () => Promise.resolve({ ok: true }),
    });
    await mutateJSON("POST", "/items", { name: "test" });
    expect(mock).toHaveBeenCalledWith("/api/dashboard/items", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "test" }),
    });
  });

  it("omits body key when body is undefined", async () => {
    const mock = mockFetch({
      json: () => Promise.resolve({}),
    });
    await mutateJSON("DELETE", "/items/1");
    expect(mock).toHaveBeenCalledWith("/api/dashboard/items/1", {
      method: "DELETE",
      headers: { "Content-Type": "application/json" },
    });
  });

  it("validates response with schema", async () => {
    mockFetch({ json: () => Promise.resolve({ id: 2, name: "bob" }) });
    const result = await mutateJSON("POST", "/items", {}, TestSchema);
    expect(result).toEqual({ id: 2, name: "bob" });
  });

  it("returns undefined for non-JSON response", async () => {
    mockFetch({
      headers: new Headers({ "content-type": "text/plain" }),
    });
    const result = await mutateJSON("POST", "/items", { x: 1 });
    expect(result).toBeUndefined();
  });

  it("throws ApiError with body on error response", async () => {
    mockFetch({
      ok: false,
      status: 422,
      statusText: "Unprocessable Entity",
      json: () => Promise.resolve({ error: "validation failed" }),
    });
    try {
      await mutateJSON("POST", "/items", {});
      expect.unreachable("should have thrown");
    } catch (e) {
      expect(e).toBeInstanceOf(ApiError);
      const err = e as ApiError;
      expect(err.status).toBe(422);
      expect(err.statusText).toBe("Unprocessable Entity");
      expect(err.body).toEqual({ error: "validation failed" });
    }
  });

  it("sets body to null when error response has invalid JSON", async () => {
    mockFetch({
      ok: false,
      status: 500,
      statusText: "Internal Server Error",
      json: () => Promise.reject(new SyntaxError("bad json")),
    });
    try {
      await mutateJSON("POST", "/items", {});
      expect.unreachable("should have thrown");
    } catch (e) {
      expect(e).toBeInstanceOf(ApiError);
      expect((e as ApiError).body).toBeNull();
    }
  });
});

describe("convenience methods", () => {
  it("postJSON uses POST", async () => {
    const mock = mockFetch({ json: () => Promise.resolve({}) });
    await postJSON("/items", { a: 1 });
    expect(mock).toHaveBeenCalledWith(
      "/api/dashboard/items",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("putJSON uses PUT", async () => {
    const mock = mockFetch({ json: () => Promise.resolve({}) });
    await putJSON("/items/1", { a: 1 });
    expect(mock).toHaveBeenCalledWith(
      "/api/dashboard/items/1",
      expect.objectContaining({ method: "PUT" }),
    );
  });

  it("patchJSON uses PATCH", async () => {
    const mock = mockFetch({ json: () => Promise.resolve({}) });
    await patchJSON("/items/1", { a: 1 });
    expect(mock).toHaveBeenCalledWith(
      "/api/dashboard/items/1",
      expect.objectContaining({ method: "PATCH" }),
    );
  });

  it("deleteJSON uses DELETE", async () => {
    const mock = mockFetch({ json: () => Promise.resolve({}) });
    await deleteJSON("/items/1");
    expect(mock).toHaveBeenCalledWith(
      "/api/dashboard/items/1",
      expect.objectContaining({ method: "DELETE" }),
    );
  });
});

describe("fetchRaw", () => {
  it("returns raw Response on success", async () => {
    mockFetch({});
    const res = await fetchRaw("/file");
    expect(res).toBeDefined();
    expect(res.ok).toBe(true);
  });

  it("throws ApiError on non-OK response", async () => {
    mockFetch({ ok: false, status: 403, statusText: "Forbidden" });
    await expect(fetchRaw("/file")).rejects.toThrow(ApiError);
    await expect(fetchRaw("/file")).rejects.toThrow("403 Forbidden");
  });

  it("prepends /api/dashboard to path", async () => {
    const mock = mockFetch({});
    await fetchRaw("/download");
    expect(mock).toHaveBeenCalledWith("/api/dashboard/download");
  });
});

describe("ApiError", () => {
  it("has correct message format", () => {
    const err = new ApiError(500, "Internal Server Error");
    expect(err.message).toBe("500 Internal Server Error");
    expect(err).toBeInstanceOf(Error);
  });

  it("stores body when provided", () => {
    const err = new ApiError(400, "Bad Request", { detail: "missing field" });
    expect(err.body).toEqual({ detail: "missing field" });
  });
});
