// SPDX-License-Identifier: MPL-2.0
//! F11 S6a: the message catalog for the setup wizard, Arthur's intro card,
//! the clock and Home's verbs (`docs/THINK-2026-11-02-setup-menu.md` §3.1,
//! §5 S6; `Plans/DISPATCH-2026-11-09-seat-c-setup-s6a-i18n-catalog.md`).
//!
//! - [`Msg`]: one id per string those surfaces show today.
//! - Two tables: **English**, word for word what ships today (a KAT reads
//!   the source files and holds them equal), and **Spanish**, the second
//!   language.
//! - [`text`]: the lookup, with a fallback to English for an empty entry.
//!   [`lookup`] says whether the fallback was taken.
//! - [`fill`]: `{}` arguments in order, or `{0}`..`{9}` by index where a
//!   language orders them differently (the date). There are no plurals.
//!
//! **S6b** moved the literals to these ids: `setup-core`, `neo-atrium`'s
//! intro, power and clock cards, `libneo::civil` and Home's clock verbs draw
//! from here. The `i18n-catalog` kill-switch shows the Language page; off,
//! the language is English and every row is what shipped before S6b (the
//! pinned table, `fixtures/en.txt`, and the crates' row KATs).
//!
//! **Command words are not translated.** `help`, `list /`, `timezone
//! Area/City`, `rtc utc` and `rtc local` are what the parsers read, so they
//! appear unchanged inside the Spanish rows. The power verbs' trigger words
//! (`shut down`, `restart`, …) aren't in the catalog for the same reason.
//! `TRANSLATION-NOTES.md` lists every row and each choice a reviewer should
//! check.

#![no_std]

extern crate alloc;

use alloc::string::String;

#[cfg(test)]
mod stray;
#[cfg(test)]
mod tests;

/// A language the catalog has a table for. The default is English (the
/// store's `Language` 0).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Lang {
    #[default]
    En,
    Es,
}

/// How many languages there are. The Language page shows when this is more
/// than one (S6b).
pub const LANGUAGE_COUNT: usize = 2;

impl Lang {
    /// Every language, in the order the Language page lists them.
    pub const ALL: [Lang; LANGUAGE_COUNT] = [Lang::En, Lang::Es];

    /// The language's own name, as its picker row shows it.
    pub const fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Es => "Español",
        }
    }

    /// The BCP 47 tag.
    pub const fn tag(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Es => "es-419",
        }
    }

    /// The language's index (the setting's stored value).
    pub const fn index(self) -> usize {
        match self {
            Lang::En => 0,
            Lang::Es => 1,
        }
    }

    /// The language with this index (the setting's stored value), if any.
    pub const fn from_index(i: usize) -> Option<Lang> {
        if i < LANGUAGE_COUNT {
            Some(Lang::ALL[i])
        } else {
            None
        }
    }
}

