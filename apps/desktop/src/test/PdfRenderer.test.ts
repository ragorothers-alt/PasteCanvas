import { describe, it, expect } from "vitest";
import { base64ToUint8Array } from "../adapters/pdf/PdfRenderer";

describe("PdfRenderer base64ToUint8Array", () => {
  it("decodes base64 to correct bytes", () => {
    // "Hello" in base64 = "SGVsbG8="
    const bytes = base64ToUint8Array("SGVsbG8=");
    expect(bytes).toEqual(new Uint8Array([72, 101, 108, 108, 111]));
  });

  it("decodes empty string to empty array", () => {
    const bytes = base64ToUint8Array("");
    expect(bytes.length).toBe(0);
  });

  it("round-trips binary data", () => {
    // Build a base64 string from known bytes [0, 255, 128]
    const original = new Uint8Array([0, 255, 128]);
    let binary = "";
    for (const b of original) binary += String.fromCharCode(b);
    const base64 = btoa(binary);

    const decoded = base64ToUint8Array(base64);
    expect(decoded).toEqual(original);
  });

  it("handles PDF magic bytes", () => {
    // "%PDF" = [37, 80, 68, 70] → base64 "JVBERg=="... let's encode "%PDF-1.4"
    const pdfHeader = "%PDF-1.4";
    let binary = "";
    for (let i = 0; i < pdfHeader.length; i++) binary += pdfHeader[i];
    const base64 = btoa(binary);

    const decoded = base64ToUint8Array(base64);
    expect(decoded[0]).toBe(37); // '%'
    expect(decoded[1]).toBe(80); // 'P'
    expect(decoded[2]).toBe(68); // 'D'
    expect(decoded[3]).toBe(70); // 'F'
  });
});
