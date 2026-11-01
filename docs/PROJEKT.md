# DJejaj — projekt aplikacji DJ-skiej

Data: 30 października 2026. Status: specyfikacja docelowa; implementacja 0.1.0 w toku.
Nazwa jest robocza. Wszystkie podane parametry wydajności są celami do pomiaru,
a nie wynikami działającej aplikacji.
Wykonany zakres i ograniczenia są opisane osobno w [stanie implementacji](IMPLEMENTACJA.md).

## 1. Cel i sposób użycia

Natywna aplikacja na MacBooka do grania i tworzenia muzyki na żywo. Użytkownik
wskazuje katalog muzyczny, wybiera dwa utwory, miksuje je, zmienia tempo,
uruchamia efekty i dokłada własne lub wygenerowane sample. Cały występ oraz
konfiguracja konsoli muszą być możliwe z klawiatury, bez ekranu dotykowego.
Priorytetem są imprezy i długie sety techno, trance, rave oraz pokrewnych
gatunków: szybkie znalezienie następnego utworu, ciągłość muzyki, kontrola
energii i improwizacja rytmiczna. Rozbudowane możliwości są odsłaniane stopniowo;
podstawowy występ nie wymaga znajomości wszystkich funkcji.

Typowa sesja:

1. Wybierz folder i poczekaj na podstawowy indeks; analizę można kontynuować w tle.
2. W stale widocznej bibliotece wybierz utwór A, a następne dodaj do kolejki setu.
3. Ustaw poziomy, tempo, punkty startu i ewentualne pętle.
4. Przygotuj B w odsłuchu i przejdź do niego na wybranej granicy frazy,
   korzystając z faderów kanałów lub crossfadera, EQ i efektów.
5. Dodawaj perkusję, bas i przejścia z samplera oraz sekwencera.
6. Załaduj kolejny utwór do zwolnionego decka i kontynuuj set; nagraj miks
   do pliku WAV i zapisz ustawienia sesji.

Szczegółowy model biblioteki, tryb LIVE i warsztat muzyczny definiuje
[Praca na żywo i wiedza muzyczna](PRACA_NA_ZYWO.md). Jest to część wymagań 1.0,
z wyjątkiem funkcji wyraźnie oznaczonych jako późniejsze rozszerzenia.

## 2. Zakres wersji 1.0

| Obszar | Planowane możliwości |
| --- | --- |
| Biblioteka | Stały panel całej kolekcji, foldery, indeks przyrostowy, filtry BPM/tonacji/tagów, playlisty, kolejka i historia setu |
| Formaty bazowe | MP3, WAV PCM, FLAC; pliki mono i stereo, co najmniej 44,1/48 kHz |
| Formaty dodatkowe | AIFF PCM, Ogg Vorbis, M4A z AAC-LC lub ALAC po testach kodeków |
| Odtwarzanie | Dwa niezależne decki, play/pause, cue, przewijanie, cztery hot cue na deck |
| Mikser | Gain, trzypasmowy EQ z kill, filtr, fadery kanałów, crossfader z trybem THRU, master stereo |
| Tempo | Varispeed 0,5–1,5×; key lock początkowo 0,8–1,2×; chwilowy pitch bend |
| Rytm | Globalny zegar BPM, tap tempo, beatgrid/downbeat, licznik taktów i fraz, ręczne markery, SYNC i pętle |
| Dobór muzyki | Tonacja z pewnością analizy i korektą, zgodność harmoniczna, energia i rola utworu; wyjaśniane sugestie |
| Efekty | Filtr rezonansowy, echo/delay, reverb, saturacja, chorus i flanger |
| Tworzenie | Osiem padów/ścieżek, 16 głosów, sekwencer 16–64 kroków, bas nutowy, osiem wzorców na ścieżkę, do 32 scen |
| Występ | Domyślny tryb LIVE, ochrona grającego decka, jawne ładowanie A/B, odsłuch cue, odwoływalne akcje oczekujące |
| Obraz dźwięku | Waveform obu decków, pozycja, beatgrid, mierniki L/R i widmo mastera |
| Trwałość | Profile klawiatury, presety, sesje i nagranie mastera WAV stereo 24-bit |

