import { describe, it, expect } from "vitest";
import {
  isPdfBytes,
  isPdfFilename,
  formatFileSize,
  titleFromFilename,
  createPdfMetadata,
  estimatePageCount,
} from "../adapters/pdf/PdfAdapter";

describe("PdfAdapter", () => {
  describe("isPdfBytes", () => {
    it("detects PDF magic bytes", () => {
      const pdf = new Uint8Array([0x25, 0x50, 0x44, 0x46, 0x2d, 0x31, 0x2e]);
      expect(isPdfBytes(pdf)).toBe(true);
    });

    it("rejects non-PDF bytes", () => {
      const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);
      expect(isPdfBytes(png)).toBe(false);
    });

    it("rejects too-short arrays", () => {
      expect(isPdfBytes(new Uint8Array([0x25, 0x50]))).toBe(false);
      expect(isPdfBytes(new Uint8Array([]))).toBe(false);
    });
  });

  describe("isPdfFilename", () => {
    it("detects .pdf extension", () => {
      expect(isPdfFilename("document.pdf")).toBe(true);
      expect(isPdfFilename("REPORT.PDF")).toBe(true);
      expect(isPdfFilename("my.file.pdf")).toBe(true);
    });

    it("rejects non-PDF filenames", () => {
      expect(isPdfFilename("document.txt")).toBe(false);
      expect(isPdfFilename("image.png")).toBe(false);
      expect(isPdfFilename("pdf")).toBe(false);
    });
  });

  describe("formatFileSize", () => {
    it("formats bytes", () => {
      expect(formatFileSize(500)).toBe("500 B");
    });

    it("formats kilobytes", () => {
      expect(formatFileSize(2048)).toBe("2.0 KB");
      expect(formatFileSize(1536)).toBe("1.5 KB");
    });

    it("formats megabytes", () => {
      expect(formatFileSize(5 * 1024 * 1024)).toBe("5.0 MB");
    });

    it("formats gigabytes", () => {
      expect(formatFileSize(2.5 * 1024 * 1024 * 1024)).toBe("2.50 GB");
    });
  });

  describe("titleFromFilename", () => {
    it("removes extension and cleans separators", () => {
      expect(titleFromFilename("my-report_2024.pdf")).toBe("my report 2024");
      expect(titleFromFilename("Annual_Report.pdf")).toBe("Annual Report");
      expect(titleFromFilename("simple.pdf")).toBe("simple");
    });

    it("handles multiple separators", () => {
      expect(titleFromFilename("a-b_c--d.pdf")).toBe("a b c d");
    });
  });

  describe("createPdfMetadata", () => {
    it("creates complete metadata", () => {
      const meta = createPdfMetadata("report.pdf", 1024 * 1024, 10);
      expect(meta.fileName).toBe("report.pdf");
      expect(meta.title).toBe("report");
      expect(meta.fileSize).toBe(1024 * 1024);
      expect(meta.fileSizeFormatted).toBe("1.0 MB");
      expect(meta.pageCount).toBe(10);
    });

    it("handles null page count", () => {
      const meta = createPdfMetadata("doc.pdf", 500);
      expect(meta.pageCount).toBeNull();
    });
  });

  describe("estimatePageCount", () => {
    it("estimates at least 1 page", () => {
      expect(estimatePageCount(100)).toBe(1);
    });

    it("estimates proportional to size", () => {
      const pages = estimatePageCount(750 * 1024); // 750KB
      expect(pages).toBe(10);
    });
  });
});
