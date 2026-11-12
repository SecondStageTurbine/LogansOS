// SPDX-License-Identifier: MPL-2.0
//! S6b's host KATs (`i18n-catalog`): the Language page, and the wizard
//! redrawn in Spanish. Every row is written out by hand, not taken from the
//! catalog, so a wrong id or a wrong fill shows here.

use super::*;

#[cfg(not(feature = "keyboard-layouts"))]
const HINT_EN: &str = "Up/Down to move, Enter to choose, Esc to go back.";
const HINT_ES: &str = "Arriba/Abajo para moverte, Enter para elegir, Esc para volver.";

fn rows(w: &Wizard) -> Vec<String> {
    let r = w.rows();
    assert!(r.len() <= MAX_ROWS, "{r:?}");
    r
}

/// Begin, then the Language page.
fn on_language(w: Wizard) -> Wizard {
    let mut w = w;
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Language));
    w
}

/// Down to Español and Enter: the page redraws in Spanish.
fn choose_spanish(w: &mut Wizard) {
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    assert_eq!(w.lang(), Lang::Es);
}

#[test]
fn the_language_page_is_shown_after_welcome() {
    const { assert!(LANGUAGE_PAGE) };
    let pages = Wizard::new(false).pages();
    assert_eq!(&pages[..2], [Page::Welcome, Page::Language]);
}

#[cfg(not(feature = "keyboard-layouts"))]
#[test]
fn the_language_page_in_english_then_spanish() {
    let mut w = on_language(Wizard::new(false));
    assert_eq!(
        rows(&w),
        [
            "Language (1 of 4)",
            "Choose the language for setup and Home.",
            "Arthur answers in English for now.",
            "> [x] English",
            "  [ ] Español",
            "  Continue",
            HINT_EN,
        ]
    );
    choose_spanish(&mut w);
    assert_eq!(
        rows(&w),
        [
            "Idioma (1 de 4)",
            "Elige el idioma de la configuración y de Inicio.",
            "Por ahora, Arthur responde en inglés.",
            "  [ ] English",
            "> [x] Español",
            "  Continuar",
            HINT_ES,
        ]
    );
    // Back to English redraws in English.
    assert_eq!(w.on(Input::Up), Outcome::Redraw);
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    assert_eq!(rows(&w)[0], "Language (1 of 4)");
}

/// The gate's walk in Spanish: Language (Español), Time zone, Comfort (both
/// on), Try.
#[cfg(not(feature = "keyboard-layouts"))]
#[test]
fn the_walk_in_spanish() {
    let mut w = on_language(Wizard::new(false));
    choose_spanish(&mut w);
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::TimeZone));
    assert_eq!(
        rows(&w),
        [
            "Zona horaria (2 de 4)",
            "¿Dónde estás? En Inicio: timezone Area/City.",
            "> [x] UTC",
            "  [ ] America/New_York",
            "  [ ] Europe/London",
            "  [ ] Australia/Sydney",
            "  Continuar",
            HINT_ES,
        ]
    );
    for _ in 0..4 {
        w.on(Input::Down);
    }
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Comfort));
    assert_eq!(
        rows(&w),
        [
            "Comodidad (3 de 4)",
            "Haz que la pantalla sea más tranquila, si quieres.",
            "> [ ] Reducir movimiento",
            "  [ ] Reducir transparencia",
            "  Continuar",
            HINT_ES,
        ]
    );
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Try));
    assert_eq!(
        rows(&w),
        [
            "Probar LogansOS (4 de 4)",
            "Zona horaria: UTC",
            "Reducir movimiento: sí. Reducir transparencia: no.",
            "> Probar LogansOS",
            HINT_ES,
        ]
    );
    assert_eq!(w.on(Input::Choose), Outcome::Done(Finish::Try));
    assert_eq!(
        w.choices(),
        Choices {
            zone: 0,
            reduce_motion: true,
            reduce_transparency: false,
            tz: None,
            lang: Lang::Es,
        }
    );
}

