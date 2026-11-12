<!-- SPDX-License-Identifier: MPL-2.0 -->
# i18n-core: translation notes (Spanish)

The Spanish table in `src/lib.rs`, row by row, for the wording review. Generated from the table, so the
two can't disagree; the KATs in `src/tests.rs` hold the English column to the pinned `fixtures/en.txt` and every row inside
the card.

## Choices that apply to every row

- **Register.** Neutral Latin American Spanish, informal *tú* ("Escribe", "Puedes"), matching the English's
  short imperatives. "computadora", not "ordenador"; "presiona", not "pulsa".
- **Commands stay as typed.** `help`, `list /`, `timezone Area/City`, `rtc utc`, `rtc local` and the power
  verbs are what the parsers read. They appear unchanged inside the Spanish rows.
- **Names stay.** LogansOS, Arthur (and its English acronym), Lens, UTC, USB, CPU, Windows, Enter, Esc, Tab.
- **Persona rules hold.** No exclamations (so no "¡" either), no emoji, economy.
- **Characters.** Beyond ASCII the table uses only `¿ á é í ñ ó ú` (F1a adds none), all in Inter Regular (KAT). The intro
  persona KAT in `neo-atrium` allows printable ASCII only; S6b relaxes it to "covered by the face" for the
  Spanish table.
