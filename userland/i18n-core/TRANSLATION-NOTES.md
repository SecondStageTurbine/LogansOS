<!-- SPDX-License-Identifier: MPL-2.0 -->
# i18n-core: translation notes (Spanish)

The Spanish table in `src/lib.rs`, row by row, for the wording review. Generated from the table, so the
two can't disagree; the KATs in `src/tests.rs` hold the English column to today's literals and every row
inside the card.

## Choices that apply to every row

- **Register.** Neutral Latin American Spanish, informal *tú* ("Escribe", "Puedes"), matching the English's
  short imperatives. "computadora", not "ordenador"; "presiona", not "pulsa".
- **Commands stay as typed.** `help`, `list /`, `timezone Area/City`, `rtc utc`, `rtc local` and the power
  verbs are what the parsers read. They appear unchanged inside the Spanish rows.
- **Names stay.** LogansOS, Arthur (and its English acronym), Lens, UTC, USB, CPU, Windows, Enter, Esc, Tab.
- **Persona rules hold.** No exclamations (so no "¡" either), no emoji, economy.
- **Characters.** Beyond ASCII the table uses only `¿ á é í ñ ó ú`, all in Inter Regular (KAT). The intro
  persona KAT in `neo-atrium` allows printable ASCII only; S6b relaxes it to "covered by the face" for the
  Spanish table.
- **Width.** Every row fits the card with its widest realistic arguments (a 30-byte zone name, a 32-byte
  typed name, the longest date). The widest Spanish row is IntroModelAbsent at 96.6% of the card.

## Every row

