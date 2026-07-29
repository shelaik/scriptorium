# Come si rilascia una versione

Scriptorium si aggiorna da dentro l'app. Perché funzioni, **ogni release deve
portare tre file oltre all'installer**, e la build deve essere firmata: se
manca la firma o il manifesto, le installazioni esistenti semplicemente non
vedono l'aggiornamento (nessun danno, ma nessun aggiornamento).

## La chiave di firma

- Privata: `%USERPROFILE%\.tauri\scriptorium.key` — **è il segreto**. Non ha
  password, quindi vale il file in sé: copia in un gestore di password e in un
  backup offline. Chi ce l'ha può far installare codice sulle macchine di chi
  usa Scriptorium.
- Pubblica: `%USERPROFILE%\.tauri\scriptorium.key.pub`, già incorporata in
  `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`). Non è un segreto.
- Se la chiave privata va persa: le installazioni esistenti non accetteranno
  più aggiornamenti. Si genera una chiave nuova, si aggiorna il `pubkey` e si
  distribuisce un installer a mano (una volta sola).

## Passi

1. **Versione in quattro punti**: `package.json`, `src-tauri/tauri.conf.json`,
   `src-tauri/Cargo.toml`, `const APP_VERSION` in `src/routes/+page.svelte`.
2. **Voce nel CHANGELOG**: `## X.Y.Z — Titolo`. È il testo che l'utente vedrà
   nel pannello di aggiornamento e, dopo il riavvio, nel riquadro «Novità».
3. **Controlli**: `npm run check` (0 errori), `cargo test --lib`, e i due della
   traduzione:

   ```powershell
   node scripts/i18n-check.mjs    # deve uscire 0
   node scripts/i18n-drift.mjs <ultimo-tag>
   ```

   - `i18n-check` fallisce se ci sono chiavi **mancanti** (resterebbero in
     italiano con l'interfaccia in inglese), **segnaposto disallineati** fra le
     due lingue (un messaggio che perde il numero), o **attributi non avvolti** —
     i `title=` e gli `aria-label`, che sono invisibili a un collaudo per
     schermate e sono la parte più voluminosa del testo. Le voci **orfane** sono
     solo un avviso: vuol dire che una stringa italiana è stata riscritta e la
     sua traduzione è rimasta indietro.
   - `i18n-drift` elenca le chiavi il cui testo italiano non compare più alla
     lettera nel tag indicato. Serve alla rete di sicurezza del meccanismo:
     **la chiave È la stringa italiana**, quindi in italiano `t(x)` restituisce
     `x` e l'app deve restare identica. Ogni riga va giustificata (frammenti di
     markup ricuciti, forma singolare nuova, riscrittura voluta) o è un difetto.

   Regola che ne discende: **quando ritocchi un testo italiano, aggiorna la
   chiave in `src/lib/i18n/en.ts`**, altrimenti quella frase smette
   silenziosamente di essere tradotta.
4. **Build firmata** — la variabile è indispensabile:

   ```powershell
   $env:TAURI_SIGNING_PRIVATE_KEY = "$env:USERPROFILE\.tauri\scriptorium.key"
   $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
   npm run tauri build
   ```

   Entrambe le variabili servono: **`…_PATH` non viene letta** (il nome giusto è
   `TAURI_SIGNING_PRIVATE_KEY`, che accetta anche un percorso), e senza
   `…_PASSWORD` il CLI resta ad aspettarla da tastiera e **la build si pianta
   in silenzio**. Peggio: quando la firma non riesce, `npm run tauri build`
   stampa l'errore ma **esce comunque con codice 0** — quindi non fidarti
   dell'esito, controlla che il `.sig` esista.

   Produce in `src-tauri/target/release/bundle/nsis/`:
   `Scriptorium_X.Y.Z_x64-setup.exe` e `Scriptorium_X.Y.Z_x64-setup.exe.sig`
   (Tauri 2 firma l'installer direttamente: niente `.nsis.zip`, quello era il
   formato della v1).
5. **Manifesto**: `node scripts/make-latest-json.mjs [note.md]`
6. **Release** con **tutti** gli allegati:

   ```powershell
   gh release create vX.Y.Z `
     src-tauri/target/release/bundle/nsis/Scriptorium_X.Y.Z_x64-setup.exe `
     src-tauri/target/release/bundle/nsis/Scriptorium_X.Y.Z_x64-setup.exe.sig `
     src-tauri/target/release/bundle/nsis/latest.json `
     src-tauri/target/release/scriptorium-cli.exe `
     src-tauri/target/release/scriptorium-mcp.exe `
     --repo shelaik/scriptorium --title "Scriptorium X.Y.Z" --notes-file note.md
   ```

   L'endpoint interrogato dall'app è
   `https://github.com/shelaik/scriptorium/releases/latest/download/latest.json`:
   punta sempre alla release **più recente non-draft**, quindi il manifesto va
   allegato a ogni release, altrimenti l'aggiornamento resta fermo a quella
   prima.

I due binari compagni (`scriptorium-cli`, `scriptorium-mcp`) si compilano **da
`src-tauri`** con `--features cli` / `--features mcp`, **senza pipe** (un
`| tail` maschera l'exit code di cargo ed è già costato una release con un
binario vecchio allegato).
