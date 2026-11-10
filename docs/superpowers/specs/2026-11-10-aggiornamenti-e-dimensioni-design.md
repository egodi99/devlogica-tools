# Aggiornamenti dall'app e dimensione degli indicatori (v1.0.4) — design

## Intento

Due migliorie indipendenti per la prossima versione.

1. **Aggiornamenti dall'app.** Oggi l'app trova le versioni nuove, ma le segnala solo con una voce
   nel menu che apre la pagina della release: poi bisogna scaricare il file e sostituirlo a mano.
   Molti colleghi non la notano, e chi la nota deve fare a mano un lavoro noioso. L'utente vuole che
   **con un clic l'app si scarichi, si sostituisca e si riavvii da sola**. Vuole anche poter
   **controllare a mano**, e che la novità si faccia notare con una **notifica di sistema**.
2. **Dimensione degli indicatori** (macOS, introdotti nella 1.0.3). Si vogliono poter ingrandire o
   rimpicciolire **i testi e il logo accanto al Dock, separatamente**.

Le due parti non dipendono l'una dall'altra e si possono implementare e verificare in qualunque
ordine. La release le contiene entrambe.

## Parte 1 — Aggiornamenti

### Obiettivi

- **Controllo automatico** come oggi: all'avvio e poi due volte al giorno (`src/update.rs`, `app.rs`).
- **Controllo manuale**: nel menu dell'icona c'è sempre la voce **"Cerca aggiornamenti"**.
  - Se c'è una versione nuova si comporta come il controllo automatico (vedi sotto).
  - Se non c'è, l'utente lo deve sapere: "Hai già l'ultima versione (1.0.4)". Va bene la
    notifica di sistema oppure il testo della voce di menu per qualche secondo; vedi *Decisioni mie*.
  - Durante il controllo la voce diventa "Controllo in corso…" e non è cliccabile.
- **Notifica di sistema** quando viene trovata una versione nuova, **una sola volta per versione**:
  non a ogni controllo, e non di nuovo dopo un riavvio per la stessa versione. Testo:
  titolo "DevLogica Tools", corpo "È disponibile la versione vX.Y.Z. Clicca per aggiornare."
  **Cliccando la notifica parte subito l'aggiornamento.**
- **Voce di menu "Aggiorna alla vX.Y.Z"**, al posto dell'attuale "Scarica la nuova versione".
  Con un clic: scarica il file giusto della release, sostituisce l'app, la chiude e la riavvia.
  - macOS: `DevLogica-Tools-macOS.zip`, che contiene `DevLogica Tools.app` (universale).
  - Windows: `DevLogica-Tools-Windows.exe`.
  - Mentre lavora, la voce mostra lo stato ("Download in corso…", poi "Installazione…") e non è
    cliccabile. Un secondo clic, o un clic sulla notifica mentre l'aggiornamento è già in corso,
    non fa partire un secondo aggiornamento.
- **In caso di errore** (rete, file mancante nella release, cartella non scrivibile, file corrotto)
  l'app **resta esattamente com'è e continua a funzionare**. La voce torna "Aggiorna alla vX.Y.Z" per
  riprovare. Accanto compare "Apri la pagina della release" (il comportamento di oggi) e il motivo va
  nel log. Una notifica dice "Aggiornamento non riuscito: <motivo breve>".
- Nella finestra **Impostazioni**, dove oggi c'è il link "È disponibile la versione … scaricala",
  compaiono gli stessi comandi: "Aggiorna alla vX.Y.Z" e "Cerca aggiornamenti", con lo stesso stato.

### Non obiettivi

- Aggiornamento silenzioso senza clic.
- Canali beta, ritorno a versioni precedenti, aggiornamenti differenziali.
- Firma del codice o notarizzazione Apple (l'app resta firmata ad-hoc).

### Anti-obiettivi (fallimento anche se "funziona")

- Un aggiornamento che si interrompe a metà e lascia un'app che non parte più, o nessuna app.
  La sostituzione deve avvenire solo con il nuovo file **completo e verificato**, e mai
  sovrascrivendo direttamente la copia in uso.
- Due copie dell'app aperte insieme dopo il riavvio, oppure nessuna (l'app vecchia si chiude e
  quella nuova non parte). Il blocco a istanza unica (porta 47824, `main.rs`) va rispettato: la nuova
  copia parte solo dopo che la vecchia ha liberato la porta, oppure riprova finché non la trova libera.
- Su macOS, l'app nuova bloccata da Gatekeeper ("non può essere aperta") dopo l'aggiornamento.
- Su Windows, una finestra di console o un avviso SmartScreen che compare a ogni aggiornamento.
- Notifiche ripetute per la stessa versione.

### Vincoli

- Scaricamento con `curl`, già usato per il controllo e presente su macOS e su Windows 10/11:
  nessuna libreria HTTPS dentro l'app. Su Windows `CREATE_NO_WINDOW`, come in `update.rs`.
- Va sempre controllato l'URL dell'asset, preso dalla risposta di
  `api.github.com/repos/<repo>/releases/latest` (`assets[].browser_download_url`).
- La build da GitHub Actions e il repository (`update::repo()`) restano come oggi. Nelle build senza
  repository configurato i comandi di aggiornamento non compaiono.
- Su Windows l'exe in esecuzione non si può sovrascrivere, ma si può rinominare: la sequenza
  "scarica accanto, rinomina il vecchio, metti il nuovo, riavvia, poi cancella il vecchio" va bene.
- Su macOS l'app si sostituisce dove si trova (di solito `/Applications/DevLogica Tools.app`).
  Se la cartella non è scrivibile vale il caso di errore.