M4A oznacza kontener: nie każdy kodek w takim pliku będzie obsługiwany.
Nieobsługiwany lub uszkodzony plik otrzymuje zrozumiały status i nie blokuje skanu.
Pliki DRM i kanały wielokanałowe są poza zakresem 1.0; mono trafia na oba kanały.
Konwersja częstotliwości próbkowania jest automatyczna i jawna w diagnostyce.

Późniejsze rozszerzenia: MIDI, AU/VST, stem separation, scratching, zewnętrzne
wejścia audio, streaming i zaawansowana aranżacja. Wersja 1.0 działa offline,
bez konta i usług chmurowych. Generowanie brzmień bazuje na syntezie DSP.

## 3. Biblioteka i dane użytkownika

Indeks zapisuje ścieżkę, identyfikator utworu, tytuł, wykonawcę, album, czas,
format, sample rate, liczbę kanałów, rozmiar i czas modyfikacji. Analiza dodaje
waveform, BPM, beatgrid, downbeat oraz tonację z osobną miarą pewności każdej
analizy. Niepewne BPM i tonacja pozostają oznaczone; użytkownik może je
poprawić, ustawić pierwszy beat oraz markery intro, break, drop i outro.
Energia 1–5, gatunek, charakter i rola w secie są początkowo tagami użytkownika.
Historia rozróżnia utwory załadowane, odsłuchane i faktycznie zagrane na masterze.

Skan jest anulowalny i pokazuje postęp. Najpierw pojawiają się metadane,
potem kosztowniejsza analiza. Odtwarzanie ma pierwszeństwo przed indeksowaniem.
Zmiana pliku unieważnia jego analizę, lecz nie usuwa bez ostrzeżenia ręcznych
cue. Niedostępny dysk lub przeniesiony plik pozostaje wpisem „brak pliku”
z możliwością wskazania nowej lokalizacji. Symlinki nie mogą tworzyć pętli skanu.

Oryginalne pliki są tylko odczytywane. SQLite przechowuje indeks i playlisty;
wersjonowany JSON — profile i sesje; odtwarzalne cache — waveform i analizę.
Dane trafiają do `~/Library/Application Support/DJejaj/`, cache do
`~/Library/Caches/DJejaj/`, a nagrania do folderu wybranego przez użytkownika.
Zmiany schematu wymagają migracji i kopii danych użytkownika; cache można odbudować.

Zapis sesji obejmuje referencje do utworów, cue, pętle, BPM, parametry miksera,
efektów, padów, wzorców, scen, tagów muzycznych, kolejki i historii setu.
Odtworzenie sesji rozpoczyna się w pauzie, w trybie LIVE.
Sesja nie zawiera kopii muzyki; aplikacja zgłasza brakujące pliki.

## 4. Konsola i estetyka

Kierunek: profesjonalna konsola o grafitowej obudowie, ciemnoszarych panelach,
matowych gałkach i czytelnych podziałkach. Deck A ma akcent bursztynowy, B —
turkusowy; clipping jest czerwony. Kolor uzupełniają nazwy, ikony i wartości.
Subtelne światło oraz cienie podkreślają położenie elementów, bez utrudniania odczytu.

W lewym górnym rogu konsoli wyświetlamy logo tekstowe **DJ EJAJ** — dokładnie
w tej pisowni, dużymi literami, wyraźną, pogrubioną czcionką o wysokim kontraście.
Bezpośrednio pod nim znajduje się podpis **by NT4.pl**, zapisany mniejszą,
czytelną czcionką. Obie linie są wyrównane do lewej i pozostają widoczne
we wszystkich kontekstach obsługi konsoli.

Układ roboczy, nie gotowa makieta graficzna:

