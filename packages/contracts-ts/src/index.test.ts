import { describe, it, expect } from "vitest";
import {
  CommandErrorSchema,
  HealthResponseSchema,
  ItemTypeSchema,
  InboxStatusSchema,
  CaptureMethodSchema,
  CaptureResponseSchema,
} from "./index";

describe("CommandErrorSchema", () => {
  it("accepts valid error payload from Rust", () => {
    const payload = {
      code: "VALIDATION_ERROR",
      message: "title exceeds 2000 bytes",
      retryable: false,
      correlationId: "0192c3d4-e5f6-7890-abcd-ef1234567890",
    };
    const result = CommandErrorSchema.safeParse(payload);
    expect(result.success).toBe(true);
  });

  it("accepts error with optional details", () => {
    const payload = {
      code: "CLIPBOARD_BUSY",
      message: "clipboard locked by another process",
      retryable: true,
      correlationId: "0192c3d4-e5f6-7890-abcd-ef1234567890",
      details: { attempt: 3, maxAttempts: 5 },
    };
    const result = CommandErrorSchema.safeParse(payload);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.retryable).toBe(true);
    }
  });

  it("rejects unknown error code", () => {
    const payload = {
      code: "UNKNOWN_CODE",
      message: "something",
      retryable: false,
      correlationId: "id",
    };
    const result = CommandErrorSchema.safeParse(payload);
    expect(result.success).toBe(false);
  });

  it("rejects missing required fields", () => {
    const payload = { code: "NOT_FOUND" };
    const result = CommandErrorSchema.safeParse(payload);
    expect(result.success).toBe(false);
  });
});

describe("HealthResponseSchema", () => {
  it("accepts valid health response", () => {
    const payload = {
      status: "ok",
      version: "0.1.0",
      correlationId: "0192c3d4-e5f6-7890-abcd-ef1234567890",
    };
    const result = HealthResponseSchema.safeParse(payload);
    expect(result.success).toBe(true);
  });

  it("rejects missing version", () => {
    const payload = { status: "ok", correlationId: "id" };
    const result = HealthResponseSchema.safeParse(payload);
    expect(result.success).toBe(false);
  });
});

describe("ItemTypeSchema", () => {
  it("accepts all P0 types", () => {
    const p0Types = ["plain_text", "image", "screenshot", "link", "pdf"];
    for (const t of p0Types) {
      expect(ItemTypeSchema.safeParse(t).success).toBe(true);
    }
  });

  it("rejects invalid type", () => {
    expect(ItemTypeSchema.safeParse("video").success).toBe(false);
    expect(ItemTypeSchema.safeParse("").success).toBe(false);
    expect(ItemTypeSchema.safeParse(123).success).toBe(false);
  });
});

describe("InboxStatusSchema", () => {
  it("accepts valid statuses", () => {
    expect(InboxStatusSchema.safeParse("new").success).toBe(true);
    expect(InboxStatusSchema.safeParse("triaged").success).toBe(true);
    expect(InboxStatusSchema.safeParse("archived").success).toBe(true);
  });

  it("rejects invalid status", () => {
    expect(InboxStatusSchema.safeParse("deleted").success).toBe(false);
  });
});

describe("CaptureMethodSchema", () => {
  it("accepts all methods", () => {
    const methods = [
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
    ];
    for (const m of methods) {
      expect(CaptureMethodSchema.safeParse(m).success).toBe(true);
    }
  });

  it("rejects unknown method", () => {
    expect(CaptureMethodSchema.safeParse("ai_generate").success).toBe(false);
  });
});

describe("CaptureResponseSchema", () => {
  it("accepts non-duplicate capture response", () => {
    const payload = {
      itemId: "0192c3d4-e5f6-7890-abcd-ef1234567890",
      committedAt: 1735689600000,
      isDuplicate: false,
      duplicateOf: null,
    };
    const result = CaptureResponseSchema.safeParse(payload);
    expect(result.success).toBe(true);
  });

  it("accepts duplicate capture response with existing id (INB-004)", () => {
    const payload = {
      itemId: "0192c3d4-e5f6-7890-abcd-ef1234567891",
      committedAt: 1735689600000,
      isDuplicate: true,
      duplicateOf: "0192c3d4-e5f6-7890-abcd-ef1234567890",
    };
    const result = CaptureResponseSchema.safeParse(payload);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.duplicateOf).toBe("0192c3d4-e5f6-7890-abcd-ef1234567890");
    }
  });

  it("rejects response missing duplicateOf field", () => {
    const payload = {
      itemId: "0192c3d4-e5f6-7890-abcd-ef1234567891",
      committedAt: 1735689600000,
      isDuplicate: false,
    };
    const result = CaptureResponseSchema.safeParse(payload);
    expect(result.success).toBe(false);
  });
});
