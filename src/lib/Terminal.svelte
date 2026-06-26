<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import "@xterm/xterm/css/xterm.css";

  // The parent handles confirmation + teardown (unmounting kills the PTY).
  let { onClose }: { onClose?: () => void } = $props();

  let container: HTMLDivElement;
  let term: Terminal | undefined;
  let fit: FitAddon | undefined;
  let unlistenOut: UnlistenFn | undefined;
  let unlistenExit: UnlistenFn | undefined;
  let ro: ResizeObserver | undefined;
  let resizeTimer: ReturnType<typeof setTimeout> | undefined;
  let cwd = $state("");
  // Current PTY session epoch; ignore events from a replaced session.
  const sess = { epoch: 0 };

  // Refit ONLY when the proposed grid (cols×rows) actually differs from the
  // current one. This guard is essential: it breaks the ResizeObserver→fit→resize
  // feedback loop that would otherwise resize the PTY every frame (ConPTY repaints
  // its buffer on each resize, which looked like the "Windows PowerShell" banner
  // reprinting endlessly).
  function doFit() {
    if (!fit || !term || !container) return;
    if (container.clientWidth < 5 || container.clientHeight < 5) return; // hidden / not laid out
    let dims;
    try {
      dims = fit.proposeDimensions();
    } catch {
      return;
    }
    if (!dims || !Number.isFinite(dims.cols) || !Number.isFinite(dims.rows)) return;
    if (dims.cols === term.cols && dims.rows === term.rows) return; // already correct → no resize, no loop
    try {
      fit.fit();
    } catch {
      /* ignore transient layout */
    }
  }

  /** Resolve once the container size has been stable for 2 frames (or maxMs). */
  function waitStableSize(maxMs = 700): Promise<void> {
    return new Promise((resolve) => {
      let prevW = -1;
      let prevH = -1;
      let stable = 0;
      let elapsed = 0;
      const tick = () => {
        const w = container?.clientWidth ?? 0;
        const h = container?.clientHeight ?? 0;
        if (w >= 5 && h >= 5 && w === prevW && h === prevH) {
          if (++stable >= 2) return resolve();
        } else {
          stable = 0;
        }
        prevW = w;
        prevH = h;
        elapsed += 16;
        if (elapsed >= maxMs) return resolve();
        requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    });
  }

  /** (Re)start the PTY, optionally in a chosen folder. */
  async function reopen(dir?: string) {
    if (!term) return;
    term.reset();
    try {
      const r = await invoke<{ epoch: number; cwd: string }>("term_open", {
        cols: term.cols,
        rows: term.rows,
        cwd: dir ?? null,
      });
      sess.epoch = Math.max(sess.epoch, r.epoch);
      cwd = r.cwd;
      term.focus();
    } catch (e) {
      term.writeln("Errore avvio terminale: " + e);
    }
  }

  async function changeFolder() {
    const dir = await open({ directory: true, multiple: false, title: "Apri il terminale in questa cartella" });
    if (typeof dir === "string") await reopen(dir);
  }

  onMount(async () => {
    term = new Terminal({
      fontFamily: "ui-monospace, 'Cascadia Code', 'Cascadia Mono', Consolas, monospace",
      fontSize: 13,
      cursorBlink: true,
      scrollback: 5000,
      theme: {
        background: "#0c0e12",
        foreground: "#e6e8ec",
        cursor: "#6f9bf0",
        selectionBackground: "#2b4a78",
      },
    });
    fit = new FitAddon();
    term.loadAddon(fit);
    term.open(container);

    unlistenOut = await listen<{ epoch: number; data: number[] }>("term-output", (e) => {
      if (e.payload.epoch < sess.epoch) return; // straggler from a replaced session
      if (e.payload.epoch > sess.epoch) sess.epoch = e.payload.epoch;
      term?.write(new Uint8Array(e.payload.data));
    });
    unlistenExit = await listen<{ epoch: number }>("term-exit", (e) => {
      if (e.payload.epoch < sess.epoch) return;
      term?.writeln("\r\n\x1b[2m[sessione terminata — premi Riavvia per ripartire]\x1b[0m");
    });
    term.onData((d) => {
      invoke("term_write", { data: d }).catch(() => {});
    });
    // Resize the PTY only AFTER the size settles. ConPTY repaints its whole
    // viewport on every resize (which reprinted the "Windows PowerShell" banner
    // line); debouncing means the visible terminal reflows fluidly on every frame
    // but the PTY is told the new size once, so ConPTY repaints at most once.
    term.onResize(({ cols, rows }) => {
      clearTimeout(resizeTimer);
      resizeTimer = setTimeout(() => {
        invoke("term_resize", { cols, rows }).catch(() => {});
      }, 140);
    });

    // Wait for the area to STOP changing size, then fit + start the PTY — so the
    // shell opens already at its final size and no resize happens while it prints
    // its banner (a resize would make ConPTY repaint, reprinting the banner line).
    await waitStableSize();
    doFit();
    await reopen();

    // Refit on any size change. IMPORTANT: call doFit() *synchronously* inside the
    // ResizeObserver callback — that keeps the browser's native loop protection,
    // so a fit that nudges the layout can't re-trigger the observer every frame
    // (which had ConPTY repainting the "Windows PowerShell" banner repeatedly).
    ro = new ResizeObserver(() => doFit());
    ro.observe(container);
    window.addEventListener("resize", doFit);
    term.focus();
  });

  onDestroy(() => {
    ro?.disconnect();
    clearTimeout(resizeTimer);
    window.removeEventListener("resize", doFit);
    unlistenOut?.();
    unlistenExit?.();
    invoke("term_close").catch(() => {});
    term?.dispose();
  });
</script>

<div class="termwrap">
  <div class="termbar">
    <span class="folder" title={cwd}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>
      <span class="cwd">{cwd || "…"}</span>
    </span>
    <span class="spacer"></span>
    <button class="tbtn" onclick={changeFolder} title="Apri il terminale in un'altra cartella">Cambia cartella…</button>
    <button class="tbtn" onclick={() => reopen()} title="Termina e riavvia la sessione nella stessa cartella">Riavvia</button>
    <button class="tbtn close" onclick={() => onClose?.()} title="Chiudi il terminale">✕ Chiudi</button>
  </div>
  <div class="termhost" bind:this={container}></div>
</div>

<style>
  .termwrap { display: flex; flex-direction: column; height: 100%; background: #0c0e12; }
  .termbar {
    display: flex; align-items: center; gap: 8px;
    padding: 6px 10px; background: #14171d; border-bottom: 1px solid #2a2f3a;
    color: #9aa1ad; font-size: 12px; flex: 0 0 auto;
  }
  .folder { display: inline-flex; align-items: center; gap: 6px; min-width: 0; color: #b4bcca; }
  .folder svg { flex: 0 0 auto; color: #6f9bf0; }
  .cwd { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: ui-monospace, monospace; }
  .spacer { flex: 1; }
  .tbtn {
    background: transparent; border: 1px solid #2a2f3a; color: #b4bcca;
    border-radius: 7px; padding: 4px 10px; font-size: 12px; cursor: pointer; white-space: nowrap;
    transition: border-color 0.15s, color 0.15s, background 0.15s;
  }
  .tbtn:hover { border-color: #6f9bf0; color: #e6e8ec; }
  .tbtn.close:hover { border-color: #ff8585; color: #ff8585; background: rgba(255, 133, 133, 0.08); }
  .termhost { flex: 1; min-height: 0; background: #0c0e12; padding: 8px 10px; box-sizing: border-box; }
  .termhost :global(.xterm) { height: 100%; }
  .termhost :global(.xterm-viewport) { background: transparent !important; }
</style>