```text
┌──────────────────────────────────────────────────────────────────┐
│ DJ EJAJ  LIVE  BPM / FRAZA    AUDIO / BUF / LOAD    REC / PANIC  │
│ by NT4.pl                                                       │
├────────────────────────┬─────────────────┬───────────────────────┤
│ DECK A                 │ MIKSER          │ DECK B                │
│ Tytuł · BPM · czas      │ Gain A / B      │ Tytuł · BPM · czas     │
│ Waveform + beatgrid    │ EQ A / B        │ Waveform + beatgrid   │
│ Play · Cue · Loop      │ Fadery · VU L/R │ Play · Cue · Loop     │
│ Tempo · Key lock      │ Crossfader/THRU │ Tempo · Key lock      │
├────────────────────────┴─────────────────┴───────────────────────┤
│ FX: FILTER / DRIVE / DELAY / REVERB    ·    GAŁKI + KLAWISZE      │
├──────────────────────────────────────────────────────────────────┤
│ 8 PADÓW · SCENA GRA / NASTĘPNA · SEKWENCER 16–64 (ROZWIJANY)    │
├──────────────────────────────────────────────────────────────────┤
│ BIBLIOTEKA: WSZYSTKIE / PLAYLISTY · SZUKAJ · BPM / KEY / ENERGIA│
│ Status A/B · Tytuł / wykonawca · BPM · Tonacja · Energia · Czas  │
│ Przewijana lista utworów       │ NASTĘPNE / HISTORIA SETU       │
└──────────────────────────────────────────────────────────────────┘
```

Każda gałka pokazuje krótką nazwę, pozycję wskazówki, łuk zakresu, wartość
z jednostką i dwa przypisane klawisze, np. `FILTER · 1,2 kHz · Q − / A +`.
Wartość neutralna ma wyraźny znacznik; aktywny panel i deck mają obwódkę.
Efekty globalne i efekty decka mają jednoznaczne etykiety celu.

Projektujemy najpierw dla obszaru 1280×800 punktów, ze skalowaniem Retina.
Biblioteka zajmuje stale dolną część ekranu i pokazuje co najmniej osiem
pełnych wierszy przy tym rozmiarze. Może zostać powiększona do przeglądania,
z zachowaniem transportów i mastera. Edytor sekwencera rozwija się kosztem
szczegółów FX i wysokości waveformów, pozostawiając co najmniej cztery wiersze
biblioteki. Na mniejszych oknach stosujemy przewijanie i zwijanie szczegółów;
transport, poziomy i aktualne przypisania pozostają widoczne. Wizualizacje
pokazują rzeczywisty stan silnika, również podczas obciążenia interfejsu.

## 5. Sterowanie klawiaturą

Przykładowy profil początkowy, edytowalny i zapisywany przez użytkownika:

| Klawisze | Działanie |
| --- | --- |
| `1` / `2` | Wybór aktywnego decka A / B |
| `Spacja` | Start aktywnego decka w LIVE; play/pause w trybie przygotowania |
| `Shift+Spacja` przytrzymane 700 ms | Świadome zatrzymanie aktywnego decka w LIVE, z widocznym postępem |
| `Q` / `A` | Zmniejsz / zwiększ częstotliwość filtra aktywnego decka |
| `W` / `S` | Zmniejsz / zwiększ send delay aktywnego decka |
| `E` / `D` | Zmniejsz / zwiększ send reverb aktywnego decka |
| `R` / `F` | Zmniejsz / zwiększ drive aktywnego decka |
| `T` / `G` | Zmniejsz / zwiększ tempo aktywnego decka |
| `←` / `→` | Crossfader w kontekście grania; wybór kroku w edytorze sekwencera |
| `Z X C V B N M ,` | Osiem padów samplera |
| `Tab` / `Shift+Tab`, `Enter` | Nawigacja i aktywacja elementów |
| `Esc` | Wyjście z wpisywania tekstu lub zamknięcie okna podrzędnego |
| `Cmd+.` | PANIC: szybkie, łagodne wyciszenie wszystkich źródeł i ogonów FX |
| `Cmd+L` / `Cmd+J` / `Cmd+K` | Fokus biblioteki / grania / sekwencera, bez zatrzymywania audio |
| `Cmd+F` | Wyszukiwanie w bibliotece |
| `Cmd+1` / `Cmd+2` | Jawne ładowanie wybranego utworu do A / B, z ochroną grającego decka |

