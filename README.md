# ComicGPU

Nederlandstalige webapp voor herstel en controle van CBZ/CBR-comics, op Windows en Linux.

## Windows

Installeer Python 3.11 of nieuwer met de optie Add Python to PATH. Download deze repository en dubbelklik `start-windows.bat`. De browser opent de app op http://localhost:8501. De eerste start installeert de dependencies en vereist internet.

Voor CBR is aanvullend een door rarfile ondersteund uitpakprogramma nodig, bijvoorbeeld UnRAR op PATH. CBZ werkt zonder externe uitpaksoftware.

## Docker / Proxmox

In je Docker-VM, vanuit deze repository:

```sh
docker compose up -d --build
```

Open `http://IP-VAN-JE-VM:8501`. Gebruik dit binnen je vertrouwde thuisnetwerk. Deze app heeft geen gebruikersaccounts. De container bevat unar voor CBR. De standaard Docker-image verwerkt op CPU.

## Gebruik

1. Upload één of meerdere CBZ-, ZIP- of CBR-bestanden.
2. Kies een comic en pas desgewenst het profiel Digitale comic, Oude kleurenscan of Zwart-wit/manga toe.
3. Controleer de lijst met mogelijke duplicaten, spreads en lage resoluties.
4. Stel per pagina rechtzetten, randen, papierkleur, rugschaduw, doordruk en rasterverzachting in.
5. Bekijk origineel en resultaat naast elkaar, inclusief een detailuitsnede op ware pixelgrootte. De detailpositie is relatief aan iedere afbeelding, waardoor bijsnijden de vergelijking kan verschuiven.
6. Markeer gecontroleerde pagina’s als beoordeeld. Mogelijke duplicaten verwijder je zelf met Pagina uitsluiten.
7. Exporteer de batch en download de ZIP met verbeterde CBZ-bestanden.

Een nieuwe uitvoer overschrijft de upload niet. Instellingen en uploads leven in de browsersessie en verdwijnen bij een serverherstart. Opslag en verwerking zijn in geheugen. Verwerk grote collecties in kleinere batches.

## Bewerkingen en grenzen

- Rechtzetten zoekt naar horizontale structuur binnen ±3 graden. Illustraties en schuine panelen kunnen de schatting beïnvloeden.
- Bijsnijden verwijdert alleen vrijwel uniforme lichte of donkere randen, maximaal 5% per zijde, met 8 pixels marge.
- Papiercorrectie schat het witpunt. Rugschaduwcorrectie egaliseert brede helderheidsverschillen. Doordrukcorrectie licht bleke pixels op. Deze instelbare filters kunnen ook bedoelde lichte tekeningen veranderen.
- Rasterverzachting gebruikt een mengbare mediaanfilter. Dit is geen automatische herkenning van moiré of restauratie van verloren lijnen.
- 2× opschalen gebruikt Lanczos op CPU of bicubische interpolatie via PyTorch op CUDA. Het is geen AI-superresolutiemodel. Installeer voor CUDA een passende PyTorch-build volgens https://pytorch.org/get-started/locally/. GPU is optioneel en wordt uitsluitend gebruikt voor opschalen.
- Brede pagina’s (breedte > 1,25 × hoogte) worden als mogelijke spread gemarkeerd. Splitsen gebeurt alleen na inschakelen, in het midden. Voor manga kan de rechterhelft eerst.
- Duplicaatdetectie is een visuele hashvergelijking. Vergelijkbare of lege pagina’s kunnen ten onrechte worden gemarkeerd.
- Bestandsnamen bepalen de natuurlijke leesvolgorde (2 voor 10). Ontbrekende verhaalpagina’s kunnen niet automatisch vastgesteld worden.
- ComicInfo.xml blijft inhoudelijk behouden. PageCount en pagina-indexen worden aangepast bij export. Afgeleide afbeeldingsmaten en DoublePage-attributen worden verwijderd om verouderde gegevens te voorkomen.
- Overige archiefbijlagen worden niet meegenomen. Beschadigde afbeeldingen verschijnen als waarschuwing en worden overgeslagen. Controleer die waarschuwingen voor export.
- Maximaal 512 MB uitgepakte archiefinhoud en 3000 archiefitems per comic, 32 megapixels per invoerpagina en 16 megapixels bij 2× opschalen.

## Tests

```sh
python -m unittest discover -s tests -v
```

Tests dekken filters, metadata, splitsen met manga-volgorde, uitsluiten, natuurlijke sortering, beschadigde afbeeldingen en lege export. CUDA en echte CBR-archieven moeten aanvullend op het doelsysteem getest worden.
