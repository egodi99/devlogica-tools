# Statistiche delle ore di call — design

## Intento

L'utente vuole sapere **quanto tempo passa in call** e vedere **come cambia nel tempo**: report
giornalieri, settimanali, mensili e annuali, con un grafico che mostri l'andamento.
Il riconoscimento delle call esiste già (`src/detect.rs`, `Office::start_my_call` / `end_my_call`).
Manca solo che le call vengano registrate e mostrate.

I dati sono **personali**: restano sul PC e non vengono mai inviati in rete ai colleghi.

Tra le bozze mostrate l'utente ha scelto la **A · Schede per periodo**: si sceglie
Giorno / Settimana / Mese / Anno, ci si sposta avanti e indietro, e si vedono un grafico a barre
del periodo e la suddivisione per app.

## Obiettivi

### Registrazione

- Si registrano **tutte le call dell'utente**: quelle riconosciute in automatico dal microfono e
  quelle avviate a mano dal menu. Per ognuna: inizio, fine, app (se nota, altrimenti "Manuale"),
  se era automatica.
- La call viene **scritta su disco appena inizia** e la sua fine viene **aggiornata a ogni
  CALL_ALIVE** (ogni 20 secondi, `next_alive` in `app.rs`) e alla chiusura. Se il PC si spegne o
  l'app si chiude di colpo, si perdono al massimo circa 20 secondi.
- **Il PC in sospensione non conta.** Se tra due salvataggi passano più di circa 60 secondi
  (il PC dormiva, l'app era bloccata), la call registrata si chiude all'ultimo salvataggio. Se al
  risveglio la call è ancora in corso, le statistiche ripartono con un nuovo tratto, senza contare
  il buco. In rete la call resta una sola: il tratto riguarda solo le statistiche.
- All'avvio, una call rimasta "aperta" da una sessione precedente (l'app è stata chiusa senza
  chiuderla) si considera finita all'ultimo aggiornamento salvato.
- Il cambio nome (`Office::rename`) ri-annuncia la call in rete con un id nuovo, ma **per le
  statistiche è la stessa call**: non deve diventare due call né lasciare un buco.
- I dati si conservano **per sempre**. Il volume è minimo: qualche migliaio di call l'anno.

### Calcolo

- Tutto si calcola nel **fuso orario locale** del PC.
- La **settimana va da lunedì a domenica**.
- Una call **a cavallo della mezzanotte** si divide sui due giorni, e quindi anche su due
  settimane, mesi o anni quando serve.
- Le durate si mostrano come "2 h 10 min", "45 min", oppure "< 1 min".
- **Media per giorno lavorativo** = totale del periodo ÷ numero di giorni lunedì–venerdì del
  periodo, contando per il periodo in corso solo i giorni fino a oggi compreso. I minuti fatti di
  sabato e domenica contano nel totale ma non aumentano il numero di giorni. Nella scheda Giorno
  la media non c'è.

### Finestra "Le mie call"

- Una **finestra a sé**, normale (non sovrapposta come gli avvisi), simile alle Impostazioni.
  Si apre:
  - dal menu dell'icona, con una voce **"Le mie call…"** vicino a "Impostazioni…";
  - dalle Impostazioni, con un pulsante "Le mie call…".
  Se è già aperta, viene portata in primo piano.
- In alto ci sono le **schede Giorno / Settimana / Mese / Anno**. All'apertura è attiva
  **Settimana**, sulla settimana corrente.
