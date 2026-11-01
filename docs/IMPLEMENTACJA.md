# DJ EJAJ — stan implementacji

Wersja rozwojowa: **0.1.0**. Projekt docelowy znajduje się w
[PROJEKT.md](PROJEKT.md), a kryteria odbioru w [ETAPY.md](ETAPY.md).
Poniższy wykaz opisuje działający kod; nie oznacza zaliczenia wszystkich
bramek wydania imprezowego 1.0.

## Dostępne funkcje

- Natywna konsola egui/wgpu/Metal: logo **DJ EJAJ**, **by NT4.pl**, dwa
  decki, mikser, gałki z przypisaniami, waveform, rzeczywiste widmo FFT,
  biblioteka i zwijany sekwencer. Układ uwzględnia ekran 1280×800 punktów.
- CPAL/CoreAudio: wybór urządzenia, żądany bufor 128/256/512, stereo master
  i opcjonalny cue na kanałach 3–4 zgodnego interfejsu. Diagnostyka pokazuje
  także rzeczywistą wielkość callbacku oraz czas jego wykonania.
- Strumieniowy dekoder Symphonia (MP3, WAV, FLAC, AIFF, Ogg/Vorbis,
  AAC/M4A zależnie od kodeka), oddzielne wątki, ograniczone kolejki PCM,
  resampling sinc, varispeed i key lock Signalsmith Stretch.
- SQLite/FTS5: przyrostowy skan i anulowanie, wyszukiwanie, filtry BPM,
  tonacji, tagów i niezagranych; playlisty z kolejnością, kolejka setu,
  zachowanie metadanych i oznaczenie brakujących plików.
- Waveform, wstępna analiza BPM/tonacji z pewnością; ręczne BPM, tonacja,
  energia, tagi, downbeat, markery formy i początek/długość frazy.
  Podpowiedzi muzyczne pokazują powód dopasowania i uwzględniają varispeed.
- Ładowanie A/B bez autoplay, ochrona grającego decka w LIVE, start,
  pauza w przygotowaniu, zatrzymanie przytrzymaniem, seek, cztery hot cue,
  pętle, tap tempo i **TEMPO SYNC** do globalnego BPM.
- Gain, trójpasmowy EQ, fadery, crossfader equal-power/THRU, filtr LP/HP,
  saturacja, stereo delay/ping-pong, reverb, chorus/flanger i limiter.
  PANIC wygasza źródła oraz ogony efektów.
- Klawiatura: regulacja według czasu przytrzymania, precyzja z Shift,
  konteksty Granie/Biblioteka/Sekwencer, edytowalne pary klawiszy,
  wykrywanie konfliktów i ochrona przy pisaniu oraz zmianie decka/fokusu.
- Osiem oryginalnych brzmień z recepturami i eksportem WAV stereo 24-bit,
  demonstracyjne nagrania, import własnych sampli; one-shot/gate/loop,
  panorama, głośność, strój, choke hi-hatów i limit 16 głosów.
- Sekwencer: osiem ścieżek, 16/32/64 kroki, osiem wzorców na ścieżkę,
  32 nazwane sceny, mute, velocity/gate, swing, bas z akcentem/slide,
  dobór nut do skali i cofanie edycji kroków. Sceny i edycje aktywnego
  wzorca wchodzą na granicy taktu; oczekującą scenę można anulować.
- Sesja JSON z kopią poprzedniej i walidacją wersji: regulatory, decki
  w pauzie, sekwencer, sample, kolejka i profil klawiszy.
- Nagranie mastera WAV stereo 24-bit z ditheringiem na osobnym wątku,
  kontrola przepełnienia i błędów zapisu. Widok gotowości sprawdza audio,
  brakujące pliki setu, bank, analizę, cue i wolne miejsce.
- Analiza plików wstrzymuje się podczas grania w LIVE. Oryginalne pliki
  muzyczne są tylko odczytywane.

## Weryfikacja

`cargo test --workspace --locked` obejmuje 34 testy: izolację stereo,
stabilność DSP i granice indeksów modulacji, limiter, ochronę ładowania,
unieważnianie PCM po seek, PANIC, zapis częściowego bufora nagrania,
key lock tonu 440 Hz przy 1,2×, bibliotekę, Unicode/FTS, playlisty,
powtarzalność syntezy, sampler, sceny, sesje, konteksty klawiatury
domknięcie nagrania przy wyjściu oraz widoczność biblioteki na 1280×800. Formatowanie i Clippy są
sprawdzane osobno. Szczegóły aktualnego przebiegu: [raport](odbior/wersja-0.1.0.md).

Próba dużej biblioteki używa **50 tys. syntetycznych metadanych**,
nie 50 tys. rzeczywistych nagrań. Test allocatora obejmuje sprawdzany
własny tor renderujący, nie całe CoreAudio ani każdą ścieżkę programu.
Testy kodeka używają WAV; deklaracja pozostałych formatów wynika z włączonych
dekoderów i wymaga szerszego zestawu plików referencyjnych.
Zrzuty w `output/screenshots/` pochodzą z działającego okna;
`docs/wizualizacje/` zawiera wcześniejszą makietę projektową.

## Otwarty zakres i ograniczenia

TEMPO SYNC ustawia prędkość; **nie synchronizuje fazy beatów**. Pozostają:
kompensacja opóźnienia key lock, pełny beatgrid z powiększeniem, kwantyzacja
startu decków/hot cue/padów, prowadzenie fraz względem każdego utworu,
chwilowy pitch bend, płynne makra Przejście/Przestrzeń/Acid (obecnie są to presety) i docelowy routing
send/return z filtrami szyn. Parametry FX są przypisane do decków;
sampler trafia na master bez tych samych sendów i osobnego cue.
Aktualna regulacja parametrów sceny nie stanowi pełnego systemu automatyzacji.

Limiter ogranicza szczyty próbek; nie jest certyfikowanym limiterem true peak.
Analiza muzyczna jest przybliżona i dopuszcza korektę ręczną. Długie pliki
są odtwarzane strumieniowo; import pojedynczego sampla ma limit 2 mln ramek.

Odsłuch jakości FX, 30-minutowa/godzinna/dwugodzinna próba występu, fizyczny
pomiar opóźnienia, referencyjne 30 fragmentów tonalnych, próby nowych
użytkowników i osobny cue czterokanałowy pozostają niewykonane.
Próba callbacku na ciszy nie potwierdza jakości dźwięku ani gotowości na imprezę.

## Środowisko i pakiet

macOS 15.7.9, Intel Core i7-8569U 2,80 GHz, 16 GB RAM.
Apple Silicon wymaga osobnej kompilacji i weryfikacji.
`scripts/package_macos.py` tworzy `output/DJ EJAJ.app` z licencjami zależności,
także zagnieżdżonych bibliotek DSP. Pakiet nie jest podpisany ani notaryzowany.
Toolchain i zależności przypięto w `rust-toolchain.toml` i `Cargo.lock`.
Ostrzeżenie Cargo o przyszłej niezgodności `block 0.1.6` dotyczy zależności
Metal i nie blokuje obecnej kompilacji.
