/**
 * PasteCanvas TypeScript contracts — Zod schemas matching Rust DTOs.
 * Runtime validation for all data crossing the Tauri command boundary.
 */
import { z } from "zod";

// --- Command Error ---

export const CommandErrorCodeSchema = z.enum([
  "VALIDATION_ERROR",
  "CLIPBOARD_BUSY",
  "UNSUPPORTED_CONTENT",
  "CONTENT_TOO_LARGE",
  "ASSET_WRITE_FAILED",
  "DATABASE_ERROR",
  "REVISION_CONFLICT",
  "HOTKEY_CONFLICT",
  "PERMISSION_DENIED",
  "NOT_FOUND",
  "INTEGRITY_ERROR",
  "INTERNAL_ERROR",
]);

export type CommandErrorCode = z.infer<typeof CommandErrorCodeSchema>;

export const CommandErrorSchema = z.object({
  code: CommandErrorCodeSchema,
  message: z.string(),
  retryable: z.boolean(),
  correlationId: z.string(),
  details: z.record(z.unknown()).optional(),
});

export type CommandError = z.infer<typeof CommandErrorSchema>;

// --- Health ---

export const HealthResponseSchema = z.object({
  status: z.string(),
  version: z.string(),
  correlationId: z.string(),
});

export type HealthResponse = z.infer<typeof HealthResponseSchema>;

// --- Common value types ---

export const EntityIdSchema = z.string().uuid();
export type EntityId = z.infer<typeof EntityIdSchema>;

export const UnixMsSchema = z.number().int().positive();
export type UnixMs = z.infer<typeof UnixMsSchema>;

export const ItemTypeSchema = z.enum([
  "plain_text",
  "rich_text",
  "code",
  "image",
  "screenshot",
  "link",
  "video_link",
  "pdf",
  "color",
  "font",
  "svg",
  "file",
]);

export type ItemType = z.infer<typeof ItemTypeSchema>;

export const InboxStatusSchema = z.enum(["new", "triaged", "archived"]);
export type InboxStatus = z.infer<typeof InboxStatusSchema>;

export const CaptureMethodSchema = z.enum([
  "manual_hotkey",
  "quick_capture",
  "capture_session",
  "canvas_paste",
  "drag_drop",
  "screenshot",
  "explorer",
  "browser",
  "import",
  "manual_create",
]);

export type CaptureMethod = z.infer<typeof CaptureMethodSchema>;

/** Capture command response — matches Rust CaptureTextCommandResponse. */
export const CaptureResponseSchema = z.object({
  itemId: z.string(),
  committedAt: z.number(),
  isDuplicate: z.boolean(),
  /** ID of existing item on duplicate (INB-004); null when not a duplicate. */
  duplicateOf: z.string().nullable(),
});

export type CaptureResponse = z.infer<typeof CaptureResponseSchema>;
