// Le sette schede della guida in-app.
//
// Gli id sono VALORI, non testo: alimentano `helpTab` nella pagina, gli id dei
// comandi della palette (`help-inizia`, …) e i rami `{#if tab === "…"}` dei due
// componenti di prosa. Non si traducono. Le etichette visibili restano dove si
// vedono — le linguette in +page.svelte e la palette — cosi' `t("…")` resta
// letterale e lo script di controllo (scripts/i18n-check.mjs) le vede.
/* i18n-exempt: valori di HelpTab, non testo — sono gli id dei comandi della
   palette (`help-inizia`…) e i rami {#if tab === "…"} di HelpIt/HelpEn */
export type HelpTab = "inizia" | "libreria" | "lettura" | "scrittura" | "scoperta" | "ai" | "faq";
