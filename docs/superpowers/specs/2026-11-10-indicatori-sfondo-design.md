# Indicatori sullo sfondo (v1.0.3) — design

## Intento

Su macOS il Dock sta al centro, in basso, e lascia libere le due zone ai suoi lati.
La 1.0.3 usa quelle zone per mostrare, **dentro lo sfondo animato**, quattro elementi:
**CPU**, **RAM**, **orologio** e **logo DevLogica**. L'utente vuole vederli "a fianco della
barra delle app", in modo discreto, senza aprire nessuna finestra.

Il rilascio comprende solo questi indicatori. L'aggiornamento automatico dell'app (scaricare e
installare) è stato discusso e **rimandato**: per ora resta il meccanismo attuale, cioè la voce di
menu che apre la pagina della release.

## Obiettivi

- Quattro elementi: CPU, RAM, orologio, logo. **Solo numeri**, nessun grafico.
- Ogni elemento si configura in tre modi: **nascosto**, **a sinistra** del Dock oppure **a destra** del Dock.
  Esempio richiesto dall'utente: RAM e orologio a sinistra, CPU a destra.
- Un interruttore generale per attivare e disattivare gli indicatori.
- Tre stili, a scelta dell'utente (gli sono piaciuti tutti e tre):
  - **A · Solo testo**: testo bianco semitrasparente direttamente sullo sfondo; il più discreto.
  - **B · Pillole come il Dock**: ogni elemento in un riquadro di vetro opaco con bordo chiaro e
    angoli arrotondati, simile al Dock.
  - **C · Orologio grande**: orologio grande e sottile; CPU e RAM piccoli sotto o accanto.
  Bozze di riferimento: `.superpowers/brainstorm/*/content/stile-indicatori-v2.html`.
- Gli indicatori compaiono **solo sullo schermo che ha il Dock**.

## Non obiettivi

- **Windows: niente.** Gli indicatori non vengono disegnati e le loro opzioni non compaiono
  nell'interfaccia (né nelle Impostazioni né nel menu).
- Grafici, cronologia, dettagli per core o per processo.
- Aggiornamento automatico dell'app.
- Cambiare il logo grande al centro dello sfondo: "Mostra il logo" resta com'è. Il logo
  nell'angolo è un elemento **separato**, con la sua impostazione.

## Anti-obiettivi (fallimento anche se "funziona")

- Indicatori che finiscono **sotto il Dock** o che lo toccano.
- Un consumo di CPU o di memoria che si nota: l'app vanta di essere leggerissima.
  Leggere le statistiche non deve aggiungere lavoro a ogni fotogramma.
- Testo sfocato o seghettato sugli schermi Retina.
- Sfondi personalizzati (`*.wgsl` degli utenti) che si rompono. Gli indicatori non devono
  richiedere modifiche agli shader esistenti né al contratto `scene()` / `logo_layout()`.

## Vincoli

- App Rust esistente: winit 0.30, wgpu 30 (backend Metal su macOS), egui 0.36 con egui-wgpu
  (già usato per le finestre in `src/ui.rs`).
- Sfondi: una finestra per schermo al livello desktop (`src/wallpaper.rs`, `src/platform/macos.rs`);
  lo shader disegna un triangolo a schermo intero (`src/render.rs`). Il disegno avviene a una
  risoluzione eventualmente ridotta (`Quality`, `Target::scale`).
- Configurazione in `src/config.rs` (`Config`, `#[serde(default)]`): i campi nuovi devono avere
  default sensati, così un `config.json` vecchio continua a caricarsi.
- La versione viene presa dal tag (`release.yml`). La release si pubblica con il tag `v1.0.3`;
  aggiornare anche `Cargo.toml` a `1.0.3` per coerenza.
- Testi dell'interfaccia e commenti in italiano, come nel resto del codice.

## Decisioni prese con l'utente

