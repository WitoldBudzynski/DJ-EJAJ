# DJ EJAJ — pierwszy miks

To lokalna wersja rozwojowa 0.1.0 na macOS 15+ (pakiet Intel).
Pełny zakres i otwarte kryteria odbioru opisuje [stan implementacji](IMPLEMENTACJA.md).

## Otwórz konsolę

Rozpakuj `output/DJ-EJAJ-0.1.0-macOS-Intel.zip` i otwórz **DJ EJAJ.app**.
Możesz też uruchomić pakiet bezpośrednio z `output/`.
Aplikacja startuje bez odtwarzania. W **Ustawieniach** wybierz wyjście audio;
na początek pozostaw bufor 256. Osobny odsłuch słuchawkowy wymaga wyjścia
czterokanałowego; zwykłe wyjście stereo obsługuje master.

## Wybierz muzykę

1. Kliknij **＋ Katalog** i wskaż folder z utworami. Poczekaj na indeksowanie.
2. Wyszukaj utwór, zaznacz go i naciśnij **⌘1**, aby załadować deck A.
   **⌘2** ładuje deck B. Samo ładowanie nie uruchamia muzyki.
3. **Przygotuj / analiza** otwiera BPM, tonację, tagi, cue i markery.
   Wyniki automatyczne można poprawić ręcznie; brak analizy nie blokuje grania.
4. **＋ Następne** buduje kolejkę setu. Playlisty są dostępne w lewym panelu
   biblioteki; na niższym ekranie przewiń ten panel w dół.

## Zagraj przejście

- **⌘J** wybiera granie, **1 / 2** aktywny deck, **Spacja** uruchamia go.
- W trybie LIVE zatrzymanie wymaga **Shift+Spacja przez 700 ms** lub
  przytrzymania ekranowego **STOP**. Grającego decka nie można podmienić.
- Pod gałkami widać pary klawiszy. **Q/A** steruje filtrem, **W/S** delay,
  **E/D** pogłosem, **R/F** saturacją. Przytrzymanie zmienia parametr płynnie;
  **Shift** daje precyzyjną regulację. Gałki dotyczą aktywnego decka.
- **TEMPO SYNC** dopasowuje tempo do zegara. Początek beatu ustaw ręcznie;
  obecna wersja nie synchronizuje fazy. Key lock zachowuje wysokość tonu.
- Mikser ma gain, EQ, fadery i crossfader. **THRU** pozwala prowadzić
  przejście samymi faderami.
- **⌘.** uruchamia PANIC i wycisza źródła wraz z ogonami efektów.

## Dołóż rytm i zapisz set

**⌘K** wybiera sekwencer, a Spacja uruchamia/zatrzymuje rytm. Strzałki
wybierają ścieżkę i krok, Enter go przełącza, `[ / ]` zmienia stronę.
Sceny przechodzą na granicy taktu; oczekującą zmianę można anulować.
**?** otwiera edytor kroków, skali i samplera. Pady: **Z X C V B N M ,**.

W lewym panelu wybierz **Nagrywaj WAV** i nowy plik. Zatrzymaj nagranie
przyciskiem **Stop REC**. **⌘S** zapisuje sesję; utwory wracają w pauzie.
Oryginalne pliki muzyczne pozostają bez zmian.

Własne brzmienia demonstracyjne można przygotować z repozytorium poleceniem
`sh scripts/run.sh --demo`. Skrypt najpierw buduje aktualną wersję aplikacji.