Aktywny kontekst jest zawsze opisany. `↑/↓` wybiera utwór w bibliotece lub
ścieżkę w sekwencerze. `Enter` w bibliotece otwiera działania dla utworu,
a w sekwencerze włącza/wyłącza wybrany krok; nigdy sam nie ładuje utworu do decka.
Skróty regulatorów i padów działają w kontekście grania; edytory mają jawne
lokalne mapowania. `Cmd+.` pozostaje dostępne w każdym kontekście.

Każdy regulator i przycisk jest dostępny nawigacją klawiaturową, również
gdy nie ma szybkiego skrótu. Użytkownik może przypisać np. `E/B` zamiast `E/D`,
jeśli usunie konflikt z padem. Edytor wykrywa kolizje w aktywnych kontekstach,
pozwala zamienić przypisania, ustawić krok, kierunek i przywrócić profil.

Przytrzymanie reguluje parametr płynnie według czasu, niezależnie od systemowej
częstotliwości powtarzania klawisza. Oba kierunki naraz zatrzymują zmianę.
Dokładna regulacja jest dostępna z `Shift`; zakres częstotliwości jest logarytmiczny.
Przełączenie decka zwalnia trzymane regulatory, aby nie zmienić przypadkiem nowego celu.

W wyszukiwarce litery wpisują tekst, a Spacja nie uruchamia utworu. Skrót
zatrzymania decka wymaga kontekstu grania; czas przytrzymania liczymy sami.
Po utracie
fokusu zwalniamy klawisze i głosy typu gate; grające decki pozostają aktywne.
PANIC ma pierwszeństwo we wszystkich kontekstach aplikacji i nie wznawia dźwięku
automatycznie. Nie przechwytujemy globalnie klawiatury innych aplikacji.
Testujemy układ polski programisty i US oraz ograniczenia równoczesnych naciśnięć
wbudowanej klawiatury; skróty systemowe macOS pozostają zarezerwowane.

## 6. Tempo, pętle i tworzenie muzyki

**Varispeed:** zmiana prędkości zmienia także wysokość dźwięku, podobnie do
gramofonu. **Key lock:** zmiana tempa z zachowaniem tonacji wymaga osobnego
algorytmu; jej jakość oraz dodatkowe opóźnienie podlegają odsłuchowi i pomiarom.
Zakresy z tabeli oznaczają plan obsługi, nie obietnicę jednakowej jakości
dla każdego materiału. Pitch bend chwilowo koryguje dopasowanie i wraca po zwolnieniu.

Globalny zegar steruje tempem, sekwencerem, delayem i kwantyzacją. Deck może
być źródłem BPM dla zegara; nie może powstać kołowa zależność synchronizacji.
SYNC wyrównuje tempo i fazę względem poprawionego beatgridu. Początkowo zakładamy
stałe tempo utworu; nagrania ze zmiennym tempem wymagają ręcznego miksowania.

Pętle: 1, 2, 4, 8 i 16 beatów, z kwantyzacją oraz krótkim przejściem na granicy.
Cue jest punktem startu, a hot cue natychmiastowym lub kwantyzowanym skokiem.
UI pokazuje, czy akcja została wykonana, czy czeka na najbliższy beat.

Sampler obsługuje one-shot, gate i loop, głośność, panoramę, strój i obwiednię.
Planowany limit początkowy to 16 jednoczesnych głosów; przekroczenie stosuje
przewidywalne wygaszenie najstarszego głosu. Osiem ścieżek sekwencera obsługuje
16, 32 lub 64 kroki z widoczną stroną 16 kroków, próbki oraz monofoniczny bas
nutowy; jego głos mieści się w łącznym limicie 16. Każda ścieżka ma do ośmiu
wzorców. Do 32 scen przywołuje ich zestawy i mute, bez przejmowania transportu
decków. Zmiany są planowane na granicy taktu lub ręcznie określonej frazy.
Velocity, gate, akcent, swing i slide basu służą muzycznej ekspresji; dokładne
reguły, wzorce startowe i ograniczenia opisuje [warsztat LIVE](PRACA_NA_ZYWO.md).

