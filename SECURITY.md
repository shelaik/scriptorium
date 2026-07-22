# Sicurezza

Scriptorium è un'applicazione **locale e single-user** per Windows. I dati (libreria,
PDF, note, progetti) restano sul computer dell'utente in `%APPDATA%\com.pdfmanage.app`;
le funzioni di **rete** (recupero metadati, ricerca online) e di **AI** (Ollama /
LM Studio locali) sono **disattivate per impostazione predefinita** e vanno abilitate
esplicitamente.

## Versioni supportate

Riceve correzioni di sicurezza solo l'**ultima versione** pubblicata nelle
[Release](https://github.com/shelaik/scriptorium/releases).

## Come segnalare una vulnerabilità

Per favore **non aprire una issue pubblica** per problemi di sicurezza.

Usa la segnalazione privata di GitHub: scheda **Security → “Report a vulnerability”**
del repository (Private Vulnerability Reporting). Descrivi il problema, i passi per
riprodurlo e l'impatto atteso. Le segnalazioni vengono valutate e, se confermate,
corrette in una release successiva; l'attribuzione è volentieri riconosciuta a chi
la desidera.

## Note

Prima di ogni rilascio significativo il codice viene sottoposto a revisioni
adversariali interne (analisi statica multi-agente più `cargo audit` / `npm audit`).
Trattandosi di un progetto personale gestito da una sola persona, i tempi di risposta
possono variare.
