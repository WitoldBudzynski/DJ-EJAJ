# DJ EJAJ 0.1.0 — raport weryfikacji lokalnej

Data: 30 października 2026. **Działająca wersja rozwojowa; formalny odbiór
wersji imprezowej 1.0 pozostaje otwarty.**

## Wersja i środowisko

- macOS 15.7.9, Intel Core i7-8569U 2,80 GHz, 16 GB RAM.
- Rust 1.99.1, zależności przypięte w Cargo.lock; profil release, Metal.
- Pakiet: `output/DJ EJAJ.app` (Intel x86_64, macOS 15+).
- SHA-256 pliku wykonywalnego: `8f2d6a8820c4bb9f7534b6483bbfb20d3a6ef1ef72af8786b52154276ec804cd`.
- Dane demonstracyjne i indeks próby odizolowano w `.local-data/smoke/`.

## Wyniki

| Sprawdzenie | Wynik |
| --- | --- |
| Budowanie release ze zablokowanymi zależnościami | zaliczone |
| Testy workspace | 34 zaliczone, 0 nieudanych, 0 pominiętych |
| rustfmt | zaliczone |
| Clippy, wszystkie moduły i cele, `-D warnings` | zaliczone |
| Uruchomienie natywnego pakietu, okna 1280×800 i żądane 1440×960 | zaliczone; oba procesy zakończone kodem 0 |
| Biblioteka przy rozwiniętym sekwencerze, 1280×800 | 5 widocznych wierszy w rzeczywistym oknie; test geometrii zaliczony |
| Biblioteka przy zwiniętym sekwencerze, 1280×800 | test geometrii: co najmniej 8 wierszy |
| Bank sampli | 8 oryginalnych plików stereo WAV, 48 kHz, 24-bit; manifest z recepturami |
| Pakiet i licencje | plik wykonywalny zgodny bajtowo z buildem; licencje zależności, w tym obu bibliotek Signalsmith |

W `output/screenshots/aplikacja-1280.png` i `aplikacja-1440.png` zapisano
zrzuty rzeczywistego renderowania aplikacji na ekranie Retina. Wymiary plików
w pikselach są dwukrotnie większe od rozmiaru okna w punktach. Większe
okno system ograniczył do 1440×944 punktów dostępnego obszaru ekranu. Decki są
załadowane i zatrzymane, więc mierniki i widmo pozostają bez sygnału.
Makieta w `docs/wizualizacje/` nie jest dowodem uruchomienia.

## Próba CoreAudio na ciszy

Źródło: [audio-60s.json](audio-60s.json).

- Urządzenie: SMSL D300; 44100 Hz, stereo.
- Żądany i faktyczny bufor: 256 ramek
  (min. 256, maks. 256).
- Czas: 60.02 s; callbacki: 10340.
- Przekroczenia budżetu: 0; błędy strumienia: brak.
- Maksymalny zmierzony czas callbacku: 0.396 ms.
- Górna granica przedziału histogramu p99: 6.25% czasu bufora.

Pomiar dotyczy callbacku na **ciszy**. Nie mierzy fizycznego opóźnienia,
pełnego obciążenia key lock/dekodowaniem ani jakości brzmienia. Czas bufora
5.80 ms nie jest deklaracją opóźnienia całej aplikacji.

## Granice automatycznych dowodów

Test dużej biblioteki obejmuje 50 tys. syntetycznych wpisów w SQLite/FTS,
nie skan 50 tys. realnych plików i nie pełny pomiar od klawiatury do obrazu.
Test key lock wykorzystuje sinusoidę 440 Hz przy 1,2×; nie zastępuje
odsłuchu muzyki ani pomiaru opóźnienia. WAV jest formatem referencyjnym testów
dekodera; inne włączone kodeki wymagają reprezentatywnego korpusu nagrań.
Test allocatora sprawdza własny tor DSP, bez deklaracji objęcia całego macOS.

Nie wykonano pełnych prób 30 min / 1 h / 2 h, testów z trzema nowymi
użytkownikami, odsłuchu porównawczego FX, pomiaru przewodowego, odbioru
czterokanałowego cue ani prób Apple Silicon. Pakiet jest lokalny,
niepodpisany i nienotaryzowany. Otwarty zakres funkcjonalny jest jawnie
opisany w [stanie implementacji](../IMPLEMENTACJA.md).