W etapie samplera powstanie własny bank: kick, snare, clap, closed/open hi-hat,
bas, riser i impact. Brzmienia generujemy lokalnie z oscylatorów, szumu,
filtrów i obwiedni, z kontrolowanym ziarnem losowania. Bank zawiera WAV stereo
48 kHz/24-bit, receptury i manifest pochodzenia. Źródła perkusyjne mogą być
centralne, a przestrzeń powstaje przez przetwarzanie stereo. Żadnego kopiowania
cudzych nagrań. To zakres przyszłej implementacji; sample nie są jeszcze wygenerowane.

## 7. Efekty i tor stereo

| Efekt | Główne parametry | Charakter i warunki jakości |
| --- | --- | --- |
| Filtr | Cutoff, resonance, LP/HP | Płynna regulacja, stabilne skrajne wartości |
| Delay | Podział beatu/ms, feedback, send | Stereo/ping-pong, ograniczone sprzężenie |
| Reverb | Decay, size, damping, send | Przestrzeń stereo, płynnie wygasający ogon |
| Saturacja | Drive, tone, output | Miękkie nasycenie, kompensacja poziomu, kontrola aliasingu |
| Chorus/flanger | Rate, depth, feedback, mix | Modulacja stereo, kontrola zgodności mono |

Presety analogowego charakteru: Tape Echo (filtracja powtórek, delikatne
nasycenie i wow/flutter), Warm Drive i Vintage Space. To interpretacje brzmienia,
bez deklaracji dokładnej emulacji konkretnego urządzenia. Szum jest opcjonalny
i domyślnie wyłączony. Phaser, bitcrusher i granular freeze to rozszerzenia po 1.0.

Tor decka: dekodowanie → prędkość/key lock → gain → EQ → filtr → drive/modulacja
→ fader kanału → crossfader. Po crossfaderze pobieramy sygnał na wspólne szyny
delay i reverb. Ich powroty trafiają do mastera: zamknięcie kanału przestaje
wzbudzać efekt, ale pozwala wybrzmieć jego ogonowi. Sampler ma własny poziom
i sendy, jest sumowany poza crossfaderem. Powroty FX nie wysyłają się wzajemnie.

Do długich przejść elektronicznych przewidujemy tryb THRU: oba decki omijają
crossfader, a proporcje ustalają fadery kanałów. Położenie crossfadera w THRU
nie ma wpływu na dźwięk i jest tak opisane. Zmiana przypisania do crossfadera
jest blokowana podczas gry danego decka w LIVE, aby nie zmienić nagle poziomu.
W trybie przygotowania przejście ma rampę i widoczny status.
Szyny FX nadal pobierają sygnał po skutecznym wzmocnieniu kanału.
Filtracja dołu na wejściu szyn delay/reverb oraz makra z ograniczonymi zakresami
wspierają czytelność miksu; szczegóły opisuje [warsztat LIVE](PRACA_NA_ZYWO.md).

Suma → master gain → limiter z powiązanym sterowaniem L/R → nagranie i wyjście.
Przetwarzanie wewnętrzne: `f32`, dwa kanały, wspólny zegar próbek. Delay/reverb
mają rzeczywiste przetwarzanie stereo; EQ nie zwęża sceny. Zmiany parametrów,
bypass i transport mają rampy, aby ograniczyć trzaski. Limiter z sufitem
−1 dBFS ogranicza szczyty próbek; pomiar true peak jest osobnym testem jakości.

Podstawowa konfiguracja zapewnia master stereo na jednym urządzeniu.
Oddzielny odsłuch słuchawkowy stereo wymaga wyjścia czterokanałowego: master
1–2, cue 3–4, na wspólnym zegarze interfejsu. Cue pobieramy przed faderem kanału,
z osobną głośnością. Brak takiego sprzętu jest jasno komunikowany; nie dzielimy
mastera na dwa kanały mono ani nie obiecujemy synchronizacji dwóch osobnych urządzeń.

## 8. Architektura i technologie

