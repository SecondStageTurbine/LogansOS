#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Independent oracle for the first-boot setup menu's pages (F11 S1).

Written from docs/THINK-2026-11-02-setup-menu.md (§2.1's screens, §4.2's
key map, §4.3's marks, §4.5's card) and the wording the rung settled, typed
here again rather than read from setup-core: the gate compares the two.

* The scene: scripts/desktop/scene_oracle.py renders Home with an n-row
  card (numpy, no neo-atrium code), the field focused and empty, as the
  menu shows it. Digests mask the Home label boxes and the card's text box.
* The words: per row, intro_oracle's column pattern over the text box,
  from exact-area glyph coverage (scripts/fonts/area_oracle). A wrong word,
  a missing row or a shifted mark fails it.

The pages held: Welcome (captured by `setup_menu_skip` before its Esc),
Try after the gate walk (UTC, both Comfort options on), captured by
`setup_menu_walk` as it shows, and (S5) the Keyboard page in Spanish with the
gate's keys typed, captured by `setup_menu_keys`.

S6b (`fixture es`, `fixture en`): the walks of `setup_menu_lang` (Spanish
chosen on the Language page) and its control `setup_menu_lang_en` (English
kept), each page as the gate captures it. Spanish: Language, Welcome and Skip
(Esc back from Language), Time zone, Comfort and Try, then the intro card and
the help card (a Home verb's card). English: the Language page and the Try
page (their numbering counts the Language page); the control's Welcome,
intro and help cards are today's fixtures, unchanged. The Spanish words are
typed here again, not read from i18n-core.

usage: setup_oracle.py fixture > fixtures/desktop-setup/setup.rs
       setup_oracle.py fixture es > fixtures/desktop-setup/setup_lang_es.rs
       setup_oracle.py fixture en > fixtures/desktop-setup/setup_lang_en.rs
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "fonts"))
sys.path.insert(0, HERE)

import area_oracle as ao  # noqa: E402
import intro_oracle as io  # noqa: E402
import label_oracle as lo  # noqa: E402
import scene_oracle as so  # noqa: E402

HINT = "Up/Down to move, Enter to choose, Esc to go back."
# Welcome, Begin focused.
WELCOME = [
    "Set up LogansOS",
    "A few short screens. You can change any of these later.",
    "> Begin",
    "  Skip setup",
    HINT,
]
# Try after the gate walk: UTC, Reduce motion and Reduce transparency on.
TRY = [
    "Try LogansOS (3 of 3)",
    "Time zone: UTC",
    "Reduce motion: on. Reduce transparency: on.",
    "> Try LogansOS",
    HINT,
]

# F11 S5 (`setup_menu_keys`): the Keyboard page with Spanish chosen (still
# focused) and the gate's keys typed into the test field (ñ, Shift+¡, ¡, ç,
# the acute dead key then a), captured as the last one lands.
KEYBOARD_ES = [
    "Keyboard (1 of 4)",
    "Choose the layout printed on your keys.",
    "Try it: ñ¿¡çá_",
    "  [ ] English (US)",
    "> [x] Spanish (Spain)",
    "  Continue",
    HINT,
]


# S6b (`setup_menu_lang`): the walk in Spanish. The hint, and every page as
# the gate captures it.
HINT_ES = "Arriba/Abajo para moverte, Enter para elegir, Esc para volver."
LANG_ES = {
    # Español chosen (Down, Enter): the page redrawn in Spanish, focus on it.
    "LANGUAGE": [
        "Idioma (1 de 4)",
        "Elige el idioma de la configuración y de Inicio.",
        "Por ahora, Arthur responde en inglés.",
        "  [ ] English",
        "> [x] Español",
        "  Continuar",
        HINT_ES,
    ],
    # Esc back to Welcome, in Spanish.
    "WELCOME": [
        "Configurar LogansOS",
        "Unas pocas pantallas breves. Puedes cambiar todo después.",
        "> Comenzar",
        "  Omitir la configuración",
        HINT_ES,
    ],
    # Esc on Welcome: the Skip confirmation (then Esc goes back).
    "SKIP": [
        "¿Omitir la configuración?",
        "Lo predeterminado es UTC, con movimiento y transparencia completos.",
        "> Omitir y usar los valores predeterminados",
        "  Volver",
        HINT_ES,
    ],
    # S1's short list, as it shows (UTC kept).
    "TIMEZONE": [
        "Zona horaria (2 de 4)",
        "¿Dónde estás? En Inicio: timezone Area/City.",
        "> [x] UTC",
        "  [ ] America/New_York",
        "  [ ] Europe/London",
        "  [ ] Australia/Sydney",
        "  Continuar",
        HINT_ES,
    ],
    "COMFORT": [
        "Comodidad (3 de 4)",
        "Haz que la pantalla sea más tranquila, si quieres.",
        "> [ ] Reducir movimiento",
        "  [ ] Reducir transparencia",
        "  Continuar",
        HINT_ES,
    ],
    "TRY": [
        "Probar LogansOS (4 de 4)",
        "Zona horaria: UTC",
        "Reducir movimiento: no. Reducir transparencia: no.",
        "> Probar LogansOS",
        HINT_ES,
    ],
    # The intro session's card (variant A, USB-only, model ready), in Spanish:
    # Arthur's name and its English expansion stay (S6a's decision).
    "INTRO": [
        "Buen día. Soy Arthur: Assistant for Real-time Tasks, Home automation and User Requests.",
        "Lo del hogar llegará después; por ahora, cuido esta computadora por ti.",
        "Trabajo solo en esta máquina y no envío a ningún lado lo que me dices.",
        "En esta máquina aún no puedo conservar nada una vez que se apaga.",
        "Pregunta con tus palabras, por ejemplo: \"¿Qué hace una CPU?\".",
        "\"list /\" te muestra los archivos de esta máquina.",
        "Escribe help para ver lo que puedes hacer tú. Esc siempre te trae aquí.",
    ],
    # Home's `help` verb: its card.
    "HELP": [
        "Lo que puedes hacer tú, sin preguntarme:",
        "Esc me deja a un lado y te lleva de vuelta a Inicio.",
        "\"list /\" abre Lens: una vista de solo lectura de los archivos de esta máquina.",
        "Los botones de la izquierda alternan entre Lens y yo; puedes acceder a ellos con Tab.",
        "Arrastra una ventana por su barra de título para moverla, o por un borde para ajustarla.",
        "Empieza a escribir para preguntarme algo, o presiona Esc para cerrar esta tarjeta.",
    ],
}
# S6b's control (`setup_menu_lang_en`): English kept. Only the pages whose
# numbering counts the Language page are new; the rest are today's.
LANG_EN = {
    "LANGUAGE": [
        "Language (1 of 4)",
        "Choose the language for setup and Home.",
        "Arthur answers in English for now.",
        "> [x] English",
        "  [ ] Español",
        "  Continue",
        HINT,
    ],
    "TRY": [
        "Try LogansOS (4 of 4)",
        "Time zone: UTC",
        "Reduce motion: off. Reduce transparency: off.",
        "> Try LogansOS",
        HINT,
    ],
}