macro_rules! catalog {
    ($($(#[$doc:meta])* $id:ident = $en:literal, $es:literal;)*) => {
        /// One string a user sees. The variant's position is its index into
        /// each table.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum Msg {
            $($(#[$doc])* $id,)*
        }

        impl Msg {
            /// Every id, in table order.
            pub const ALL: &'static [Msg] = &[$(Msg::$id,)*];
        }

        /// How many ids there are.
        pub const MSG_COUNT: usize = Msg::ALL.len();

        /// The English table: today's literals, byte for byte.
        const EN: [&str; MSG_COUNT] = [$($en,)*];

        /// The Spanish table (neutral Latin American Spanish).
        const ES: [&str; MSG_COUNT] = [$($es,)*];
    };
}

catalog! {
    // ---- The setup wizard (`setup-core`, and the view's notices) ----
    /// The last row of every wizard page.
    SetupHint = "Up/Down to move, Enter to choose, Esc to go back.",
        "Arriba/Abajo para moverte, Enter para elegir, Esc para volver.";
    /// The last row of the searchable Time zone page.
    SetupSearchHint = "Type to search. Up/Down, Enter to choose.",
        "Escribe para buscar. Arriba/Abajo, Enter para elegir.";
    SetupBegin = "Begin", "Comenzar";
    SetupSkip = "Skip setup", "Omitir la configuración";
    SetupSkipDefaults = "Skip setup and use the defaults",
        "Omitir y usar los valores predeterminados";
    SetupBack = "Back", "Volver";
    SetupContinue = "Continue", "Continuar";
    /// The Try page's option.
    SetupTryOption = "Try LogansOS", "Probar LogansOS";
    /// A check-box option: the argument is the mark, `[x]` or `[ ]`.
    SetupReduceMotionOption = "{} Reduce motion", "{} Reducir movimiento";
    SetupReduceTransparencyOption = "{} Reduce transparency", "{} Reducir transparencia";
    SetupTitleWelcome = "Set up LogansOS", "Configurar LogansOS";
    SetupTitleSkip = "Skip setup?", "¿Omitir la configuración?";
    SetupTitleLanguage = "Language", "Idioma";
    SetupTitleKeyboard = "Keyboard", "Teclado";
    SetupTitleTimeZone = "Time zone", "Zona horaria";
    SetupTitleComfort = "Comfort", "Comodidad";
    SetupTitleTry = "Try LogansOS", "Probar LogansOS";
    /// A page's title: its name, then `N of M`.
    SetupTitlePage = "{} ({} of {})", "{} ({} de {})";
    SetupBodyWelcome = "A few short screens. You can change any of these later.",
        "Unas pocas pantallas breves. Puedes cambiar todo después.";
    SetupBodySkip = "The defaults are UTC, with full motion and transparency.",
        "Lo predeterminado es UTC, con movimiento y transparencia completos.";
    SetupBodyLanguage = "English is the only language for now.",
        "Por ahora, el inglés es el único idioma.";
    SetupBodyKeyboard = "US is the only keyboard layout for now.",
        "Por ahora, la única distribución de teclado es la de EE. UU.";
    /// The Keyboard page with layouts (S5, `keyboard-layouts`).
    SetupKeyboardBodyChoose = "Choose the layout printed on your keys.",
        "Elige la distribución impresa en tus teclas.";
    /// The test field; the argument is what was typed, then a cursor `_`.
    SetupKeyboardTryIt = "Try it: {}_", "Pruébalo: {}_";
    /// The Try summary's keyboard row; the argument is a layout's name.
    SetupSummaryKeyboard = "Keyboard: {}", "Teclado: {}";
    /// The layouts' names (`kernel/src/drivers/keymap_layout_logic.rs`
    /// `LAYOUT_NAMES`, in that order).
    LayoutEnglishUs = "English (US)", "Inglés (EE. UU.)";
    LayoutSpanishSpain = "Spanish (Spain)", "Español (España)";
    SetupBodyTimeZoneNoSearch = "Where are you? At Home: timezone Area/City.",
        "¿Dónde estás? En Inicio: timezone Area/City.";
    /// The confirmed zone; the argument is its name.
    SetupBodyZoneSet = "Time zone: {}. The clock is set.",
        "Zona horaria: {}. El reloj está en hora.";
    SetupBodyZoneLater = "Change it later at Home: timezone Area/City.",
        "Cámbiala después en Inicio: timezone Area/City.";
    SetupBodySearchEmpty = "Type a city to search, or pick one.",
        "Escribe una ciudad para buscar, o elige una.";
    /// The argument is the typed search (two spaces before the bracket).
    SetupBodySearchNone = "Search: {}  (no zone matches)",
        "Buscar: {}  (ninguna zona coincide)";
    SetupBodySearch = "Search: {}", "Buscar: {}";
    SetupBodyComfort = "Make the screen calmer, if you like.",
        "Haz que la pantalla sea más tranquila, si quieres.";
    /// Stands in for a zone name when none is held.
    SetupNotSet = "not set", "sin definir";
    /// The Try summary's and Home's zone row; the argument is the name.
    ZoneRow = "Time zone: {}", "Zona horaria: {}";
    SetupSummaryZoneClockNotSet = "Time zone: {} (clock not set)",
        "Zona horaria: {} (reloj sin ajustar)";
    ZoneRowNotSet = "Time zone: not set", "Zona horaria: sin definir";
    /// The arguments are [`Msg::On`] or [`Msg::Off`].
    SetupSummaryComfort = "Reduce motion: {}. Reduce transparency: {}.",
        "Reducir movimiento: {}. Reducir transparencia: {}.";
    On = "on", "sí";
    Off = "off", "no";
    /// The view's notice when the zone couldn't be stored; the argument is
    /// the zone in use.
    SetupNoticeZoneInUse = "Couldn't save the time zone. In use: {}.",
        "No se pudo guardar la zona horaria. En uso: {}.";
    SetupNoticeZoneRestart = "Couldn't save the time zone. Restart to check it.",
        "No se pudo guardar la zona horaria. Reinicia para comprobarla.";

    // ---- S6b: the Language page (`setup-core`, `i18n-catalog`) ----
    SetupBodyLanguageChoose = "Choose the language for setup and Home.",
        "Elige el idioma de la configuración y de Inicio.";
    /// Arthur's replies stay in English (S6b's scope).
    SetupBodyLanguageArthur = "Arthur answers in English for now.",
        "Por ahora, Arthur responde en inglés.";

    // ---- S4: the Network page (`setup-core`) ----
    SetupTitleNetwork = "Network", "Red";
    SetupContinueNoNetwork = "Continue without a network", "Continuar sin red";
    NetUnavailable = "Network status is unavailable.",
        "El estado de la red no está disponible.";
    NetNoLink = "Wired: no link.", "Cable: sin enlace.";
    /// The argument is the device's driver.
    NetDevice = "Device: {}.", "Dispositivo: {}.";
    /// The argument is the IPv4 address, dotted.
    NetConnected = "Wired: connected, {}", "Cable: conectado, {}";
    NetNoAddress = "Wired: connected, no address yet.",
        "Cable: conectado, aún sin dirección.";
    /// The arguments are the driver and the lease's time left
    /// ([`Msg::LeaseMinutes`] and its kin).
    NetDeviceLease = "Device: {}. Lease: {}.", "Dispositivo: {}. Concesión: {}.";
    NetWifiLater = "Wi-Fi: coming in a later version.",
        "Wi-Fi: llegará en una versión posterior.";
    LeaseUnderMinute = "under a minute left", "queda menos de un minuto";
    /// The argument is the minutes.
    LeaseMinutes = "{} min left", "quedan {} min";
    /// The arguments are the hours, then the minutes.
    LeaseHours = "{} h {} min left", "quedan {} h {} min";

    // ---- Arthur's intro card and the help card (`neo-atrium` intro) ----
    IntroGreeting = "Good day. I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests.",
        "Buen día. Soy Arthur: Assistant for Real-time Tasks, Home automation and User Requests.";
    IntroName = "I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests.",
        "Soy Arthur: Assistant for Real-time Tasks, Home automation and User Requests.";
    IntroHousehold = "The household side comes later; for now, I look after this computer for you.",
        "Lo del hogar llegará después; por ahora, cuido esta computadora por ti.";
    IntroPrivacy = "I work entirely on this machine, and I don't send what you tell me anywhere.",
        "Trabajo solo en esta máquina y no envío a ningún lado lo que me dices.";
    IntroUsbOnly = "On this machine I can't keep anything once it's switched off, not yet.",
        "En esta máquina aún no puedo conservar nada una vez que se apaga.";
    IntroAsk = "Ask in your own words: \"What does a CPU do?\", for instance.",
        "Pregunta con tus palabras, por ejemplo: \"¿Qué hace una CPU?\".";
    IntroList = "\"list /\" shows you the files on this machine.",
        "\"list /\" te muestra los archivos de esta máquina.";
    IntroListStill = "\"list /\" still shows you the files on this machine.",
        "\"list /\" sigue mostrándote los archivos de esta máquina.";
    IntroExit = "Type help for what you can do yourself. Esc always brings you back here.",
        "Escribe help para ver lo que puedes hacer tú. Esc siempre te trae aquí.";
    HelpTitle = "What you can do yourself, without asking me:",
        "Lo que puedes hacer tú, sin preguntarme:";
    HelpEsc = "Esc sets me aside and brings you back to Home.",
        "Esc me deja a un lado y te lleva de vuelta a Inicio.";
    HelpList = "\"list /\" opens the Lens: a read-only view of the files on this machine.",
        "\"list /\" abre Lens: una vista de solo lectura de los archivos de esta máquina.";
    HelpChips = "The chips on the left switch between me and the Lens; Tab reaches them.",
        "Los botones de la izquierda alternan entre Lens y yo; puedes acceder a ellos con Tab.";
    HelpDrag = "Drag a window by its title bar to move it, or by an edge to resize it.",
        "Arrastra una ventana por su barra de título para moverla, o por un borde para ajustarla.";
    HelpType = "Start typing to ask me something, or press Esc to close this card.",
        "Empieza a escribir para preguntarme algo, o presiona Esc para cerrar esta tarjeta.";
    /// Home's affordance while no card shows.
    IntroHint = "Type help, or click here, for what you can do yourself.",
        "Escribe help, o haz clic aquí, para ver lo que puedes hacer tú.";
    IntroModelAbsent = "I can hear you, but my model isn't on this system yet. Boot the LogansOS USB it came with.",
        "Te oigo, pero aún no tengo mi modelo. Arranca el USB de LogansOS que vino con el equipo.";
    IntroModelDamaged = "My model file is damaged, so I won't use it. Re-create the USB from the original image.",
        "Mi modelo está dañado, así que no lo usaré. Vuelve a crear el USB con la imagen original.";
    IntroModelLoadFailed = "My model didn't load, so I'm running without it. The details are in /model.",
        "Mi modelo no se cargó, así que funciono sin él. Los detalles están en /model.";
    /// The arguments are the GB this computer has, then the GB needed.
    IntroTooLittleMemory = "This computer has {} GB of memory; my smallest brain needs {} GB.",
        "Esta computadora tiene {} GB de memoria; mi cerebro más pequeño necesita {} GB.";
    IntroCantThink = "I can't think yet, but \"list /\" still shows you the files on this machine.",
        "Aún no puedo pensar, pero \"list /\" sigue mostrándote los archivos de esta máquina.";
    /// A card's second button.
    ButtonBack = "[ Back ]", "[ Volver ]";

    // ---- The clock (`neo-atrium` clock, `libneo` civil) ----
    GoodMorning = "Good morning", "Buenos días";
    GoodAfternoon = "Good afternoon", "Buenas tardes";
    GoodEvening = "Good evening", "Buenas noches";
    /// The intro's first row with a wall clock: greeting, time, date.
    GreetingRow = "{}. It's {} on {}.", "{}. Ahora: {}, {}.";
    Sunday = "Sunday", "domingo";
    Monday = "Monday", "lunes";
    Tuesday = "Tuesday", "martes";
    Wednesday = "Wednesday", "miércoles";
    Thursday = "Thursday", "jueves";
    Friday = "Friday", "viernes";
    Saturday = "Saturday", "sábado";
    January = "January", "enero";
    February = "February", "febrero";
    March = "March", "marzo";
    April = "April", "abril";
    May = "May", "mayo";
    June = "June", "junio";
    July = "July", "julio";
    August = "August", "agosto";
    September = "September", "septiembre";
    October = "October", "octubre";
    November = "November", "noviembre";
    December = "December", "diciembre";
    /// The 12-hour clock's suffixes, with their leading space.
    Am = " AM", " a. m.";
    Pm = " PM", " p. m.";
    /// S6b: the long date (`libneo::civil`). The arguments, by index, are
    /// the weekday, the month, the day and the year.
    DateLong = "{0}, {1} {2}, {3}", "{0} {2} de {1} de {3}";

    // ---- Home's verbs: the power card (`neo-atrium` power) ----
    PowerAskShutDown = "Shut down this computer?", "¿Apagar esta computadora?";
    PowerShutDown = "Shut down", "Apagar";
    PowerAskRestart = "Restart this computer?", "¿Reiniciar esta computadora?";
    PowerRestart = "Restart", "Reiniciar";
    PowerWarning = "Open apps close first; anything they haven't saved is lost.",
        "Las apps abiertas se cierran antes; se pierde lo que no hayan guardado.";
    PowerConfirmHint = "Enter to go ahead; any other key goes back.",
        "Enter para continuar; cualquier otra tecla vuelve atrás.";
    PowerShuttingDown = "Shutting down...", "Apagando...";
    PowerRestarting = "Restarting...", "Reiniciando...";
    PowerBusy = "It's already shutting down.", "Ya se está apagando.";
    PowerUnsupported = "This system can't do that from here.",
        "Este sistema no puede hacer eso desde aquí.";
    PowerFailed = "That didn't work.", "No funcionó.";
    /// The argument is the (negative) errno.
    PowerError = "(error {}) Press any key.", "(error {}) Presiona cualquier tecla.";

    // ---- Home's verbs: the clock rows (`neo-init` desktop session) ----
    /// The arguments are the zone's name and its offset label.
    ZoneRowNow = "Time zone: {} ({})", "Zona horaria: {} ({})";
    ClockKeepsLocal = "This computer's clock keeps local time (as Windows does).",
        "El reloj de esta computadora guarda la hora local (como Windows).";
    ClockKeepsUtc = "This computer's clock keeps UTC.",
        "El reloj de esta computadora guarda la hora UTC.";
    ClockLegacy1 = "Saved. This system shows the computer's clock",
        "Guardado. Este sistema muestra el reloj de la computadora";
    ClockLegacy2 = "as it reads, without a time zone.",
        "tal como lo lee, sin zona horaria.";
    ClockSetZone = "Set your zone: timezone America/New_York",
        "Define tu zona: timezone America/New_York";
    ClockChangeZone = "Change it: timezone Area/City", "Cámbiala: timezone Area/City";
    ClockRestart = "Restart to read this computer's clock this way.",
        "Reinicia para leer así el reloj de esta computadora.";
    ClockOffUtc = "Off by whole hours? Type: rtc utc",
        "¿Desfasada en horas exactas? Escribe: rtc utc";
    ClockOffLocal = "Off by whole hours? Type: rtc local",
        "¿Desfasada en horas exactas? Escribe: rtc local";
    ClockTitle = "The clock", "El reloj";
    /// The argument is the zone's name.
    ClockZoneSet = "Time zone set to {}.", "Zona horaria definida: {}.";
    /// The argument is what was typed.
    ClockNoZone = "There's no time zone called \"{}\".",
        "No hay ninguna zona horaria llamada \"{}\".";
    ClockTypeZone = "Type timezone Area/City, for example",
        "Escribe timezone Area/City, por ejemplo";
    ClockTypeZoneExamples = "timezone America/New_York or timezone Europe/London.",
        "timezone America/New_York o timezone Europe/London.";
    ClockDidYouMean = "Did you mean:", "Quizás quisiste decir:";
    ClockSavedLocal = "Saved: this computer's clock keeps local time.",
        "Guardado: el reloj de esta computadora guarda la hora local.";
    ClockSavedUtc = "Saved: this computer's clock keeps UTC.",
        "Guardado: el reloj de esta computadora guarda la hora UTC.";
    /// S6b: Home's readout, after the time while the zone isn't set (S6a
    /// missed it: the literal sat in neo-init's clock module).
    ClockReadoutZoneNotSet = " (zone not set)", " (zona sin definir)";
}

/// The table for `lang`.
const fn table(lang: Lang) -> &'static [&'static str; MSG_COUNT] {
    match lang {
        Lang::En => &EN,
        Lang::Es => &ES,
    }
}

/// `msg` in `lang`, and whether the English fallback was taken (an empty
/// entry). A KAT holds that no entry falls back today.
pub const fn lookup(lang: Lang, msg: Msg) -> (&'static str, bool) {
    let s = table(lang)[msg as usize];
    if s.is_empty() {
        (EN[msg as usize], true)
    } else {
        (s, false)
    }
}

/// `msg` in `lang`, falling back to English.
pub const fn text(lang: Lang, msg: Msg) -> &'static str {
    lookup(lang, msg).0
}

/// One piece of a template: literal text, or an argument slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Piece<'a> {
    Text(&'a str),
    /// The argument with this index: `{}` takes the next in order, `{N}`
    /// (one digit) takes argument `N`.
    Arg(usize),
}

/// The pieces of `template`, in order (no allocation: `libneo::civil` fills
/// a fixed buffer with them). A `{` that starts neither `{}` nor `{N}` is
/// text.
pub fn pieces(template: &str) -> impl Iterator<Item = Piece<'_>> {
    let mut rest = template;
    let mut next = 0usize;
    core::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let b = rest.as_bytes();
        if b[0] == b'{' {
            if b.get(1) == Some(&b'}') {
                rest = &rest[2..];
                next += 1;
                return Some(Piece::Arg(next - 1));
            }
            if b.len() >= 3 && b[1].is_ascii_digit() && b[2] == b'}' {
                rest = &rest[3..];
                return Some(Piece::Arg(usize::from(b[1] - b'0')));
            }
        }
        // Text up to the next `{` after the first character (which may be a
        // `{` that starts no slot, or a multi-byte character).
        let first = rest.chars().next().map_or(1, char::len_utf8);
        let end = rest[first..].find('{').map_or(rest.len(), |i| i + first);
        let (text, tail) = rest.split_at(end);
        rest = tail;
        Some(Piece::Text(text))
    })
}