Rust jest proponowany dla kontroli pamięci, współbieżności i wydajności.
Sam wybór języka nie zapewnia niskiego opóźnienia; potrzebne są właściwy tor
audio, ograniczona praca w callbacku i pomiary na MacBooku.

| Element | Propozycja i uzasadnienie |
| --- | --- |
| Silnik i logika | Rust, workspace Cargo, moduły niezależne od GUI |
| Audio urządzenia | CPAL, którego backendem na macOS jest CoreAudio ([źródło](https://github.com/RustAudio/cpal)) |
| Dekodowanie | Symphonia: MP3/WAV/FLAC oraz wybrane kolejne kodeki; MP3 wymaga aktywacji feature ([źródło](https://github.com/pdeljanov/Symphonia)) |
| Interfejs | egui/eframe, aplikacja natywna z renderowaniem wgpu; własne kontrolki konsoli ([źródło](https://github.com/emilk/egui/blob/main/crates/eframe/README.md)) |
| Key lock | Do oceny Signalsmith Stretch: biblioteka C++ do pitch/time, MIT, z raportowaniem opóźnienia ([źródło](https://github.com/Signalsmith-Audio/signalsmith-stretch)) |
| Trwałość | SQLite oraz wersjonowany JSON; zapisy poza wątkiem audio |

Signalsmith oznacza niewielką, odizolowaną integrację C++/FFI, jeśli przejdzie
próby. Preferujemy Rust dla pozostałego kodu; czysto rustowy zamiennik musi
przejść te same testy jakości. W etapie 0 zapisujemy decyzję, zamiast od razu
tworzyć własny zaawansowany algorytm rozciągania czasu. Wersje bibliotek,
MSRV i minimalny macOS zostaną przypięte po próbach; dokumentacja gałęzi głównej
nie zastępuje testów konkretnego wydania. Symphonia deklaruje MPL-2.0
([licencja projektu](https://github.com/pdeljanov/Symphonia)); przed dystrybucją
trzeba zachować wymagane informacje licencyjne wszystkich wybranych zależności.

Planowany podział, jeszcze nieutworzony:

```text
crates/app/         okno, konsola, fokus, ustawienia, mapowanie klawiszy
crates/audio/       transport, zegar, decki, mikser, routing i urządzenia
crates/dsp/         EQ, efekty, limiter, resampling i adapter key lock
crates/library/     indeks, dekodowanie w tle, metadane, cache analizy
crates/sampler/     synteza, głosy, pady, sekwencer i receptury
assets/            fonty, ikony, presety i wygenerowany bank
tests/fixtures/    małe, legalnie redystrybuowalne pliki testowe
docs/              specyfikacja, decyzje i protokoły odbioru
```

Trzy oddzielne obszary wykonania:

1. **Audio:** gotowe bufory PCM, miksowanie, DSP, zegar, zaplanowane zdarzenia.
   Zero odczytu dysku, zapytań SQL, logowania, blokujących locków i alokacji
   oraz zwalniania pamięci w callbacku. Bufory i głosy są przygotowane wcześniej.
2. **Prace w tle:** skan, dekodowanie z wyprzedzeniem, analiza, cache i zapis WAV.
   Kolejki i pamięć są ograniczone; dekodowanie ma pierwszeństwo przed analizą.
3. **GUI:** klawisze, ustawienia i wizualizacja. Zablokowane GUI nie przerywa audio.

Komendy trafiają do ograniczonych kolejek; ciągłe regulatory mogą scalać zmiany.
PANIC i zwolnienia klawiszy mają osobny, odporny na przepełnienie mechanizm.
Audio odsyła aktualne wartości i zredukowane dane mierników bez oczekiwania
na GUI. Wyzwalanie sampli i sekwencera jest planowane względem zegara próbek.
Indeksowane zapytania biblioteki, podpowiedzi doboru muzyki, preview cue oraz
przygotowanie następnego utworu nie blokują odtwarzania. Wirtualizowana lista
rysuje tylko widoczne wiersze; PCM nie obejmuje całej kolekcji. Limit cache,
budżet scen i kolejność ograniczania prac w tle definiuje warsztat LIVE.
Miejsce wykonywania key lock wybieramy po profilowaniu, z uwzględnieniem
kosztu, buforowania, seek i pełnego opóźnienia.

Brak PCM powoduje krótkie wygaszenie i licznik błędów, bez powtórzenia starego
bufora. Przewinięcie unieważnia poprzednio przygotowane dane. Odłączenie wyjścia
wycisza sesję; ponowny start wymaga świadomego działania. Zapis nagrania korzysta
z kolejki; brak miejsca lub jej przepełnienie kończy nagrywanie z komunikatem,
bez zatrzymania miksu i bez ukrywania luk w pliku.

## 9. Cele jakościowe i pomiary

Punkt odniesienia do potwierdzenia: MacBook Apple Silicon M1, 8 GB RAM lub
konkretny dostępny model zapisany w etapie 0. Priorytetem jest Apple Silicon;
wersja Intel wymaga osobnego przebiegu testów. Nie deklarujemy wyników na
nieprzetestowanym sprzęcie. Pomiary wykonujemy w kompilacji release.

| Kryterium | Cel i warunki |
| --- | --- |
| Format roboczy | Preferowane 48 kHz; obsługa 44,1 kHz, bez wymuszania nieobsługiwanego trybu urządzenia |
| Bufor wyjścia | Cel 128/256 ramek, stabilny fallback 512; raportujemy faktyczny bufor |
| Responsywność | Cel ≤20 ms od przyjęcia komendy audio do wyjścia przewodowego bez key lock; ≤50 ms z key lock |
| Stabilność | 0 niedoborów danych i przekroczeń terminu callbacku podczas godzinnego testu obciążenia i dwugodzinnego setu LIVE |
| Zapas DSP | Czas callbacku p99 <50% jego budżetu, maksimum <80% w tej samej sesji |
| Pamięć | Cel ≤1 GB RSS przy 10 tys. wpisów, dwóch deckach i banku sampli; ograniczony cache PCM |
| UI | Cel 60 kl./s w konsoli, dopuszczalne 30 podczas analizy; audio zachowuje priorytet |
| Biblioteka | Przy 10 tys. wpisów p95 wyszukiwania ≤100 ms od ostatniego klawisza; brak blokowania GUI/audio |
| Stereo | Test izolacji L/R, sceny stereo i sumy mono; brak NaN/Inf oraz niekontrolowanego feedbacku |

Przy 48 kHz bufor 128 ramek obejmuje około 2,67 ms, a 256 — 5,33 ms.
To czas jednego bufora, nie całkowite opóźnienie. Wynik uwzględnia też kolejki,
algorytmy, limiter i urządzenie. Reakcję od fizycznego naciśnięcia klawisza
raportujemy osobno, jeśli zostanie zmierzona. Bluetooth jest trybem wygodnego
odsłuchu, poza kryteriami niskiego opóźnienia występu.

Jakość potwierdzają testy sygnałów oraz porównania odsłuchowe przy wyrównanej
głośności: transjenty, wokal, bas, szeroki miks i cisza. Badamy aliasing saturacji,
granice pętli, zmiany parametrów i artefakty key lock. Nie utożsamiamy przejścia
testów liczbowych z dobrym brzmieniem. Jeśli cel nie zostanie osiągnięty,
powstaje raport i korekta projektu; etapu nie oznaczamy jako zaliczony.

## 10. Granica obecnego opracowania

Powstają dokumenty, bez kodu silnika, instalowania zależności i generowania
gotowego banku audio. Kolejność przyszłych prac i scenariusze odbioru opisuje
[plan etapów](ETAPY.md). Przed implementacją etapu 0 należy ustalić rzeczywisty
MacBook i wersję macOS oraz dostępność przewodowego wyjścia i interfejsu cue.
Brak zewnętrznego interfejsu nie blokuje projektu z masterem stereo.
Wersja 1.0 wymaga też odbioru biblioteki, przejść między kolejnymi utworami,
muzycznych narzędzi i pracy imprezowej z [rozszerzenia LIVE](PRACA_NA_ZYWO.md).