- Solo macOS; su Windows nessun indicatore e nessuna opzione visibile.
- Elementi: CPU, RAM, orologio, logo; solo numeri.
- Posizione per elemento: nascosto / sinistra / destra.
- Tre stili selezionabili (A, B, C come sopra).
- Solo sullo schermo con il Dock.
- Aggiornamento automatico fuori dalla 1.0.3.

## Decisioni mie (proposte all'utente, da confermare leggendo)

- **Dati mostrati**: CPU come percentuale complessiva di tutti i core (es. `CPU 23%`).
  RAM come usata / totale in GB con una cifra decimale e virgola italiana (es. `RAM 11,2 / 16 GB`).
  Orologio `HH:MM`, 24 ore, ora locale.
- **Frequenza**: CPU e RAM letti ogni ~2 secondi, in un thread a parte o con un timer che non
  dipende dai fotogrammi; l'orologio si aggiorna al cambio del minuto. A ogni fotogramma il
  rendering usa solo i valori già letti.
- **Dove si configura**: nuova sezione "Indicatori sullo sfondo" nella finestra **Impostazioni**
  (le opzioni sono troppe per un menu). Più, nel sottomenu *Sfondi animati* del menu dell'icona,
  la voce spuntabile **"Mostra gli indicatori"**, che equivale all'interruttore generale.
  Su Windows né la sezione né la voce di menu compaiono.
- **Legame con gli sfondi**: gli indicatori fanno parte dello sfondo. Se gli sfondi animati sono
  disattivati, o in pausa, non si vedono. In pausa restano fermi insieme allo sfondo.
  La sezione delle Impostazioni lo dice in una riga.
- **Ordine dentro un lato** (fisso, non configurabile): dall'esterno verso il Dock
  logo → orologio → RAM → CPU. A sinistra si legge quindi `logo  14:32  RAM…  CPU…`;
  a destra lo stesso ordine allo specchio, cioè con il logo sul bordo destro dello schermo.
- **Default per chi aggiorna**: indicatori **attivi** con stile **A**, logo e orologio a sinistra,
  RAM e CPU a destra. È una novità della release, quindi deve vedersi, e si disattiva con un clic.
- **Animazione di apertura**: gli indicatori compaiono con una dissolvenza, insieme al logo grande
  (stessa temporizzazione di `since`), così non spuntano prima dell'animazione dello sfondo.
- **Margini**: gli elementi stanno in basso, allineati alla fascia del Dock, con un margine di
  circa 16 pt dal bordo dello schermo. Se lo spazio tra il bordo e il Dock non basta, gli
  elementi che non ci stanno vengono omessi, partendo da quello più vicino al Dock, invece di
  sovrapporsi.

## Posizione e larghezza del Dock (decisione dell'utente: permesso Accessibilità)

La larghezza del Dock cambia da sola: dipende dalle app fisse, da quelle aperte, dalla dimensione
delle icone e dall'ingrandimento. L'utente ha scelto di **misurarla esattamente** chiedendola al
Dock tramite le API di **Accessibilità**, accettando il permesso relativo.

- **Quando chiedere il permesso** (mia decisione): solo su macOS e solo se gli indicatori sono
  attivi, con `AXIsProcessTrustedWithOptions` e il prompt di sistema, al massimo una volta per
  avvio. Se gli indicatori sono disattivati, il permesso non si chiede mai.