/// How many arguments `template` takes: one per `{}`, and one past the
/// highest `{N}`.
pub fn arg_count(template: &str) -> usize {
    let (mut seq, mut max) = (0usize, 0usize);
    let mut rest = template;
    while let Some(at) = rest.find('{') {
        let b = &rest.as_bytes()[at..];
        if b.get(1) == Some(&b'}') {
            seq += 1;
        } else if b.len() >= 3 && b[1].is_ascii_digit() && b[2] == b'}' {
            max = max.max(usize::from(b[1] - b'0') + 1);
        }
        rest = &rest[at + 1..];
    }
    seq.max(max)
}

/// `template` with each slot replaced by its argument. A slot with no
/// argument stays as it is (`{}`); spare arguments are ignored (a KAT holds
/// every table's counts equal, so neither happens).
pub fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    for piece in pieces(template) {
        match piece {
            Piece::Text(t) => out.push_str(t),
            Piece::Arg(i) => match args.get(i) {
                Some(a) => out.push_str(a),
                None => out.push_str("{}"),
            },
        }
    }
    out
}

/// [`fill`] on `msg` in `lang`.
pub fn format(lang: Lang, msg: Msg, args: &[&str]) -> String {
    fill(text(lang, msg), args)
}

/// F1a: the macro for [`files`], a catalog with its own tables and lookups.
/// It sits here, after the main catalog, so the main catalog's source (and
/// the image's binaries built from it) is unchanged.
macro_rules! files_catalog {
    ($($(#[$doc:meta])* $id:ident = $en:literal, $es:literal;)*) => {
        /// One string Files shows. The variant's position is its index into
        /// each table.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum Msg {
            $($(#[$doc])* $id,)*
        }

        impl Msg {
            /// Every id, in table order.
            pub const ALL: &'static [Msg] = &[$(Msg::$id,)*];
        }

        /// How many ids there are.
        pub const MSG_COUNT: usize = Msg::ALL.len();

        /// The English table.
        const EN: [&str; MSG_COUNT] = [$($en,)*];

        /// The Spanish table (neutral Latin American Spanish).
        const ES: [&str; MSG_COUNT] = [$($es,)*];

        /// The table for `lang`.
        const fn table(lang: $crate::Lang) -> &'static [&'static str; MSG_COUNT] {
            match lang {
                $crate::Lang::En => &EN,
                $crate::Lang::Es => &ES,
            }
        }

        /// `msg` in `lang`, and whether the English fallback was taken (an
        /// empty entry). A KAT holds that no entry falls back.
        pub const fn lookup(lang: $crate::Lang, msg: Msg) -> (&'static str, bool) {
            let s = table(lang)[msg as usize];
            if s.is_empty() {
                (EN[msg as usize], true)
            } else {
                (s, false)
            }
        }

        /// `msg` in `lang`, falling back to English.
        pub const fn text(lang: $crate::Lang, msg: Msg) -> &'static str {
            lookup(lang, msg).0
        }

        /// [`fill`]($crate::fill) on `msg` in `lang`.
        pub fn format(lang: $crate::Lang, msg: Msg, args: &[&str]) -> alloc::string::String {
            $crate::fill(text(lang, msg), args)
        }
    };
}

/// F1a: the Files app's rows (`files-core`), in English and Spanish only: its
/// kinds, sizes, hints, confirmations and every F0 error. A catalog of its own so the
/// image's binaries, which link the main catalog, don't carry them: only
/// `files-core` (linked into no image until F1b) references this table. The
/// same rules and KATs hold for it: both tables complete, English pinned in
/// `fixtures/en.txt`, every row in the face and inside the card.
pub mod files {
    files_catalog! {
        FilesTitle = "Files", "Archivos";
        /// The kind column.
        FilesKindFolder = "Folder", "Carpeta";
        FilesKindFile = "File", "Archivo";
        FilesKindModel = "Model", "Modelo";
        FilesKindLink = "Link", "Enlace";
        /// The size column; the argument is the number, already written with
        /// [`Msg::FilesDecimalMark`].
        FilesSizeBytes = "{} B", "{} B";
        FilesSizeKb = "{} KB", "{} KB";
        FilesSizeMb = "{} MB", "{} MB";
        /// The decimal mark in a size (1.5 KB, 1,5 KB).
        FilesDecimalMark = ".", ",";
        /// A folder's size column: its entry count.
        FilesItemsOne = "1 item", "1 elemento";
        FilesItemsMany = "{} items", "{} elementos";
        FilesEmpty = "This folder is empty.", "Esta carpeta está vacía.";
        /// The argument is the limit (64).
        FilesFolderFullNote = "This folder holds {} items, the most it can.",
            "Esta carpeta tiene {} elementos, el máximo posible.";
        /// The first new folder's name; later ones add " 2", " 3", ...
        FilesNewFolderName = "New folder", "Carpeta nueva";
        FilesHintBrowse = "Enter opens. Backspace goes up. N makes a folder. F2 renames.",
            "Enter abre. Retroceso sube. N crea una carpeta. F2 cambia el nombre.";
        FilesHintBrowse2 = "Del deletes. C copies. V pastes.", "Supr elimina. C copia. V pega.";
        FilesRenameTitle = "Rename", "Cambiar nombre";
        FilesHintNaming = "Type a name. Enter to save, Esc to cancel.",
            "Escribe un nombre. Enter para guardar, Esc para cancelar.";
        FilesConfirmDeleteFile = "Delete this file? This can't be undone.",
            "¿Eliminar este archivo? No se puede deshacer.";
        FilesConfirmDeleteFolder = "Delete this empty folder?", "¿Eliminar esta carpeta vacía?";
        FilesConfirmHint = "Enter to delete; any other key goes back.",
            "Enter para eliminar; cualquier otra tecla vuelve atrás.";
        FilesNameEmpty = "A name can't be empty.", "El nombre no puede estar vacío.";
        /// The argument is the limit (47).
        FilesNameTooLong = "A name can be at most {} bytes long.",
            "Un nombre puede tener como máximo {} bytes.";
        FilesNameSlash = "A name can't contain /.", "Un nombre no puede contener /.";
        FilesNameControl = "A name can't contain control characters.",
            "Un nombre no puede contener caracteres de control.";
        FilesNameDot = "A name can't be . or ..", "Un nombre no puede ser . ni ..";
        FilesErrStale = "That folder moved or was removed. This is the nearest one above it.",
            "Esa carpeta se movió o se eliminó. Esta es la más cercana por encima.";
        FilesErrExists = "Something here already has that name.", "Ya hay algo aquí con ese nombre.";
        FilesErrNotEmpty = "That folder isn't empty. Empty it first.",
            "Esa carpeta no está vacía. Vacíala primero.";
        FilesErrNoSpace = "There isn't enough space on the disk.",
            "No hay espacio suficiente en el disco.";
        FilesErrRefused = "That isn't allowed here.", "Eso no está permitido aquí.";
        /// The argument is the limit (64).
        FilesErrFolderFull = "This folder is full: {} items at most.",
            "Esta carpeta está llena: {} elementos como máximo.";
        FilesErrNotFound = "That item is gone; the list is up to date now.",
            "Ese elemento ya no está; la lista ya está al día.";
        FilesErrBusy = "The system needs that item, so it stays.",
            "El sistema necesita ese elemento, así que se queda.";
        FilesErrRetry = "The disk was busy. Try again.", "El disco estaba ocupado. Inténtalo de nuevo.";
        FilesErrIo = "The disk couldn't do that.", "El disco no pudo hacerlo.";
        FilesErrInvalid = "That can't be done here; the disk may be full.",
            "Eso no se puede hacer aquí; puede que el disco esté lleno.";
        /// The argument is the (negative) errno.
        FilesErrOther = "That didn't work (error {}).", "No funcionó (error {}).";
        FilesRootUnavailable = "Files can't read its folders right now.",
            "Archivos no puede leer sus carpetas ahora.";
        FilesCopied = "Copied. Open a folder and paste it there.",
            "Copiado. Abre una carpeta y pégalo ahí.";
        /// The argument is the limit in KB (32).
        FilesCopyTooBig = "Only files up to {} KB copy for now. Nothing was copied.",
            "Por ahora solo se copian archivos de hasta {} KB. No se copió nada.";
        FilesCopyNotFile = "Only files copy for now, not folders.",
            "Por ahora solo se copian archivos, no carpetas.";
        FilesPasted = "Pasted.", "Pegado.";
        FilesNothingToPaste = "Copy a file first.", "Copia un archivo primero.";
        FilesNoApp = "No app opens this.", "Ninguna app abre esto.";
        FilesModelArthur = "Models are Arthur's; no app opens them.",
            "Los modelos son de Arthur; ninguna app los abre.";
    }
}
