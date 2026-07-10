// KaTeX rendering for the in-app math previews.
//
// The Rust backend emits math as `<span class="tex">RAW LaTeX</span>` placeholders
// (see `wiki::render_html_live`); here we fill them in — in the webview — with KaTeX.
// KaTeX covers far more LaTeX than the MathML path, renders `\mathrm{…}` fully upright
// (fractions/scripts included) and handles gathered/aligned natively. The file exports
// (`wiki::render_html` → MathML) stay self-contained and never reach this code.
//
// KaTeX is pure JS (no eval), bundled with the app, so it satisfies the strict CSP; its
// CSS + web fonts are emitted by Vite as same-origin assets.
import katex from "katex";
import "katex/dist/katex.min.css";

// `trust:false` (no \url/\href/\includegraphics side effects), non-strict so minor
// LaTeX quirks render instead of erroring. KaTeX's default `htmlAndMathml` output keeps
// the visually-hidden MathML layer that assistive tech and text selection rely on.
const OPTS = { strict: false as const, trust: false };

/** Render one LaTeX string to KaTeX HTML. On a parse error returns the graceful (red)
 *  error markup PLUS the message, so the formula editor can flag an invalid formula. */
export function renderMathString(tex: string, display: boolean): { html: string; error: string } {
  const t = tex.trim();
  if (!t) return { html: "", error: "" };
  try {
    const html = katex.renderToString(t, { ...OPTS, displayMode: display, throwOnError: true });
    return { html, error: "" };
  } catch (e) {
    const error = e instanceof Error ? e.message : String(e);
    let html = "";
    try {
      html = katex.renderToString(t, { ...OPTS, displayMode: display, throwOnError: false });
    } catch {
      html = "";
    }
    return { html, error };
  }
}

// KaTeX output cached by (display flag + LaTeX): an `{@html}` update swaps the whole
// subtree, so every placeholder is a fresh node and the same formulas would otherwise be
// re-parsed on each keystroke batch. Reusing the cached HTML keeps math-heavy notes snappy.
const cache = new Map<string, string>();

/** Whether KaTeX can parse+render `tex` without error — used to rerank local OCR
 *  hypotheses (keep the best candidate that actually compiles). */
export function isMathValid(tex: string): boolean {
  const t = tex.trim();
  if (!t) return false;
  try {
    katex.renderToString(t, { ...OPTS, displayMode: true, throwOnError: true });
    return true;
  } catch {
    return false;
  }
}

/** Render every not-yet-rendered `<span class="tex">` placeholder inside `root` with
 *  KaTeX (reusing cached output). Idempotent; a parse error leaves the raw LaTeX visible. */
export function renderMathInto(root: HTMLElement): void {
  const spans = root.querySelectorAll<HTMLElement>("span.tex:not([data-rendered])");
  spans.forEach((el) => {
    el.setAttribute("data-rendered", "1");
    const tex = el.textContent ?? "";
    const display = el.classList.contains("block");
    const key = (display ? "B" : "I") + tex;
    let html = cache.get(key);
    if (html === undefined) {
      try {
        html = katex.renderToString(tex, { ...OPTS, displayMode: display, throwOnError: false });
      } catch {
        html = "";
      }
      if (cache.size > 2000) cache.clear(); // simple memory bound
      cache.set(key, html);
    }
    if (html) el.innerHTML = html;
  });
}

/** Svelte action: (re)render math placeholders after an `{@html}` injection. Pass the
 *  HTML string as the argument so Svelte fires `update` whenever the content changes
 *  (Svelte applies the markup update before calling the action's `update`). */
export function mathRender(node: HTMLElement, _html?: unknown) {
  renderMathInto(node);
  return {
    update() {
      renderMathInto(node);
    },
  };
}