- **Senza permesso** (mia decisione): gli indicatori si vedono lo stesso. Al centro si lascia
  libera una fascia fissa e abbondante, larga quanto un Dock pieno, e si applica la regola di
  omissione. La sezione delle Impostazioni mostra lo stato del permesso ("Posizione esatta del
  Dock: consentita / non consentita") e un pulsante che apre *Privacy e sicurezza →
  Accessibilità*. Appena il permesso arriva, l'app passa alla misura esatta senza riavvio.
- **Frequenza**: la misura del Dock si aggiorna insieme a CPU e RAM (~2 s) oppure tramite
  notifiche AX. Non si fa mai a ogni fotogramma. Va bene un ritardo di un paio di secondi dopo
  l'apertura di un'app.
- **Quale schermo**: quello che contiene il rettangolo del Dock restituito da AX. Senza permesso,
  o con il Dock nascosto automaticamente, si usa il confronto `frame` / `visibleFrame` e, se non
  basta, lo schermo principale. Con il Dock **nascosto automaticamente** gli indicatori restano in
  basso e si usa la fascia fissa, perché il Dock compare sopra di loro solo al passaggio del mouse.
- **Dock a sinistra o a destra** dello schermo: gli indicatori restano negli angoli in basso dello
  schermo che ha il Dock, senza fascia centrale da evitare.
- **Avvertenza firma** (da verificare in implementazione): l'app non è firmata con un certificato
  sviluppatore. macOS lega il permesso Accessibilità alla firma dell'eseguibile, quindi è
  probabile che dopo **ogni aggiornamento** il permesso vada ridato, togliendo e rimettendo l'app
  dall'elenco. Va documentato nel README. Il fallback senza permesso garantisce che nel frattempo
  non si rompa niente.

## Stile C nei casi misti (mia decisione)

- L'orologio grande "ospita" sotto di sé CPU e RAM solo se sono **sullo stesso lato**. Le due righe
  piccole vanno impilate sotto l'orologio, e l'insieme conta come un unico blocco per la regola di
  omissione.
- CPU o RAM da soli su un lato senza orologio: testo piccolo, come nello stile A.
- Orologio nascosto: lo stile C si comporta come lo stile A.
- Il logo nello stile C resta accanto al blocco, come nelle altre varianti.

## Logo nell'angolo (mia decisione)

Si usa `assets/logo.png`, lo stesso logo dello sfondo, rimpicciolito. La scritta "DEVLOGICA"
nelle bozze era solo un segnaposto. Nello stile B il logo sta nella sua pillola.

## Lasciato al builder

- **Come disegnare il testo e il logo nell'angolo.** Un'opzione è un secondo passaggio egui sulla
  superficie dello sfondo, dopo lo shader (egui-wgpu è già presente e gestisce testo e immagini
  con antialiasing). In alternativa si può preparare una piccola texture. Unico vincolo: gli
  shader degli sfondi, compresi quelli personalizzati, restano invariati. Attenzione a
  `Target::scale` < 1: il testo deve restare nitido o almeno ben leggibile anche in qualità
  *Bassa*, eventualmente disegnando gli indicatori a risoluzione piena.
- **Come leggere CPU e RAM su macOS**: API Mach (`host_statistics64`, `host_processor_info`)
  o il crate `sysinfo`, preferendo ciò che pesa meno sull'eseguibile e in memoria. Per "RAM usata"
  conviene una definizione vicina a quella di Monitoraggio Attività (memoria app + wired + compressa).
- **Dettagli dell'implementazione Accessibilità** (vedi sezione *Posizione e larghezza del Dock*):
  quale elemento AX interrogare (tipicamente l'`AXList` figlio dell'app Dock, con `AXPosition` /
  `AXSize`), conversione delle coordinate in pixel della finestra dello sfondo, polling o
  `AXObserver`.
- **Ora locale** senza aggiungere dipendenze pesanti (per esempio `localtime_r` via libc).
- Dimensioni esatte di font, opacità e raggi degli stili, partendo dalle bozze.
- Test: logica pura (formattazione dei valori, assegnazione di sinistra/destra e ordine,
  omissione per mancanza di spazio, default e compatibilità del `config.json` vecchio) con test
  unitari, come il resto del progetto. Aggiornare l'anteprima `--ui impostazioni` se serve.
- Aggiornare il README (sezione *Sfondi animati*) e aggiungere `.superpowers/` al `.gitignore`.
