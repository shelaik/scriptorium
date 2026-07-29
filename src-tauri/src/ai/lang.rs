//! La lingua in cui l'AI locale deve RISPONDERE.
//!
//! E' una manopola separata da quella dell'interfaccia (scelta esplicita
//! dell'utente): chi legge paper in inglese puo' volere l'interfaccia in
//! italiano e i riassunti in inglese, o viceversa. Il valore predefinito
//! (`auto`) segue l'interfaccia, cosi' chi non ci pensa non deve pensarci.
//!
//! Prima la lingua era saldata dentro undici frasi diverse — «Rispondi … in
//! italiano», «Riassumi in italiano», «Spiega in italiano», «Scrivi in
//! italiano» — cioe' incastonata nella sintassi italiana del prompt. Tradurre
//! quelle frasi avrebbe prodotto ibridi fragili tipo «Riassumi in English».
//!
//! Qui c'e' UNA sola formulazione, autonoma e scritta NELLA lingua di
//! destinazione (che per i modelli piccoli e' il segnale piu' forte), e un solo
//! punto — [`with_lang`] — in cui la lingua entra in un prompt. Il corpo del
//! prompt resta in italiano: e' codice sorgente, non uscita.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    It,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::It => "it",
            Lang::En => "en",
        }
    }

    /// `"it"`/`"en"` -> la lingua; qualunque altra cosa -> italiano.
    pub fn from_code(s: &str) -> Lang {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" => Lang::En,
            _ => Lang::It,
        }
    }

    /// La direttiva, autonoma e nella lingua richiesta. Unica formulazione
    /// dell'app: se un prompt di prosa non passa da qui, non ha lingua.
    pub fn directive(self) -> &'static str {
        match self {
            Lang::It => "Scrivi la risposta in italiano.",
            Lang::En => "Write your answer in English.",
        }
    }

    /// Solo per il compito «traduci», dove la lingua NON e' una direttiva ma
    /// l'oggetto stesso del lavoro.
    pub fn in_lang(self) -> &'static str {
        match self {
            Lang::It => "in italiano",
            Lang::En => "in inglese",
        }
    }

    /// Etichetta del prefisso di completamento («RISPOSTA (con citazioni [n]):»):
    /// e' un trucco che orienta il modello, non un'istruzione.
    pub fn answer_label(self) -> &'static str {
        match self {
            Lang::It => "RISPOSTA",
            Lang::En => "ANSWER",
        }
    }
}

/// Antepone la direttiva di lingua al corpo del prompt.
pub fn with_lang(lang: Lang, body: &str) -> String {
    format!("{}\n\n{body}", lang.directive())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directive_is_written_in_its_own_language() {
        // Il senso della direttiva: e' scritta NELLA lingua che chiede, non
        // tradotta a parole in italiano. Un modello piccolo obbedisce di piu'.
        assert!(Lang::En.directive().contains("English"));
        assert!(Lang::It.directive().contains("italiano"));
        assert!(!Lang::En.directive().contains("italiano"));
    }

    #[test]
    fn unknown_codes_fall_back_to_italian() {
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("EN "), Lang::En);
        assert_eq!(Lang::from_code("it"), Lang::It);
        assert_eq!(Lang::from_code("auto"), Lang::It);
        assert_eq!(Lang::from_code(""), Lang::It);
        assert_eq!(Lang::from_code("de"), Lang::It);
    }

    #[test]
    fn with_lang_puts_the_directive_first() {
        let p = with_lang(Lang::En, "Riassumi in 4-6 frasi.");
        assert!(p.starts_with("Write your answer in English."));
        // Il corpo resta in italiano: e' sorgente, non uscita.
        assert!(p.contains("Riassumi in 4-6 frasi."));
    }
}
