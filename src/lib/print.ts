// Printing helpers: print a single PDF or a set of selected PDFs as one job.
//
// Strategy: load the PDF bytes into a hidden same-origin <iframe> (blob URL) and
// drive the browser/WebView print dialog from there. This prints the real PDF —
// not the app UI — and keeps the user inside the app. For multiple documents we
// merge them into a single PDF (pdf-lib) so it's one print dialog, one job.
import { invoke } from "@tauri-apps/api/core";
import { PDFDocument } from "pdf-lib";

/** Fetch a document's raw PDF bytes from the backend. Throws for ref-only items (no file). */
async function fetchPdf(id: number): Promise<Uint8Array> {
  const buf = (await invoke("read_pdf", { id })) as ArrayBuffer;
  return new Uint8Array(buf);
}

/** Print already-in-memory PDF bytes via a hidden iframe. Resolves once the dialog has been triggered. */
export function printPdfData(data: Uint8Array | ArrayBuffer): Promise<void> {
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
  // Copy into a fresh ArrayBuffer so a detached/transferred buffer can't surprise us.
  const blob = new Blob([bytes.slice()], { type: "application/pdf" });
  const url = URL.createObjectURL(blob);

  return new Promise<void>((resolve, reject) => {
    const iframe = document.createElement("iframe");
    iframe.setAttribute("aria-hidden", "true");
    iframe.style.cssText =
      "position:fixed;right:0;bottom:0;width:0;height:0;border:0;visibility:hidden;";

    let settled = false;
    const finish = (err?: unknown) => {
      if (settled) return;
      settled = true;
      // Keep the iframe alive briefly so the print job can spool, then clean up.
      setTimeout(() => {
        URL.revokeObjectURL(url);
        iframe.remove();
      }, 60_000);
      if (err) reject(err);
      else resolve();
    };

    iframe.onload = () => {
      try {
        const win = iframe.contentWindow;
        if (!win) throw new Error("print frame unavailable");
        win.focus();
        win.addEventListener?.("afterprint", () => finish(), { once: true });
        win.print();
        // afterprint isn't guaranteed in every engine — resolve optimistically too.
        setTimeout(() => finish(), 1500);
      } catch (e) {
        finish(e);
      }
    };
    iframe.onerror = () => finish(new Error("impossibile caricare il PDF per la stampa"));

    iframe.src = url;
    document.body.appendChild(iframe);
  });
}

/** Print one document by id. */
export async function printDocument(id: number): Promise<void> {
  await printPdfData(await fetchPdf(id));
}

/** Print a standalone HTML document (e.g. a rendered note) via a hidden iframe, so
 *  only the content prints — not the app UI. The user picks "Save as PDF" to export. */
export function printHtml(html: string): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    const iframe = document.createElement("iframe");
    iframe.setAttribute("aria-hidden", "true");
    iframe.style.cssText =
      "position:fixed;right:0;bottom:0;width:0;height:0;border:0;visibility:hidden;";

    let settled = false;
    const finish = (err?: unknown) => {
      if (settled) return;
      settled = true;
      setTimeout(() => iframe.remove(), 60_000);
      if (err) reject(err);
      else resolve();
    };

    iframe.onload = () => {
      try {
        const win = iframe.contentWindow;
        if (!win) throw new Error("print frame unavailable");
        win.focus();
        win.addEventListener?.("afterprint", () => finish(), { once: true });
        win.print();
        setTimeout(() => finish(), 1500);
      } catch (e) {
        finish(e);
      }
    };
    iframe.onerror = () => finish(new Error("impossibile preparare la stampa"));
    // srcdoc keeps the frame same-origin (inherits the app CSP: inline styles,
    // data: images and MathML all render; there is no script in the export).
    iframe.srcdoc = html;
    document.body.appendChild(iframe);
  });
}

export interface PrintResult {
  /** How many documents made it into the print job. */
  printed: number;
  /** How many were skipped (reference-only / unreadable / un-mergeable). */
  skipped: number;
}

/**
 * Print several documents as a single merged PDF (one print dialog).
 * Reference-only entries (no PDF on disk) and unreadable files are skipped, not fatal.
 */
export async function printDocuments(ids: number[]): Promise<PrintResult> {
  if (ids.length === 0) return { printed: 0, skipped: 0 };
  if (ids.length === 1) {
    try {
      await printDocument(ids[0]);
      return { printed: 1, skipped: 0 };
    } catch {
      return { printed: 0, skipped: 1 };
    }
  }

  // Bound the in-memory merge so a huge selection can't OOM the WebView.
  const MAX_MERGE_DOCS = 50;
  const MAX_MERGE_BYTES = 250 * 1024 * 1024; // 250 MB total
  const merged = await PDFDocument.create();
  let printed = 0;
  let skipped = 0;
  let totalBytes = 0;
  for (const id of ids) {
    if (printed >= MAX_MERGE_DOCS || totalBytes >= MAX_MERGE_BYTES) {
      skipped++;
      continue;
    }
    try {
      const bytes = await fetchPdf(id);
      totalBytes += bytes.byteLength;
      if (totalBytes > MAX_MERGE_BYTES) {
        skipped++;
        continue;
      }
      const src = await PDFDocument.load(bytes, { ignoreEncryption: true });
      const pages = await merged.copyPages(src, src.getPageIndices());
      for (const p of pages) merged.addPage(p);
      printed++;
    } catch {
      skipped++;
    }
  }

  if (printed === 0) return { printed: 0, skipped };
  const out = await merged.save();
  await printPdfData(out);
  return { printed, skipped };
}
