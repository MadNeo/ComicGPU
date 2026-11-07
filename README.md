# ComicGPU 0.2 — Rust

Comicrestauratie met één Rust-kern, een Windows-app (Tauri) en een webserver (Axum). De bediening is gedeeld. Python is niet nodig voor de Rust-versie.

## Windows gebruiken

Open de [GitHub Actions-builds](https://github.com/MadNeo/ComicGPU/actions), kies een geslaagde **Rust checks and builds**-run en download **ComicGPU-Windows**. Pak de ZIP uit en start `comicgpu-desktop.exe`. De app opent een eigen venster en kiest zelf een vrije lokale poort. Microsoft Edge WebView2 Runtime moet aanwezig zijn. `comicgpu-server.exe` kan desgewenst los worden gestart en toont de browser-URL in de console.

Voor CBR gebruikt Windows de ingebouwde `C:\Windows\System32\tar.exe` (libarchive). Als jouw Windows-versie dit RAR-bestand niet ondersteunt, converteer het eerst naar CBZ. CBZ heeft geen externe archiefsoftware nodig.

Zelf bouwen vereist de Rust MSVC-toolchain, Visual Studio C++ Build Tools en WebView2:

```powershell
cargo build --locked --release -p comicgpu-desktop -p comicgpu-server
.\target\release\comicgpu-desktop.exe
```

De workflow levert uitvoerbare bestanden. Ze zijn niet digitaal ondertekend en er is nog geen installer gepubliceerd.

## Docker / Proxmox

Vanuit een checkout van deze repository in je Docker-VM:

```sh
docker compose up -d --build
docker compose logs comicgpu
```

Gebruik de getoonde start-URL en vervang `0.0.0.0` door het IP-adres van je VM. Neem `?token=...` over. Het token wordt willekeurig gemaakt bij elke serverstart. Voor een vast token kun je `COMICGPU_TOKEN` als omgevingsvariabele instellen (minimaal 16 tekens). Bewaar dat token privé. Gebruik de server binnen je vertrouwde netwerk. Er zijn geen afzonderlijke gebruikersaccounts.

De container bevat libarchive-tools voor CBR. GPU-toegang vanuit Docker/LXC vraagt passende hostdrivers en device-doorgifte. CPU werkt standaard. De hardwaremelding laat zien welke grafische backend beschikbaar is. Softwarematige adapters kunnen ook worden gemeld.

Direct op Linux:

```sh
cargo build --locked --release -p comicgpu-server
./target/release/comicgpu-server
```

De standaard server luistert op `127.0.0.1:8501`. Stel voor je LAN `COMICGPU_BIND=0.0.0.0:8501` in. Installeer `libarchive-tools` voor CBR.

## Werkwijze

1. Voeg één of meer CBZ/CBR-comics toe, of selecteer een map. Bij een map worden ook comics in submappen geselecteerd.
2. Kies een comic en pas eventueel het profiel Digitale comic, Oude kleurenscan of Zwart-wit/manga toe.
3. Gebruik de controlelijst om mogelijke duplicaten, lage resoluties en spreads te beoordelen.
4. Stel per pagina correcties in en klik op **Voorvertoning bijwerken**.
5. Vergelijk de afbeeldingen, eventueel op 100% pixelgrootte. Beide beelden scrollen relatief mee. Door bijsnijden of opschalen kunnen overeenkomstige details op andere posities staan.
6. Markeer beoordeelde pagina’s. Sluit ongewenste pagina’s zelf uit. Splits brede pagina’s desgewenst in het midden, met een optie voor rechterhelft eerst.
7. Kies 1–4 parallelle pagina’s en verwerk alle comics. Download elke verbeterde CBZ via de getoonde links.

Uploads en resultaten worden in geheugen verwerkt. De werkruimte blijft bestaan zolang de server draait, maar de browserinstellingen verdwijnen na herladen. Verwijder afgehandelde comics met de verwijderknop. Bij grote scans of weinig RAM gebruik je één parallelle pagina en kleine batches. De volledige uitvoer van één comic staat tijdens export in geheugen.

## Functies en grenzen

- Automatisch rechtzetten binnen ±3 graden met witruimte rondom, zodat rotatie geen hoeken afsnijdt.
- Uniforme scanranden bijsnijden tot maximaal 5% per zijde, met acht pixels marge.
- Instelbare papierkleurcorrectie, brede schaduwcorrectie, lichte doordrukcorrectie en mediaanfilter voor rasterverzachting.
- Zwart-witconversie, 2× Lanczos-opschaling op CPU en 2× bicubische opschaling via wgpu. Dit zijn klassieke filters. De kwaliteit hangt af van de scan en gekozen sterkte.
- wgpu ondersteunt passende grafische backends zoals DirectX 12 en Vulkan. Beschikbaarheid en maximale afbeeldingsgrootte hangen af van driver en apparaat. GPU wordt uitsluitend gebruikt voor opschalen. Bij een fout kun je GPU uitschakelen.
- Natuurlijk sorteren van bestandsnamen, melding van beschadigde pagina’s en vergelijking van visuele hashes voor mogelijke duplicaten. Automatische signalen vragen beoordeling.
- ComicInfo.xml blijft inhoudelijk behouden. PageCount en Image-indexen worden opnieuw opgebouwd, ook wanneer een beschadigde pagina wegvalt. Afgeleide maatgegevens en DoublePage-attributen worden verwijderd.
- Spreads worden alleen gesplitst na inschakelen. De voorvertoning toont de volledige bewerkte pagina met een melding over de uitvoervolgorde.
- Uitvoer is CBZ met PNG-pagina’s. CBR wordt alleen ingelezen. Andere archiefbijlagen worden niet geëxporteerd.
- Maximaal 512 MB archief/invoerinhoud, 3000 archiefitems, 64 MB per gecodeerd paginabestand, 32 megapixels per afbeelding en 16 megapixels invoer bij 2× opschalen. De totale werkruimte accepteert maximaal 1 GB gecodeerde pagina’s. Gedecodeerde beelden en uitvoer vragen aanvullend RAM.

## Back-up en rollback

De volledige vorige Python-versie staat op de permanente branch:

**[backup/python-2026-11-07](https://github.com/MadNeo/ComicGPU/tree/backup/python-2026-11-07)**

Exacte commit: `34bdd1bd2772e730de330d826d2d902b03633e50`.

Om de vorige versie apart uit te checken:

```sh
git fetch origin
git worktree add ../ComicGPU-python origin/backup/python-2026-11-07
```

Op Windows kun je vervolgens `../ComicGPU-python/start-windows.bat` starten met Python geïnstalleerd. Voor Docker stop je eerst de Rust-container met `docker compose down`. Start daarna de oude versie vanuit `../ComicGPU-python` met `docker compose -p comicgpu-python up -d --build`. Beide gebruiken standaard poort 8501. De vorige README en dependencies staan volledig op de back-upbranch.

Sla gewenste resultaten vóór het stoppen op. Sessies en nog niet gedownloade resultaten zijn tijdelijk. Je bronbestanden blijven op hun oorspronkelijke locatie.

## Ontwikkeling en verificatie

```sh
cargo fmt --all --check
cargo test --locked -p comicgpu-core
cargo build --locked -p comicgpu-server
cargo build --locked -p comicgpu-desktop --target x86_64-pc-windows-msvc
```

De tests controleren pixelbehoud bij het digitale profiel, filteruitvoer, invoervalidatie, natuurlijk sorteren, beschadigde pagina’s, duplicaatsignalen, manga-splitsing, ComicInfo-remapping, beoordeling vóór export en cropmarges. De GPU-test draait wanneer een adapter beschikbaar is. De GitHub-workflow bouwt de Linux-server en Windows-uitvoer. Tijdens de migratie zijn daarnaast upload, profielkeuze, preview, beoordeling, batch-export en downloaden in een headless browser getest. De Windows-app is vanaf Linux naar Windows gecompileerd, maar niet op een Windows-desktop uitgevoerd. GPU-verwerking is getest via een softwarematige adapter (llvmpipe), nog niet op een fysieke RTX 4080. Een eenvoudig RAR4-archief is via libarchive ingelezen; niet alle RAR-varianten zijn getest.