- **Permesso Accessibilità** (indicatori della 1.0.3): con la firma ad-hoc l'identità dell'app
  cambia a ogni versione, quindi è atteso che macOS chieda di nuovo il permesso dopo ogni
  aggiornamento. Non è un problema da risolvere in questa versione. Il README lo deve dire chiaramente.

### Decisioni mie (da confermare leggendo)

- L'aggiornamento parte anche se l'utente è in call: l'ha chiesto lui, e il riavvio dura pochi
  secondi. La call riprende da sola perché il rilevamento del microfono riparte. È accettato che
  i colleghi vedano per un attimo la call chiusa e che la durata riparta da zero: non serve
  conservare lo stato della call attraverso il riavvio.
- Il controllo manuale mostra **sempre** la notifica quando trova una versione nuova, anche se
  quella versione era già stata notificata. La regola "una sola volta per versione" vale solo per
  il controllo automatico, che è l'unico ad aggiornare `notified_version`.
- "Hai già l'ultima versione" si mostra con una notifica di sistema, solo quando il controllo è
  stato chiesto a mano. Il controllo automatico non dice mai niente se non c'è una versione nuova.
- La versione già notificata si salva in `config.json` (es. `notified_version`), così dopo un
  riavvio non si notifica di nuovo.
- Dopo un aggiornamento riuscito, al primo avvio della versione nuova il log registra
  "Aggiornato da X a Y". Nessuna notifica in più.
- Pulizia: i file temporanei (zip scaricato, cartella estratta, vecchio exe rinominato) vengono
  cancellati dalla versione nuova al suo primo avvio, o al più tardi al successivo.

### Lasciato al builder

- **Come mostrare la notifica e ricevere il clic**, su entrambi i sistemi:
  - macOS: per esempio `UNUserNotificationCenter` con un delegate. Va verificato che funzioni con
    un'app firmata ad-hoc e con `ActivationPolicy::Accessory`; al primo uso macOS chiede il
    permesso per le notifiche.
  - Windows: per esempio la notifica "balloon" dell'icona di sistema
    (`Shell_NotifyIcon` con `NIF_INFO` e `NIN_BALLOONUSERCLICK`), oppure una notifica toast.
  Se su un sistema il clic non si può intercettare in modo affidabile, la notifica deve comunque
  comparire, con il testo "apri il menu per aggiornare" invece di "clicca per aggiornare".
  Questa scelta va scritta nel piano.
- **Come sostituire e riavviare** in modo sicuro: per esempio un piccolo script (`/bin/sh` su macOS,
  `cmd`/PowerShell nascosto su Windows) lanciato staccato, che aspetta la chiusura del processo,
  sposta i file e riapre l'app. Oppure la stessa app nuova avviata con un argomento tipo
  `--completa-aggiornamento`. Vincoli: gli anti-obiettivi qui sopra.
- **Verifica del file scaricato**: almeno dimensione uguale a quella dichiarata dall'API (`size`
  dell'asset) e, su macOS, firma valida (`codesign --verify`) e `Info.plist` con la versione attesa.
- Rimozione della quarantena su macOS (`xattr -dr com.apple.quarantine`), se `curl` la imposta (di
  norma no).
- Struttura del codice: per esempio `src/update.rs` per il controllo e la scelta dell'asset, con i
  test sulla risposta JSON di GitHub, e un modulo `src/install.rs` per scaricamento e sostituzione.

## Parte 2 — Dimensione degli indicatori (solo macOS)

### Obiettivi

- Due regolazioni separate nella sezione **"Indicatori sullo sfondo"** delle Impostazioni:
  - **"Dimensione dei testi"**: CPU, RAM e orologio, in tutti e tre gli stili;
  - **"Dimensione del logo"**.
- Ogni regolazione è un cursore dal **75% al 200%**, con il valore predefinito al **100%**, cioè le
  misure della 1.0.3. I passi sono da 5% e la percentuale si vede accanto al cursore.
- La modifica si vede **subito** sullo sfondo, senza riavvio.
- Testi più grandi ingrandiscono in proporzione anche le pillole (stile B) e il blocco dell'orologio
  grande (stile C), comprese le spaziature interne.

### Vincoli

- Valgono tutte le regole della 1.0.3: ordine sui due lati, omissione intera degli elementi che non
  ci stanno, il blocco non esce mai dallo schermo, solo sullo schermo con il Dock.
  Con testi grandi è normale che qualche elemento venga omesso.
- Il `config.json` della 1.0.3 si carica senza problemi (default 100%).

### Decisioni mie

- I margini verso il bordo e verso il Dock (16 pt, 12 pt) restano fissi.
- Nessuna regolazione dal menu dell'icona: solo dalle Impostazioni.
- Un pulsante "Ripristina" riporta entrambe le dimensioni al 100%.

## Rilascio

- Versione **1.0.4** (mia decisione, segue la numerazione della 1.0.3).
- README: aggiornare la sezione sugli aggiornamenti ("Pubblicare una nuova versione" e
  "Installazione"), la sezione sugli indicatori e la nota sul permesso Accessibilità.
- Il primo aggiornamento a un clic sarà quello **dalla 1.0.4 alla versione successiva**. Chi ha la
  1.0.3 deve installare la 1.0.4 a mano un'ultima volta. Per provare davvero la procedura prima di
  distribuirla serve una release di prova (vedi la nota per il builder qui sotto).

### Nota per il builder sulla verifica

La procedura di aggiornamento va provata da una build a una build più recente, su macOS e su
Windows, prima della release vera. Un modo: una variabile d'ambiente solo di sviluppo che finge una
versione corrente più vecchia (es. `DEVLOGICA_FAKE_VERSION=1.0.0`) contro la release 1.0.3 già
pubblicata. Così si prova lo scaricamento, la sostituzione e il riavvio senza pubblicare niente.