- **Width.** Every row fits the card with its widest realistic arguments (a 30-byte zone name, a 32-byte
  typed name, the longest date; for F1a, a size, the limits and an errno at their widest). The widest Spanish row is IntroModelAbsent at 96.7% of the card.

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
| `SetupKeyboardBodyChoose` | Choose the layout printed on your keys. | Elige la distribución impresa en tus teclas. | S5's Keyboard page (`keyboard-layouts`). "distribución" for layout, as in SetupBodyKeyboard; "impresa en tus teclas" for "printed on your keys". |
| `SetupKeyboardTryIt` | Try it: {}_ | Pruébalo: {}_ | The test field: what was typed, then the `_` cursor. Measured with 24 `W`s (`FIELD_MAX`): 65% of the card. |
| `SetupSummaryKeyboard` | Keyboard: {} | Teclado: {} | The Try summary's row; the argument is a layout's name in the same language. |
| `LayoutEnglishUs` | English (US) | Inglés (EE. UU.) | The layout's name, as the option row and the summary show it. "EE. UU." with the RAE spacing, as in SetupBodyKeyboard. |
| `LayoutSpanishSpain` | Spanish (Spain) | Español (España) | The Spain layout (J.c.: US and Spanish keyboards). |
| `SetupBodyTimeZoneNoSearch` | Where are you? At Home: timezone Area/City. | ¿Dónde estás? En Inicio: timezone Area/City. | **Home** is "Inicio" everywhere (the Home view). `timezone Area/City` is the command and stays as typed. |
| `SetupBodyZoneSet` | Time zone: {}. The clock is set. | Zona horaria: {}. El reloj está en hora. | "The clock is set" means set to the right time: "El reloj está en hora". |
| `SetupBodyZoneLater` | Change it later at Home: timezone Area/City. | Cámbiala después en Inicio: timezone Area/City. | "Cámbiala" (la zona). The command stays. |
| `SetupBodySearchEmpty` | Type a city to search, or pick one. | Escribe una ciudad para buscar, o elige una. |  |
| `SetupBodySearchNone` | Search: {}  (no zone matches) | Buscar: {}  (ninguna zona coincide) | Keeps the two spaces before the bracket. |
| `SetupBodySearch` | Search: {} | Buscar: {} |  |
| `SetupBodyComfort` | Make the screen calmer, if you like. | Haz que la pantalla sea más tranquila, si quieres. | Chad's wording (relay 384). Informal *tú*, as everywhere in this table. |
| `SetupNotSet` | not set | sin definir | "sin definir" (not "no configurado"). |
| `ZoneRow` | Time zone: {} | Zona horaria: {} |  |
| `SetupSummaryZoneClockNotSet` | Time zone: {} (clock not set) | Zona horaria: {} (reloj sin ajustar) | "reloj sin ajustar". |
| `ZoneRowNotSet` | Time zone: not set | Zona horaria: sin definir |  |
| `SetupSummaryComfort` | Reduce motion: {}. Reduce transparency: {}. | Reducir movimiento: {}. Reducir transparencia: {}. |  |
| `On` | on | sí | "sí"/"no" for the summary's on/off. "activado/desactivado" is longer and gendered. |
| `Off` | off | no | See On. |
| `SetupNoticeZoneInUse` | Couldn't save the time zone. In use: {}. | No se pudo guardar la zona horaria. En uso: {}. |  |
| `SetupNoticeZoneRestart` | Couldn't save the time zone. Restart to check it. | No se pudo guardar la zona horaria. Reinicia para comprobarla. | "comprobarla" (la zona). |
| `SetupBodyLanguageChoose` | Choose the language for setup and Home. | Elige el idioma de la configuración y de Inicio. | S6b. "Inicio" for Home, as elsewhere. |
| `SetupBodyLanguageArthur` | Arthur answers in English for now. | Por ahora, Arthur responde en inglés. | S6b. Arthur's replies stay in English (the dispatch's scope), so the page says so. |
| `SetupTitleNetwork` | Network | Red | S4's page (S6b moved its strings in). |
| `SetupContinueNoNetwork` | Continue without a network | Continuar sin red |  |
| `NetUnavailable` | Network status is unavailable. | El estado de la red no está disponible. |  |
| `NetNoLink` | Wired: no link. | Cable: sin enlace. | "Cable" for "Wired" (a wired connection); "enlace" is the link layer's term. |
| `NetDevice` | Device: {}. | Dispositivo: {}. | The argument is a driver name (12 bytes at most). |
| `NetConnected` | Wired: connected, {} | Cable: conectado, {} | The argument is the dotted IPv4 address. |
| `NetNoAddress` | Wired: connected, no address yet. | Cable: conectado, aún sin dirección. |  |
| `NetDeviceLease` | Device: {}. Lease: {}. | Dispositivo: {}. Concesión: {}. | "Concesión" is the DHCP lease's usual Spanish term. Widest Spanish S6b row: 70% of the card. |
| `NetWifiLater` | Wi-Fi: coming in a later version. | Wi-Fi: llegará en una versión posterior. |  |
| `LeaseUnderMinute` | under a minute left | queda menos de un minuto | Fills NetDeviceLease's second argument. |
| `LeaseMinutes` | {} min left | quedan {} min | Always plural in Spanish: the minutes are 1 to 59, and "quedan 1 min" reads as an abbreviation, as in English. |
| `LeaseHours` | {} h {} min left | quedan {} h {} min |  |
| `IntroGreeting` | Good day. I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests. | Buen día. Soy Arthur: Assistant for Real-time Tasks, Home automation and User Requests. | **Decision: the acronym's expansion stays in English, as the name it spells.** "ARTHUR" is an English acronym; translating the expansion (Chad's alternative, "Asistente para tareas en tiempo real, automatización del hogar y solicitudes de usuario") no longer spells it. It also doesn't fit: measured with the card's face and size, that row is 114% of the card with "Buen día." and 104% without it, and a Spanish gloss next to the English would be longer still. A gloss on a row of its own would change the intro's row count, which is S6b's and the persona's call. **J.c. decides**; the catalog takes either answer. |
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
| `HelpChips` | The chips on the left switch between me and the Lens; Tab reaches them. | Los botones de la izquierda alternan entre Lens y yo; puedes acceder a ellos con Tab. | Chad's wording for "Tab reaches them" (relay 384). "chips" become "botones" (the Spine chips are buttons to the user); "entre Lens y yo" puts Arthur last. 90% of the card. |
| `HelpDrag` | Drag a window by its title bar to move it, or by an edge to resize it. | Arrastra una ventana por su barra de título para moverla, o por un borde para ajustarla. | "ajustarla" for "resize it", shorter than "cambiar su tamaño" (this row is 91% of the card). |
| `HelpType` | Start typing to ask me something, or press Esc to close this card. | Empieza a escribir para preguntarme algo, o presiona Esc para cerrar esta tarjeta. | "presiona" (Latin American) rather than "pulsa" (Spain). |
| `IntroHint` | Type help, or click here, for what you can do yourself. | Escribe help, o haz clic aquí, para ver lo que puedes hacer tú. | `help` stays. |
| `IntroModelAbsent` | I can hear you, but my model isn't on this system yet. Boot the LogansOS USB it came with. | Te oigo, pero aún no tengo mi modelo. Arranca el USB de LogansOS que vino con el equipo. | Keeps "it came with" ("que vino con el equipo"; Chad, relay 384). His full line ("…que venía con esta computadora", keeping "en este sistema") measures 127% of the card, so "my model isn't on this system yet" becomes "aún no tengo mi modelo" and "the computer" becomes "el equipo". "Te oigo" is the literal "I can hear you". 96.7% of the card. |
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
| `DateLong` | {0}, {1} {2}, {3} | {0} {2} de {1} de {3} | S6b. The arguments by index: weekday, month, day, year, so Spanish puts the day first: "miércoles 21 de octubre de 2026". Weekday and month stay lower case, as Spanish writes them. |
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
| `ClockReadoutZoneNotSet` |  (zone not set) |  (zona sin definir) | S6b. Home's clock readout, after the time (with its leading space). S6a missed it; the no-stray-literals scan found it. "sin definir" as in SetupNotSet. |
| `FilesTitle` | Files | Archivos | F1a. The app's name: "Archivos", as the window and the Spine chip say it. |
| `FilesKindFolder` | Folder | Carpeta | F1a. The kind column. |
| `FilesKindFile` | File | Archivo | F1a. |
| `FilesKindModel` | Model | Modelo | F1a. |
| `FilesKindLink` | Link | Enlace | F1a. A symbolic link (NeoFS lists them but won't read them): "Enlace". |
| `FilesSizeBytes` | {} B | {} B | F1a. Units stay as symbols (B, KB, MB) in both languages. |
| `FilesSizeKb` | {} KB | {} KB | F1a. |
| `FilesSizeMb` | {} MB | {} MB | F1a. |
| `FilesDecimalMark` | . | , | F1a. The decimal mark in a size: a comma in Spanish ("1,5 KB"). |
| `FilesItemsOne` | 1 item | 1 elemento | F1a. A folder's size column. Two rows, since the catalog has no plurals; "0 elementos" uses the plural, as Spanish does. |
| `FilesItemsMany` | {} items | {} elementos | F1a. |
| `FilesEmpty` | This folder is empty. | Esta carpeta está vacía. | F1a. |
| `FilesFolderFullNote` | This folder holds {} items, the most it can. | Esta carpeta tiene {} elementos, el máximo posible. | F1a. |
| `FilesNewFolderName` | New folder | Carpeta nueva | F1a. The new folder's name; later ones are "Carpeta nueva 2", "Carpeta nueva 3". Adjective after the noun, the usual order. |
| `FilesHintBrowse` | Enter opens. Backspace goes up. N makes a folder. F2 renames. | Enter abre. Retroceso sube. N crea una carpeta. F2 cambia el nombre. | F1a. "Retroceso" is the Backspace key's name; Enter, N and F2 stay. Present tense ("abre", "sube") for what each key does. |
| `FilesHintBrowse2` | Del deletes. C copies. V pastes. | Supr elimina. C copia. V pega. | F1a. "Supr" is the Delete key's label on Spanish keyboards; C and V stay. |
| `FilesRenameTitle` | Rename | Cambiar nombre | F1a. "Cambiar nombre" (not "Renombrar"), the usual menu wording. |
| `FilesHintNaming` | Type a name. Enter to save, Esc to cancel. | Escribe un nombre. Enter para guardar, Esc para cancelar. | F1a. |
| `FilesConfirmDeleteFile` | Delete this file? This can't be undone. | ¿Eliminar este archivo? No se puede deshacer. | F1a. "Eliminar" for delete throughout. No trash in the MVP, so it says it can't be undone. |
| `FilesConfirmDeleteFolder` | Delete this empty folder? | ¿Eliminar esta carpeta vacía? | F1a. |
| `FilesConfirmHint` | Enter to delete; any other key goes back. | Enter para eliminar; cualquier otra tecla vuelve atrás. | F1a. As PowerConfirmHint. |
| `FilesNameEmpty` | A name can't be empty. | El nombre no puede estar vacío. | F1a. |
| `FilesNameTooLong` | A name can be at most {} bytes long. | Un nombre puede tener como máximo {} bytes. | F1a. The limit is in bytes (47), as NeoFS counts it; a name in accented letters reaches it sooner. |
| `FilesNameSlash` | A name can't contain /. | Un nombre no puede contener /. | F1a. |
| `FilesNameControl` | A name can't contain control characters. | Un nombre no puede contener caracteres de control. | F1a. |
| `FilesNameDot` | A name can't be . or .. | Un nombre no puede ser . ni .. | F1a. Ends without a period after "..", which is the name itself. |
| `FilesErrStale` | That folder moved or was removed. This is the nearest one above it. | Esa carpeta se movió o se eliminó. Esta es la más cercana por encima. | F1a. Shown after Files falls back to an ancestor (Muse's path-binding case). "la más cercana por encima": the nearest folder above. |
| `FilesErrExists` | Something here already has that name. | Ya hay algo aquí con ese nombre. | F1a. |
| `FilesErrNotEmpty` | That folder isn't empty. Empty it first. | Esa carpeta no está vacía. Vacíala primero. | F1a. "Vacíala primero": empty it (the folder) first. |
| `FilesErrNoSpace` | There isn't enough space on the disk. | No hay espacio suficiente en el disco. | F1a. |
| `FilesErrRefused` | That isn't allowed here. | Eso no está permitido aquí. | F1a. |
| `FilesErrFolderFull` | This folder is full: {} items at most. | Esta carpeta está llena: {} elementos como máximo. | F1a. |
| `FilesErrNotFound` | That item is gone; the list is up to date now. | Ese elemento ya no está; la lista ya está al día. | F1a. |
| `FilesErrBusy` | The system needs that item, so it stays. | El sistema necesita ese elemento, así que se queda. | F1a. |
| `FilesErrRetry` | The disk was busy. Try again. | El disco estaba ocupado. Inténtalo de nuevo. | F1a. |
| `FilesErrIo` | The disk couldn't do that. | El disco no pudo hacerlo. | F1a. |
| `FilesErrInvalid` | That can't be done here; the disk may be full. | Eso no se puede hacer aquí; puede que el disco esté lleno. | F1a. F0 reports no space, a folder too deep and a bad move with one error; the message names the likely one. |
| `FilesErrOther` | That didn't work (error {}). | No funcionó (error {}). | F1a. `(error -38)` stays; only the words are translated. |
| `FilesRootUnavailable` | Files can't read its folders right now. | Archivos no puede leer sus carpetas ahora. | F1a. |
| `FilesCopied` | Copied. Open a folder and paste it there. | Copiado. Abre una carpeta y pégalo ahí. | F1a. |
| `FilesCopyTooBig` | Only files up to {} KB copy for now. Nothing was copied. | Por ahora solo se copian archivos de hasta {} KB. No se copió nada. | F1a. The MVP's 32 KB copy limit. |
| `FilesCopyNotFile` | Only files copy for now, not folders. | Por ahora solo se copian archivos, no carpetas. | F1a. |
| `FilesPasted` | Pasted. | Pegado. | F1a. |
| `FilesNothingToPaste` | Copy a file first. | Copia un archivo primero. | F1a. |
| `FilesNoApp` | No app opens this. | Ninguna app abre esto. | F1a. "app" stays, as in PowerWarning. |
| `FilesModelArthur` | Models are Arthur's; no app opens them. | Los modelos son de Arthur; ninguna app los abre. | F1a. Models are Arthur's (`.neom`). |