/// Esc back to Welcome, and its Skip confirmation, in Spanish.
#[test]
fn welcome_and_skip_in_spanish() {
    let mut w = on_language(Wizard::new(false));
    choose_spanish(&mut w);
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::Welcome));
    assert_eq!(
        rows(&w),
        [
            "Configurar LogansOS",
            "Unas pocas pantallas breves. Puedes cambiar todo después.",
            "> Comenzar",
            "  Omitir la configuración",
            HINT_ES,
        ]
    );
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::SkipConfirm));
    assert_eq!(
        rows(&w),
        [
            "¿Omitir la configuración?",
            "Lo predeterminado es UTC, con movimiento y transparencia completos.",
            "> Omitir y usar los valores predeterminados",
            "  Volver",
            HINT_ES,
        ]
    );
}

/// The stored language (`with_lang`) starts the wizard in it.
#[test]
fn a_stored_language_starts_the_wizard_in_it() {
    let w = Wizard::new(false).with_lang(Lang::Es);
    assert_eq!(rows(&w)[0], "Configurar LogansOS");
    let w = on_language(w);
    assert_eq!(rows(&w)[3..5], ["> [ ] English", "  [x] Español"]);
}

/// S4's Network page in Spanish.
#[cfg(not(feature = "keyboard-layouts"))]
#[test]
fn the_network_page_in_spanish() {
    let net = |n| {
        let mut w = on_language(Wizard::new(false).with_network(n));
        choose_spanish(&mut w);
        w.on(Input::Down);
        assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Network));
        w
    };
    let w = net(Network::Connected {
        driver: String::from("cdc-ecm"),
        address: Some([10, 0, 2, 15]),
        lease_secs: Some(86_000),
    });
    assert_eq!(
        rows(&w),
        [
            "Red (2 de 5)",
            "Cable: conectado, 10.0.2.15",
            "Dispositivo: cdc-ecm. Concesión: quedan 23 h 53 min.",
            "Wi-Fi: llegará en una versión posterior.",
            "> Continuar",
            HINT_ES,
        ]
    );
    let w = net(Network::NoLink {
        driver: Some(String::from("rtl8125")),
    });
    assert_eq!(
        rows(&w),
        [
            "Red (2 de 5)",
            "Cable: sin enlace.",
            "Dispositivo: rtl8125.",
            "Wi-Fi: llegará en una versión posterior.",
            "> Continuar sin red",
            HINT_ES,
        ]
    );
    let w = net(Network::Unavailable);
    assert_eq!(rows(&w)[1], "El estado de la red no está disponible.");
    let w = net(Network::Connected {
        driver: String::from("cdc-ecm"),
        address: None,
        lease_secs: None,
    });
    assert_eq!(
        rows(&w)[1..3],
        [
            "Cable: conectado, aún sin dirección.",
            "Dispositivo: cdc-ecm."
        ]
    );
    assert_eq!(lease_left(Lang::Es, 0), "queda menos de un minuto");
    assert_eq!(lease_left(Lang::Es, 300), "quedan 5 min");
}

/// S5's Keyboard page in Spanish: the layouts' names come from the catalog,
/// whose English entries are `LAYOUT_NAMES`.
#[cfg(feature = "keyboard-layouts")]
#[test]
fn the_keyboard_page_in_spanish() {
    for (i, &m) in LAYOUT_MSGS.iter().enumerate() {
        assert_eq!(i18n_core::text(Lang::En, m), LAYOUT_NAMES[i]);
    }
    let mut w = on_language(Wizard::new(false));
    choose_spanish(&mut w);
    w.on(Input::Down);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Keyboard));
    assert_eq!(
        rows(&w),
        [
            "Teclado (2 de 5)",
            "Elige la distribución impresa en tus teclas.",
            "Pruébalo: _",
            "> [x] Inglés (EE. UU.)",
            "  [ ] Español (España)",
            "  Continuar",
            HINT_ES,
        ]
    );
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    assert_eq!(w.on(Input::Char(b'n')), Outcome::Redraw);
    assert_eq!(rows(&w)[2], "Pruébalo: n_");
    w.on(Input::Down);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::TimeZone));
    for _ in 0..4 {
        w.on(Input::Down);
    }
    w.on(Input::Choose);
    w.on(Input::Down);
    w.on(Input::Down);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Try));
    assert_eq!(
        rows(&w)[1..4],
        [
            "Zona horaria: UTC",
            "Teclado: Español (España)",
            "Reducir movimiento: no. Reducir transparencia: no.",
        ]
    );
}
