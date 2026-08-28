import { HertaError } from "./errors";
import type { AuthScope, QueryValue, RecordUpload, UploadFile } from "./types";

export function normalizeBaseUrl(baseUrl: string): string {
  const normalized = baseUrl.trim().replace(/\/+$/, "");
  if (!normalized) {
    throw new HertaError("baseUrl must not be empty", { kind: "configuration" });
  }
  try {
    return new URL(normalized).toString().replace(/\/$/, "");
  } catch (cause) {
    throw new HertaError(`Invalid baseUrl: ${baseUrl}`, { kind: "configuration", cause });
  }
}

export function encodePath(value: string): string {
  return encodeURIComponent(value);
}

export function buildUrl(
  baseUrl: string,
  path: string,
  query?: Record<string, QueryValue>,
): string {
  if (!path.startsWith("/")) {
    throw new HertaError(`Request path must start with '/': ${path}`, {
      kind: "configuration",
    });
  }
  const url = new URL(`${baseUrl}${path}`);
  if (query) {
    for (const [key, raw] of Object.entries(query)) {
      if (raw === undefined || raw === null) continue;
      const value = Array.isArray(raw) ? raw.join(",") : String(raw);
      url.searchParams.set(key, value);
    }
  }
  return url.toString();
}

export function scopePath(scope: AuthScope): string {
  if (scope.kind === "admin") return "/api/admin/auth";
  if (scope.kind === "collection") return `/api/auth/${encodePath(scope.collection)}`;
  return "/api/auth";
}

export function sameScope(left: AuthScope, right: AuthScope): boolean {
  return (
    left.kind === right.kind &&
    (left.kind !== "collection" ||
      (right.kind === "collection" && left.collection === right.collection))
  );
}

export function uploadForm<TData extends object>(upload: RecordUpload<TData>): FormData {
  const form = new FormData();
  if (upload.data !== undefined) form.append("data", JSON.stringify(upload.data));
  for (const [field, rawFiles] of Object.entries(upload.files)) {
    const files = Array.isArray(rawFiles) ? rawFiles : [rawFiles];
    for (const file of files as readonly UploadFile[]) {
      appendUploadFile(form, field, file);
    }
  }
  return form;
}

export function appendUploadFile(form: FormData, field: string, file: UploadFile): void {
  const descriptor = file instanceof Blob ? { blob: file } : file;
  const filename =
    descriptor.filename ??
    (typeof File !== "undefined" && descriptor.blob instanceof File
      ? descriptor.blob.name
      : "blob");
  form.append(field, descriptor.blob, filename);
}

export function authSessionExpiry(expiresIn: number): number {
  return Date.now() + Math.max(0, expiresIn) * 1_000;
}
