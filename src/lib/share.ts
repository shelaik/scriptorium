// Sharing helpers. Web WhatsApp/Teams/Gmail can't receive a local file via a
// link, so we share the real PDF by copying it to the clipboard (the user pastes
// it with Ctrl+V) and opening the target app with a pre-filled message. Outlook
// desktop is special-cased to attach the file directly.
import { invoke } from "@tauri-apps/api/core";

export type ShareTarget = "whatsapp" | "teams" | "gmail" | "outlook";

export interface ShareOutcome {
  ok: boolean;
  note: string;
}

function openExternal(url: string): Promise<void> {
  return invoke<void>("open_external", { url });
}

/** Reveal a document's PDF in the file explorer. */
export function revealDocument(id: number): Promise<void> {
  return invoke<void>("reveal_pdf", { id });
}

/** Open an http(s) URL in the system browser (e.g. a paper's landing page). */
export function openInBrowser(url: string): Promise<void> {
  return openExternal(url);
}

async function copyPdfs(ids: number[]): Promise<number> {
  try {
    return await invoke<number>("copy_pdfs_to_clipboard", { ids });
  } catch {
    return 0;
  }
}

function composeUrl(target: ShareTarget, subject: string, body: string): string {
  const s = encodeURIComponent(subject);
  const b = encodeURIComponent(body);
  switch (target) {
    case "whatsapp":
      return `https://wa.me/?text=${b}`;
    case "teams":
      return `https://teams.microsoft.com/l/chat/0/0?message=${b}`;
    case "gmail":
      return `https://mail.google.com/mail/?view=cm&fs=1&su=${s}&body=${b}`;
    case "outlook":
      return `https://outlook.office.com/mail/deeplink/compose?subject=${s}&body=${b}`;
  }
}

/**
 * Share the given documents to a target app. The real PDF(s) are copied to the
 * clipboard so they can be pasted into the conversation; for a single document
 * Outlook desktop attaches the file automatically when available.
 */
export async function shareTo(
  target: ShareTarget,
  ids: number[],
  label: string,
  link?: string | null,
): Promise<ShareOutcome> {
  if (!ids.length) return { ok: false, note: "Niente da condividere" };
  const subject = label || "Documento PDF";
  // Include the paper's original link (DOI) so the recipient can open it directly.
  const body = `Ti condivido: ${subject}` + (link && link.trim() ? `\n${link.trim()}` : "");

  // Outlook desktop, single document: attach the file directly.
  if (target === "outlook" && ids.length === 1) {
    try {
      await invoke<void>("share_via_outlook", { id: ids[0] });
      return { ok: true, note: "Outlook: PDF allegato a una nuova email" };
    } catch {
      // Outlook desktop not installed — fall back to webmail + clipboard below.
    }
  }

  const copied = await copyPdfs(ids);
  await openExternal(composeUrl(target, subject, body));

  if (copied === 0) {
    return { ok: true, note: "App aperta. Nessun PDF allegabile (riferimenti senza file)." };
  }
  const missing = ids.length - copied;
  const tail = missing > 0 ? ` (${missing} senza file saltati)` : "";
  const verb = copied > 1 ? `${copied} PDF copiati` : "PDF copiato";
  return { ok: true, note: `${verb}: incolla nella conversazione con Ctrl+V${tail}` };
}