def emit(face, L, W, H, pages):
    for name, rows in pages:
        n = len(rows)
        print("pub const %s_ROWS: [&str; %d] = [" % (name, n))
        for r in rows:
            print('    "%s",' % io.esc(r))
        print("];")
        print("/// The %s card's text box at 1920x1080 (masked in the digest)." % name.lower())
        print("pub const %s_TEXT_BOX: (usize, usize, usize, usize) = (%d, %d, %d, %d);"
              % ((name,) + so.card_text(L, n)))
        rgb, Lr = so.render(W, H, "glass", True, "home", card_rows=n)
        d = so.digest(rgb, dict(Lr, labels=Lr["home_labels"] + [so.card_text(L, n)]))
        print("pub const %s_GLASS_1920X1080: u64 = 0x%016x;" % (name, d))
        specs = io.rows_spec(face, L, rows, False)
        out = [io.pattern(face, rgb, s) for s in specs]
        print("/// %s rows: band (x, y, w, h), green threshold, column pattern." % name)
        print("pub const %s_PATTERNS: [Row; %d] = [" % (name, len(out)))
        for band, thr, pat in out:
            print("    ((%d, %d, %d, %d), %d, \"%s\")," % (band + (thr, pat)))
        print("];")


def fixture_lang(lang):
    face = ao.Face(lo.FONT)
    W, H = 1920, 1080
    L = so.layout(W, H)
    pages = LANG_ES if lang == "es" else LANG_EN
    print("// SPDX-License-Identifier: MPL-2.0")
    print("// GENERATED by scripts/desktop/setup_oracle.py fixture %s — do not edit." % lang)
    if lang == "es":
        print("// F11 S6b (`setup_menu_lang`): every page of the walk in Spanish, the intro")
        print("// card and the help card, as the gate captures them.")
    else:
        print("// F11 S6b's control (`setup_menu_lang_en`): the Language and Try pages in")
        print("// English (the rest of the walk is today's fixtures).")
    print("// Digests: FNV-1a over every visible pixel, the three Home label boxes and")
    print("// the card's text box hashed as zero. Rows: per text row, its band, the")
    print("// green threshold and a column pattern from exact-area glyph coverage")
    print("// (# inked, . blank, ? either). Independent of neo-atrium, neo-text,")
    print("// setup-core and i18n-core.")
    print("/// A text row: its band (x, y, w, h), green threshold, column pattern.")
    print("pub type Row = ((usize, usize, usize, usize), u8, &'static str);")
    emit(face, L, W, H, list(pages.items()))


def fixture():
    face = ao.Face(lo.FONT)
    W, H = 1920, 1080
    L = so.layout(W, H)
    print("// SPDX-License-Identifier: MPL-2.0")
    print("// GENERATED by scripts/desktop/setup_oracle.py fixture — do not edit.")
    print("// F11 S1: the setup menu's Welcome and Try pages on Home's card, and S5's")
    print("// Keyboard page (Spanish, the field typed). Digests:")
    print("// FNV-1a over every visible pixel, the three Home label boxes and the")
    print("// card's text box hashed as zero. Rows: per text row, its band, the green")
    print("// threshold and a column pattern from exact-area glyph coverage (# inked,")
    print("// . blank, ? either). Independent of neo-atrium, neo-text and setup-core.")
    print("/// A text row: its band (x, y, w, h), green threshold, column pattern.")
    print("pub type Row = ((usize, usize, usize, usize), u8, &'static str);")
    emit(face, L, W, H, (("WELCOME", WELCOME), ("TRY", TRY), ("KEYBOARD_ES", KEYBOARD_ES)))


def main():
    if len(sys.argv) == 2 and sys.argv[1] == "fixture":
        fixture()
        return
    if len(sys.argv) == 3 and sys.argv[1] == "fixture" and sys.argv[2] in ("es", "en"):
        fixture_lang(sys.argv[2])
        return
    print(__doc__)
    sys.exit(2)


if __name__ == "__main__":
    main()
