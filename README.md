<div align="center">
  <img src="assets/icon.svg" width="100" alt="Icona OC Setup Easy">
  <h1>OC Setup Easy</h1>
  <p>Apri l’EFI, confronta i kext con <code>Kernel → Add</code> e modifica OpenCore dal tuo Hackintosh.</p>
  <p><b>Rust</b> · WebKit di macOS · Interfaccia in italiano · Funzionamento offline</p>
</div>

![Panoramica di OC Setup Easy con dati dimostrativi](assets/panoramica.png)

## Dalla partizione alla configurazione

1. In **Volumi EFI** individua il disco dal modello, dal collegamento e dall’identificatore. Premi **Monta**, poi **Apri EFI**. Per lavorare su una copia usa **Apri cartella EFI** e seleziona il volume, la cartella `EFI` o la cartella `OC` che contiene `config.plist`.
2. In **Verifica e backup** crea uno ZIP prima di intervenire. L’archivio contiene la cartella EFI quando OC è al suo interno. Se apri una cartella OC isolata, l’archivio contiene quella cartella.
3. In **Kext e componenti** confronta il contenuto delle directory con le voci registrate. **Copia un kext** importa il bundle su disco. **Registra** prepara la voce disattivata nel plist. Controlla i plug-in, attiva le voci desiderate e usa le frecce per impostare l’ordine.
4. In **Editor plist** naviga per sezione e modifica i campi in base al tipo. Usa la vista **XML** per creare o rinominare chiavi e modificare interi array. **Salva plist** o **⌘S** verifica e scrive la bozza. Chiudi la configurazione prima di smontare il volume.

## Cosa puoi controllare

| Vista | Dati e operazioni |
| --- | --- |
| **Panoramica** | CPU, core e thread, memoria fisica, versione e build macOS, kernel Darwin, architettura, modello SMBIOS e versione OpenCore riportata dalla NVRAM, quando disponibile. |
| **Volumi EFI** | Identificazione disco, dimensione della partizione, punto di mount, montaggio e smontaggio. macOS può chiedere la password amministratore. |
| **Editor plist** | Sezioni e chiavi personalizzate, dizionari, array e valori scalari. Lettura e salvataggio dei plist XML e binari nel formato originale. |
| **Kext e componenti** | Kext e plug-in, versioni, bundle ID, dipendenze dichiarate e loro ordine tra i bundle rilevati. Inventario e registrazione anche per ACPI, driver UEFI e strumenti. Rimuovere una voce conserva il file su disco. |
| **Verifica e backup** | Output completo di ocvalidate sulla bozza, copia del plist precedente a ogni salvataggio e archivio ZIP su richiesta. |
| **Hardware e sistema** | Boot arguments, kext effettivamente caricati, profili energia e report disponibili sulla macchina tramite system_profiler, tra cui GPU, audio, PCI, USB, rete e NVMe. |

### Salvataggi e ripristino

L’app verifica che il plist su disco corrisponda ancora alla copia aperta. Se un altro editor lo ha modificato, il salvataggio si ferma. La nuova versione viene preparata in un file temporaneo dello stesso volume e sostituisce l’originale dopo la creazione del backup. L’importazione rifiuta un bundle omonimo già presente.

Apri **Verifica e backup → Mostra backup** per raggiungere `~/Library/Application Support/OC Setup Easy/Backups`. Per ripristinare un plist, chiudi la configurazione, copia il backup scelto in `EFI/OC` con il nome `config.plist`, poi riapri la cartella. Per recuperare altri file, estrai lo ZIP in una cartella separata e controllane il contenuto.

Il validatore incluso usa lo schema di **OpenCore 1.0.8**. Le regole possono cambiare tra versioni. Un controllo superato riguarda la configurazione e non garantisce l’avvio dell’hardware. La colonna di stato dei componenti segnala separatamente i file assenti. [Documentazione e release ufficiali di OpenCore](https://github.com/acidanthera/OpenCorePkg/releases/tag/1.0.8)

## Compilazione e installazione

Servono macOS, gli strumenti da riga di comando di Xcode e Rust stabile con Cargo. Il bundle viene compilato per l’architettura del Mac che esegue la build. Il target minimo dichiarato è macOS 11. La versione x86-64 è stata provata su un Hackintosh AMD con macOS 26.7.1.

Dalla cartella del repository:

```sh
./scripts/build-app.sh
open "dist/OC Setup Easy.app"
```

Per installare l’app, trascina **OC Setup Easy.app** da `dist` in **Applicazioni**. La prima compilazione scarica le dipendenze Rust e l’archivio ufficiale OpenCore 1.0.8. Lo script controlla lo SHA-256 dell’archivio, estrae ocvalidate e firma localmente il bundle. L’app installata funziona offline.

### Sorgenti e test

`src/` contiene il motore Rust. `native/window.m` collega AppKit e WKWebView ai comandi Rust. `ui/` contiene l’interfaccia HTML, CSS e JavaScript, il font e la grafica locale.

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

I test coprono identificatori di partizione, lettura da una EFI annidata, backup, conflitti di scrittura, plist binari, importazione di kext con plug-in e gestione dei duplicati.

OpenCore e ocvalidate sono progetti di Acidanthera. Le licenze di ocvalidate e del carattere Barlow Semi Condensed accompagnano il bundle. [Licenza OpenCorePkg](licenses/OpenCorePkg.txt) · [Licenza Barlow](ui/FONT-LICENSE.txt)