| id | English | Spanish | note |
|---|---|---|---|
| `SetupHint` | Up/Down to move, Enter to choose, Esc to go back. | Arriba/Abajo para moverte, Enter para elegir, Esc para volver. | "Up/Down" are the arrow keys: "Arriba/Abajo". **Enter** and **Esc** keep their key-cap names. |
| `SetupSearchHint` | Type to search. Up/Down, Enter to choose. | Escribe para buscar. Arriba/Abajo, Enter para elegir. | As SetupHint. |
| `SetupBegin` | Begin | Comenzar |  |
| `SetupSkip` | Skip setup | Omitir la configuración | "Omitir" for "skip" throughout. |
| `SetupSkipDefaults` | Skip setup and use the defaults | Omitir y usar los valores predeterminados | Shortened: "Skip and use the defaults" (the page title already says it is setup). |
| `SetupBack` | Back | Volver | "Volver" for "back", here and on the card buttons. |
| `SetupContinue` | Continue | Continuar |  |
| `SetupTryOption` | Try LogansOS | Probar LogansOS |  |
| `SetupReduceMotionOption` | {} Reduce motion | {} Reducir movimiento |  |
| `SetupReduceTransparencyOption` | {} Reduce transparency | {} Reducir transparencia |  |
| `SetupTitleWelcome` | Set up LogansOS | Configurar LogansOS |  |
| `SetupTitleSkip` | Skip setup? | ¿Omitir la configuración? |  |
| `SetupTitleLanguage` | Language | Idioma |  |
| `SetupTitleKeyboard` | Keyboard | Teclado |  |
| `SetupTitleTimeZone` | Time zone | Zona horaria |  |
| `SetupTitleComfort` | Comfort | Comodidad | "Comodidad"; "Accesibilidad" would claim more than two toggles. |
| `SetupTitleTry` | Try LogansOS | Probar LogansOS |  |
| `SetupTitlePage` | {} ({} of {}) | {} ({} de {}) | "{} ({} de {})": the page counter. Positional, same order. |
| `SetupBodyWelcome` | A few short screens. You can change any of these later. | Unas pocas pantallas breves. Puedes cambiar todo después. |  |
| `SetupBodySkip` | The defaults are UTC, with full motion and transparency. | Lo predeterminado es UTC, con movimiento y transparencia completos. | "Lo predeterminado es…" reads more naturally than a literal "The defaults are…". |
| `SetupBodyLanguage` | English is the only language for now. | Por ahora, el inglés es el único idioma. |  |
| `SetupBodyKeyboard` | US is the only keyboard layout for now. | Por ahora, la única distribución de teclado es la de EE. UU. | "distribución de teclado" for layout; "EE. UU." with the RAE spacing. |
| `SetupBodyTimeZoneNoSearch` | Where are you? At Home: timezone Area/City. | ¿Dónde estás? En Inicio: timezone Area/City. | **Home** is "Inicio" everywhere (the Home view). `timezone Area/City` is the command and stays as typed. |
| `SetupBodyZoneSet` | Time zone: {}. The clock is set. | Zona horaria: {}. El reloj está en hora. | "The clock is set" means set to the right time: "El reloj está en hora". |
| `SetupBodyZoneLater` | Change it later at Home: timezone Area/City. | Cámbiala después en Inicio: timezone Area/City. | "Cámbiala" (la zona). The command stays. |
| `SetupBodySearchEmpty` | Type a city to search, or pick one. | Escribe una ciudad para buscar, o elige una. |  |
| `SetupBodySearchNone` | Search: {}  (no zone matches) | Buscar: {}  (ninguna zona coincide) | Keeps the two spaces before the bracket. |
| `SetupBodySearch` | Search: {} | Buscar: {} |  |
| `SetupBodyComfort` | Make the screen calmer, if you like. | Haz la pantalla más tranquila, si quieres. | Informal *tú* ("Haz…, si quieres"), as everywhere in this table. |
| `SetupNotSet` | not set | sin definir | "sin definir" (not "no configurado"). |
| `ZoneRow` | Time zone: {} | Zona horaria: {} |  |
| `SetupSummaryZoneClockNotSet` | Time zone: {} (clock not set) | Zona horaria: {} (reloj sin ajustar) | "reloj sin ajustar". |
| `ZoneRowNotSet` | Time zone: not set | Zona horaria: sin definir |  |
| `SetupSummaryComfort` | Reduce motion: {}. Reduce transparency: {}. | Reducir movimiento: {}. Reducir transparencia: {}. |  |
| `On` | on | sí | "sí"/"no" for the summary's on/off. "activado/desactivado" is longer and gendered. |
| `Off` | off | no | See On. |
| `SetupNoticeZoneInUse` | Couldn't save the time zone. In use: {}. | No se pudo guardar la zona horaria. En uso: {}. |  |
| `SetupNoticeZoneRestart` | Couldn't save the time zone. Restart to check it. | No se pudo guardar la zona horaria. Reinicia para comprobarla. | "comprobarla" (la zona). |
| `IntroGreeting` | Good day. I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests. | Buen día. Soy Arthur: Assistant for Real-time Tasks, Home automation and User Requests. | Arthur's name is an English acronym, so its expansion stays in English (translating it breaks the acronym). **Reviewer: check whether J.c. wants a Spanish gloss instead.** |
| `IntroName` | I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests. | Soy Arthur: Assistant for Real-time Tasks, Home automation and User Requests. | As IntroGreeting. |
| `IntroHousehold` | The household side comes later; for now, I look after this computer for you. | Lo del hogar llegará después; por ahora, cuido esta computadora por ti. | "Lo del hogar" for "the household side"; "cuido esta computadora por ti". |
| `IntroPrivacy` | I work entirely on this machine, and I don't send what you tell me anywhere. | Trabajo solo en esta máquina y no envío a ningún lado lo que me dices. |  |
| `IntroUsbOnly` | On this machine I can't keep anything once it's switched off, not yet. | En esta máquina aún no puedo conservar nada una vez que se apaga. |  |
| `IntroAsk` | Ask in your own words: "What does a CPU do?", for instance. | Pregunta con tus palabras, por ejemplo: "¿Qué hace una CPU?". | The example question is translated ("¿Qué hace una CPU?"); Arthur answers in English until the model is prompted in Spanish (S6b or later). |
| `IntroList` | "list /" shows you the files on this machine. | "list /" te muestra los archivos de esta máquina. | `list /` is a command and stays. |
| `IntroListStill` | "list /" still shows you the files on this machine. | "list /" sigue mostrándote los archivos de esta máquina. |  |
| `IntroExit` | Type help for what you can do yourself. Esc always brings you back here. | Escribe help para ver lo que puedes hacer tú. Esc siempre te trae aquí. | `help` is the command word; "te trae aquí" drops "back" for width. |
| `HelpTitle` | What you can do yourself, without asking me: | Lo que puedes hacer tú, sin preguntarme: |  |
| `HelpEsc` | Esc sets me aside and brings you back to Home. | Esc me deja a un lado y te lleva de vuelta a Inicio. |  |
| `HelpList` | "list /" opens the Lens: a read-only view of the files on this machine. | "list /" abre Lens: una vista de solo lectura de los archivos de esta máquina. | **Lens** is the app's name and stays (the chip says "Lens"). |
| `HelpChips` | The chips on the left switch between me and the Lens; Tab reaches them. | Los botones de la izquierda alternan entre Lens y yo; Tab los alcanza. | "chips" become "botones" (the Spine chips are buttons to the user). "entre Lens y yo" puts Arthur last, as Spanish politeness has it. |
| `HelpDrag` | Drag a window by its title bar to move it, or by an edge to resize it. | Arrastra una ventana por su barra de título para moverla, o por un borde para ajustarla. | "ajustarla" for "resize it", shorter than "cambiar su tamaño" (this row is 91% of the card). |
| `HelpType` | Start typing to ask me something, or press Esc to close this card. | Empieza a escribir para preguntarme algo, o presiona Esc para cerrar esta tarjeta. | "presiona" (Latin American) rather than "pulsa" (Spain). |
| `IntroHint` | Type help, or click here, for what you can do yourself. | Escribe help, o haz clic aquí, para ver lo que puedes hacer tú. | `help` stays. |
| `IntroModelAbsent` | I can hear you, but my model isn't on this system yet. Boot the LogansOS USB it came with. | Te escucho, pero mi modelo aún no está en este sistema. Arranca con el USB de LogansOS. | Shortened for width (the widest Spanish row, 96.6% of the card): "Boot the LogansOS USB **it came with**" loses "it came with". **Reviewer: check the shorter wording is acceptable.** |
| `IntroModelDamaged` | My model file is damaged, so I won't use it. Re-create the USB from the original image. | Mi modelo está dañado, así que no lo usaré. Vuelve a crear el USB con la imagen original. | Shortened for width: the first draft overflowed the card by 6 px. |
| `IntroModelLoadFailed` | My model didn't load, so I'm running without it. The details are in /model. | Mi modelo no se cargó, así que funciono sin él. Los detalles están en /model. | `/model` is a path and stays. |
| `IntroTooLittleMemory` | This computer has {} GB of memory; my smallest brain needs {} GB. | Esta computadora tiene {} GB de memoria; mi cerebro más pequeño necesita {} GB. | "mi cerebro más pequeño" keeps the persona's metaphor. |
| `IntroCantThink` | I can't think yet, but "list /" still shows you the files on this machine. | Aún no puedo pensar, pero "list /" sigue mostrándote los archivos de esta máquina. |  |
| `ButtonBack` | [ Back ] | [ Volver ] | Drawn as `[ Volver ]`. |
| `GoodMorning` | Good morning | Buenos días |  |
| `GoodAfternoon` | Good afternoon | Buenas tardes |  |
| `GoodEvening` | Good evening | Buenas noches | "Buenas noches" is a greeting as well as a farewell in Spanish, so it covers 18:00-04:59. |
| `GreetingRow` | {}. It's {} on {}. | {}. Ahora: {}, {}. | "{}. Ahora: {}, {}." avoids "Son las/Es la", which would need the hour's number (singular at 1 o'clock). **Reviewer: check the register.** |
| `Sunday` | Sunday | domingo | Weekday and month names are lower case in Spanish. |
| `Monday` | Monday | lunes |  |
| `Tuesday` | Tuesday | martes |  |
| `Wednesday` | Wednesday | miércoles |  |
| `Thursday` | Thursday | jueves |  |
| `Friday` | Friday | viernes |  |
| `Saturday` | Saturday | sábado |  |
| `January` | January | enero | See Sunday. **Not in S6a:** the date's order. `libneo::civil` writes "Weekday, Month D, YYYY"; Spanish wants "miércoles, 30 de septiembre de 2026". That needs a date pattern per language, which S6b adds with these names. |
| `February` | February | febrero |  |
| `March` | March | marzo |  |
| `April` | April | abril |  |
| `May` | May | mayo |  |
| `June` | June | junio |  |
| `July` | July | julio |  |
| `August` | August | agosto |  |
| `September` | September | septiembre |  |
| `October` | October | octubre |  |
| `November` | November | noviembre |  |
| `December` | December | diciembre |  |
| `Am` |  AM |  a. m. | "a. m." / "p. m." with the RAE spacing, each with the leading space the clock writes. |
| `Pm` |  PM |  p. m. | See Am. |
| `PowerAskShutDown` | Shut down this computer? | ¿Apagar esta computadora? |  |
| `PowerShutDown` | Shut down | Apagar |  |
| `PowerAskRestart` | Restart this computer? | ¿Reiniciar esta computadora? |  |
| `PowerRestart` | Restart | Reiniciar |  |
| `PowerWarning` | Open apps close first; anything they haven't saved is lost. | Las apps abiertas se cierran antes; se pierde lo que no hayan guardado. | "apps" is common in Latin America; "antes" for "first". |
| `PowerConfirmHint` | Enter to go ahead; any other key goes back. | Enter para continuar; cualquier otra tecla vuelve atrás. | "continuar" for "go ahead". |
| `PowerShuttingDown` | Shutting down... | Apagando... | Keeps the three dots. |
| `PowerRestarting` | Restarting... | Reiniciando... |  |
| `PowerBusy` | It's already shutting down. | Ya se está apagando. |  |
| `PowerUnsupported` | This system can't do that from here. | Este sistema no puede hacer eso desde aquí. |  |
| `PowerFailed` | That didn't work. | No funcionó. |  |
| `PowerError` | (error {}) Press any key. | (error {}) Presiona cualquier tecla. | `(error -38)` stays; only the instruction is translated. |
| `ZoneRowNow` | Time zone: {} ({}) | Zona horaria: {} ({}) |  |
| `ClockKeepsLocal` | This computer's clock keeps local time (as Windows does). | El reloj de esta computadora guarda la hora local (como Windows). | "guarda la hora local" for "keeps local time". |
| `ClockKeepsUtc` | This computer's clock keeps UTC. | El reloj de esta computadora guarda la hora UTC. |  |
| `ClockLegacy1` | Saved. This system shows the computer's clock | Guardado. Este sistema muestra el reloj de la computadora | Two rows that read as one sentence; the split stays at the same place. |
| `ClockLegacy2` | as it reads, without a time zone. | tal como lo lee, sin zona horaria. |  |
| `ClockSetZone` | Set your zone: timezone America/New_York | Define tu zona: timezone America/New_York | The command stays. |
| `ClockChangeZone` | Change it: timezone Area/City | Cámbiala: timezone Area/City |  |
| `ClockRestart` | Restart to read this computer's clock this way. | Reinicia para leer así el reloj de esta computadora. |  |
| `ClockOffUtc` | Off by whole hours? Type: rtc utc | ¿Desfasada en horas exactas? Escribe: rtc utc | "¿Desfasada en horas exactas?" (la hora). `rtc utc` stays. |
| `ClockOffLocal` | Off by whole hours? Type: rtc local | ¿Desfasada en horas exactas? Escribe: rtc local | See ClockOffUtc. |
| `ClockTitle` | The clock | El reloj |  |
| `ClockZoneSet` | Time zone set to {}. | Zona horaria definida: {}. | "Zona horaria definida: {}." keeps the name last, so no gender agreement is needed. |
| `ClockNoZone` | There's no time zone called "{}". | No hay ninguna zona horaria llamada "{}". |  |
| `ClockTypeZone` | Type timezone Area/City, for example | Escribe timezone Area/City, por ejemplo |  |
| `ClockTypeZoneExamples` | timezone America/New_York or timezone Europe/London. | timezone America/New_York o timezone Europe/London. | Two commands; only "or" becomes "o". |
| `ClockDidYouMean` | Did you mean: | Quizás quisiste decir: | "Quizás quisiste decir:" avoids a question that the list below would have to close with "?". |
| `ClockSavedLocal` | Saved: this computer's clock keeps local time. | Guardado: el reloj de esta computadora guarda la hora local. |  |
| `ClockSavedUtc` | Saved: this computer's clock keeps UTC. | Guardado: el reloj de esta computadora guarda la hora UTC. |  |
