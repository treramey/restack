/**
 * Fetch wrapper for /api/* endpoints.
 * Centralises error handling and JSON parsing.
 */

const API_BASE = "";

export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
    public readonly body: unknown,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export async function apiFetch<T>(
  path: string,
  init?: RequestInit,
): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, init);

  if (!res.ok) {
    const body: unknown = await res.json().catch(() => ({}));
    const message =
      body !== null &&
      typeof body === "object" &&
      "error" in body &&
      typeof (body as Record<string, unknown>)["error"] === "string"
        ? (body as { error: string }).error
        : `API ${res.status}: ${res.statusText}`;
    throw new ApiError(message, res.status, body);
  }

  return res.json() as Promise<T>;
}