- Sotto le schede: il **totale grande** del periodo, il **nome del periodo** ("oggi", "10–16
  novembre", "novembre 2026", "2026"), le frecce **‹ ›** per passare al periodo prima e dopo, e la
  **media per giorno lavorativo**. La freccia › non va oltre il periodo in corso.
- **Grafico a barre** del periodo:
  - Settimana: 7 barre, una per giorno (lun…dom);
  - Mese: una barra per giorno del mese;
  - Anno: 12 barre, una per mese (G F M A M G L A S O N D);
  - i giorni futuri del periodo in corso sono barre vuote;
  - passando il mouse su una barra si vede il valore ("gio 13: 3 h 20 min").
- **Scheda Giorno**: niente grafico, al suo posto l'**elenco delle call** del giorno, con
  ora di inizio e fine, app e durata, dalla più recente. Con zero call: "Nessuna call".
- **Per app**: sotto il grafico o l'elenco, le app ordinate per tempo, con una barretta
  proporzionale e la durata.
- La call **in corso** è compresa nei totali e cresce mentre la finestra è aperta.
  Basta un aggiornamento ogni pochi secondi, senza ridisegnare di continuo.
- Lo stile è quello delle finestre esistenti in `ui.rs`: sfondo scuro, accento rosso delle call.
  La bozza approvata è in `.superpowers/brainstorm/19658-1794388580/content/statistiche-layout.html`,
  opzione A.

### Controllo sui dati (nelle Impostazioni)

Nelle Impostazioni c'è una sezione **"Le mie call"** con:

- il pulsante **"Le mie call…"** che apre la finestra;
- l'interruttore **"Registra le mie call"**, attivo di default:
  - da disattivato non si scrive più niente su disco, nemmeno la call in corso;
  - se si disattiva durante una call, quello che era già registrato resta e la registrazione si
    ferma in quel momento. Se si riattiva durante una call, si registra da quel momento;
  - lo storico già salvato resta e si può ancora guardare. Nella finestra "Le mie call" una riga
    avvisa "Registrazione disattivata";
- **"Cancella lo storico…"**: si scelgono una data **dal** e una data **al** (giorni interi, estremi
  compresi, fuso locale) e si conferma. La conferma dice cosa verrà cancellato:
  "Cancellare 37 call (21 h 40 min) dal 1° al 31 ottobre 2026?".
  - Una call a cavallo di un estremo viene **tagliata**: si cancella solo la parte dentro
    l'intervallo e il resto rimane.
  - Se l'intervallo comprende la call in corso, si cancella la parte già registrata e la call
    continua a essere registrata da quel momento.
  - Per cancellare tutto basta scegliere un intervallo che comprende tutto. Per comodità c'è un
    pulsante "Tutto" che imposta dalla prima call registrata a oggi.
  - La cancellazione è definitiva e usa la stessa scrittura atomica del salvataggio. Un eventuale
    file `…danneggiato` messo da parte non viene toccato.

### Anteprima e documentazione

- `--ui statistiche <file.png>` in `snapshot.rs` genera un'anteprima della finestra con dati di
  esempio, come per le altre finestre.
- Il README ha una sezione "Le mie call".

## Non obiettivi

- Condividere le statistiche con i colleghi o confrontarle.
- Statistiche sulle call **dei colleghi**.
- Esportazione (CSV o altro), modifica o cancellazione di singole call (si cancella solo per
  intervallo di date).
- Un promemoria per le call manuali dimenticate aperte ("Sei ancora in call?").
- Obiettivi, limiti o avvisi del tipo "oggi hai fatto troppe call".
- Le altre bozze (riepilogo con confronti, calendario dell'anno).

## Anti-obiettivi

- **Dati personali che escono dal PC**: niente nei pacchetti UDP, niente nei log oltre a quello
  che già c'è.
- **Perdere lo storico**: un file danneggiato o scritto a metà non deve cancellare le call degli
  anni prima. La scrittura deve essere atomica (file temporaneo e poi rinomina, come già fa
  `install.rs` per l'app). Se il file non si legge, va messo da parte (per esempio
  `call.json.danneggiato`) invece di essere sovrascritto.
- **Ore contate due volte o perse** al cambio nome, al riavvio dell'app o a cavallo della
  mezzanotte.
- **Una finestra che consuma CPU** quando è aperta e ferma.

## Vincoli

- Rust, egui 0.36, come il resto dell'app. Windows e macOS.
- Il **grafico si disegna con il painter di egui**, senza librerie di grafici.
- Il **calcolo** (divisione per giorno, raggruppamenti, medie, formati) va in un modulo puro e
  **testato**, separato dall'interfaccia e dall'archivio, come `indicators.rs` rispetto a
  `overlay.rs`.
- Il file sta nella cartella dell'app (`config::app_dir()`), accanto a `config.json`.

## Decisioni mie (da verificare leggendo)

- Una **finestra a sé** invece di una sezione nelle Impostazioni, che sono già lunghe.
- La **scheda Giorno è un elenco** invece di un grafico.
- **Scrittura all'inizio e ogni 20 secondi**, invece che solo a fine call.
- La scheda attiva all'apertura è **Settimana**.
- **Sospensione:** un buco di più di circa 60 secondi tra due salvataggi non conta come call.
- L'interruttore e la cancellazione stanno **nelle Impostazioni**, non nella finestra delle
  statistiche.
- Una call a cavallo di un estremo dell'intervallo da cancellare viene **tagliata**, non
  cancellata per intero.
- **Niente promemoria** per le call manuali dimenticate: nessuna risposta, da confermare.
- Per la data locale serve una libreria per date e fusi orari. Oggi l'app sa leggere l'ora locale
  solo su macOS (`platform::local_hm`). Consiglio `chrono` con le funzioni dell'ora locale, ma la
  scelta resta a chi costruisce.

## Lasciato a chi costruisce

- Il formato del file (JSON unico o JSON Lines) e il nome esatto.
- La struttura dei moduli (per esempio `callstats.rs` per il calcolo e `callstore.rs` per
  l'archivio).
- Le misure della finestra e i dettagli grafici, purché coerenti con la bozza A e con lo stile
  delle altre finestre.
- Come rendere il tooltip sulle barre.
- Il controllo per scegliere le date (per esempio il `DatePickerButton` di `egui_extras`, oppure
  campi giorno/mese/anno).
