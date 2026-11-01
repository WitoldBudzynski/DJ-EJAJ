# DJejaj — praca na żywo i wiedza muzyczna

Status: rozszerzenie projektu, bez implementacji. Wymagania uzupełniają
[projekt główny](PROJEKT.md) oraz [kryteria etapów](ETAPY.md).

## 1. Konsola do imprez elektronicznych

Główne zadania to znalezienie następnego utworu, przygotowanie go poza masterem,
przejście w odpowiedniej frazie i utrzymanie ciągłości setu. Konsola zachowuje
symetrię dwóch decków, fizyczne skojarzenia gałek i faderów oraz stałe położenie
transportów. Biblioteka jest równorzędną częścią instrumentu.

Trzy konteksty obsługi mają jawny fokus: **Granie**, **Biblioteka**, **Sekwencer**.
Nie zmieniają trwającego audio. Pasek kontekstu wyświetla aktualne skróty;
wybór utworu, wybór decka i utwór faktycznie grający to odrębne stany.
Zaawansowane ustawienia otwierają się przy wybranym module, bez zasłaniania
mastera i ostrzeżeń. Podpowiedzi rozwijają krótkie nazwy, np. „send — ilość
sygnału wysyłanego do efektu”.

W ciemnym pomieszczeniu najważniejsze są czytelne wartości, wystarczający
kontrast, umiarkowana jasność i brak migających dekoracji. Widoczne są: deck
na masterze, czas do końca, BPM, pozycja we frazie, poziom mastera i stan REC.
Przekroczenie poziomu, brak pliku i spóźniona akcja mają tekstowy opis.
Alerty nie przesuwają listy ani regulatorów i nie przejmują fokusu podczas grania.

## 2. Cała biblioteka pod ręką

### Widoki, kolumny i selekcja

Domyślny widok „Wszystkie utwory” obejmuje całą kolekcję, z liczbą wyników,
przewijaniem i możliwością natychmiastowego wyczyszczenia filtrów. Cała kolekcja
oznacza dostęp do każdego wpisu, a nie próbę wyświetlenia tysięcy wierszy naraz.
Biblioteka pozostaje widoczna w trakcie miksu i obsługi sekwencera.

- Lewy panel: Wszystkie, foldery, playlisty, Ulubione, Do przygotowania.
- Środek: status A/B, wykonawca i tytuł, BPM, tonacja, energia, długość;
  dodatkowe kolumny są opcjonalne i można zmieniać ich szerokość.
- Prawy panel: kolejka „Następne” i historia z czasem zagrania.
- Filtry: tekst, zakres BPM, tonacja, tagi, energia, niezagrane i dostępne pliki.
- Oznaczenia: GRA A/B, ZAŁADOWANY, ODSŁUCH, ZAGRANY, BRAK PLIKU, NIEPRZEANALIZOWANY.

Zmiana zaznaczenia nie zmienia audio. Powrót z decka przywraca zaznaczenie,
przewinięcie, sortowanie i filtry. Zakończona analiza nie przestawia automatycznie
wiersza pod palcem: aktualizujemy dane, a nowe sortowanie stosujemy jawnie lub
po zakończeniu nawigacji. Zaznaczenie jest związane z identyfikatorem utworu,
nie numerem wiersza. Filtry mają widoczne etykiety, np. „BPM 136–142”.

### Wybór i odsłuch

`Cmd+L` przenosi fokus do listy, `Cmd+F` do wyszukiwania, `↑/↓` wybiera utwór,
`Page Up/Down` przewija stronami. `Enter` otwiera działania: „Dodaj do Następne”,
„Odsłuchaj”, „Załaduj A”, „Załaduj B”, „Przygotuj”. Dla ładowania istnieją
bezpośrednie skróty `Cmd+1/2`. Wszystkie działania są dostępne bez myszy.

Kolejkę można zmieniać klawiaturą bez przesuwania aktualnie grających utworów.
Nie ma domyślnego automatycznego ładowania ani odtwarzania po zakończeniu utworu.
Ostrzeżenie o końcu działa np. 32 i 8 taktów przed końcem przy poprawnym gridzie;
gdy gridu brak, pokazujemy czas w sekundach, bez pozornej dokładności fraz.

Odsłuch biblioteki ma osobny transport i głośność, trafia wyłącznie do cue.
W konfiguracji z samym masterem stereo jest niedostępny podczas LIVE;
przeglądanie pliku nie może niespodziewanie zabrzmieć na sali. Przygotowanie
muzyki na jednym wyjściu jest możliwe w osobnym trybie poza występem.

### Wyjaśniane sugestie kolejnego utworu

Widok „Pasujące do A/B” proponuje kolejność na podstawie zadanego zakresu
tempa, zgodności tonacji, energii, tagów i historii. Pokazuje przyczynę, np.
„+2 BPM · ta sama tonacja · niezagrany · energia 3→4”. Użytkownik wybiera
priorytety i zawsze może wrócić do pełnej listy. Brak analizy nie usuwa utworu
z kolekcji; nieznane cechy nie są traktowane jako potwierdzone dopasowanie.

Zmiana prędkości bez key lock zmienia wysokość: sugestie uwzględniają
aktualne przestrojenie, a przy niejednoznaczności pokazują „sprawdź odsłuch”.
„Energia” jest oceną muzyczną użytkownika; BPM, głośność i energia nie są
tym samym. Algorytm nie decyduje o jakości przejścia za DJ-a.

## 3. Przełączanie utworów bez przerwania setu

Podstawowy cykl to A → B → kolejny utwór na zwolnionym A, a nie podmiana
pliku we wciąż grającym odtwarzaczu. Dwa decki wystarczają do tego cyklu;
banki wzorców nie oznaczają kolejnych stale przetwarzanych decków.

1. Podczas gry A wybierz kandydata i dodaj go do „Następne”.
2. Załaduj go jawnie do zatrzymanego B. Otrzymuje stan „Przygotowywanie”,
   potem „Gotowy”; ładowanie nie włącza transportu i nie zmienia faderów.
3. Odsłuchaj przez cue, sprawdź tempo, poziom, downbeat i punkt wejścia.
   Bez cue przygotuj go wcześniej; aplikacja nie udaje niezależnego odsłuchu.
4. Ustaw B poza masterem za pomocą fadera lub przypisania crossfadera,
   uruchom go na wybranym takcie/frazie i przeprowadź przejście.
5. Zamknij kanał A, świadomie zatrzymaj deck i dopiero wtedy załaduj następny.

**LIVE jest domyślny.** Każdy grający deck jest chroniony przed podmianą,
również przy zamkniętym faderze. Próba ładowania pokazuje „Deck A gra — wybierz B
lub zatrzymaj A”; nie otwiera okna potwierdzenia nad konsolą i nie zatrzymuje audio.
`Spacja` nie pauzuje już grającego decka; zatrzymanie wymaga przytrzymania
`Shift+Spacja` przez 700 ms w kontekście grania. PANIC pozostaje natychmiastową
komendą wyciszenia. Tryb przygotowania dopuszcza zwykłe play/pause i jest
wyraźnie oznaczony; jego włączenie w LIVE wymaga świadomej akcji.

Ładowanie jest transakcją: stary utwór pozostaje w zatrzymanym decku, dopóki
nowy nie zostanie poprawnie otwarty i wstępnie zbuforowany. Błąd lub anulowanie
nie zeruje ustawień starego utworu. Ponowne ładowanie unieważnia poprzednie
zadanie; spóźniony wynik nie nadpisuje nowszego wyboru. Podmiana i start są
oddzielnymi akcjami, a podwójne naciśnięcie nie wywołuje podwójnego startu.

Historia oznacza „zagrany” dopiero po 30 sekundach skumulowanego czasu gry
ze źródłowym sygnałem powyżej −60 dBFS i skutecznym wzmocnieniem toru do mastera
powyżej −40 dB. Progi są ustawieniami, a cue się nie liczy. To heurystyka
historii, nie pomiar słyszalności na sali; wpis można poprawić ręcznie.

## 4. Wiedza muzyczna zamieniona w narzędzia

### Beat, takt, fraza i forma

Profil startowy przyjmuje metrum 4/4 i ćwierćnutowy puls. Takt zawiera cztery
beaty, a 16 kroków szesnastkowych wypełnia jeden takt. Początek taktu
(downbeat) i początek frazy to osobne informacje. Przy 140 BPM osiem taktów
trwa około 13,7 s, 16 około 27,4 s, a 32 około 54,9 s.

DJ ustawia początek frazy i wybiera jej długość 8/16/32 taktów; to wygodne
szablony pracy, nie uniwersalne prawo gatunku. Licznik pokazuje np.
„takt 13/16 · do wejścia 3 takty”. Markery Intro/Break/Build/Drop/Outro
ułatwiają decyzję; w 1.0 są ręczne. Automatyczne rozpoznawanie struktury
utworu jest późniejszym rozszerzeniem. Beatgrid nie dowodzi poprawnej frazy.

Przykładowe sposoby pracy:

- **Techno:** długie nakładanie perkusyjnego intro/outro, zmiany co 16/32 takty,
  stopniowe otwieranie filtrów i świadome przekazanie basu.
- **Trance:** unikanie przypadkowego nakładania dwóch dominujących melodii,
  dopasowanie harmoniczne i zaplanowane wejścia po breakach.
- **Rave/acid:** akcentowany riff basowy, slide, krótsze wariacje, rytmiczne
  wyciszenia i powrót pełnego rytmu w wybranej frazie.

To propozycje presetów i ćwiczeń, nie ograniczenia użytkownika. Szablony tempa
można zapisać jako własne zakresy; aplikacja nie przypisuje gatunku wyłącznie z BPM.

### Harmonia, bas i przestrzeń

Tonacja jest pokazywana nazwą muzyczną, opcjonalnie kodem koła harmonicznego.
Punktem wyjścia są ta sama tonacja, relacja dominantowa/subdominantowa i tonacja
paralelna; wynik zależy od materiału i wymaga odsłuchu. Koło harmoniczne pomaga
w orientacji, lecz nie gwarantuje zgodności każdej melodii.
Taką rolę praktycznej wskazówki opisuje też
[Native Instruments](https://blog.native-instruments.com/harmonic-mixing-rules-and-how-to-break-them/).
W przypadku perkusyjnych lub niejednoznacznych fragmentów pokazujemy brak
pewnej tonacji zamiast wymuszonej etykiety.

EQ LOW i osobne poziomy kick/bass samplera pozwalają uniknąć kumulacji dołu
dwóch pełnych miksów. Początkowy preset przejścia sugeruje „jeden dominujący
bas”, ale nie wykonuje automatycznego kill. Filtr na wejściu pogłosu i delayu
ogranicza rozmywanie niskich częstotliwości. Reverb, stereo i saturację oceniamy
również w mono, przy wyrównanej głośności. Szerokość nie zastępuje czytelnego rytmu.

Makra startowe „Przejście”, „Przestrzeń” i „Acid” łączą ograniczony zestaw
parametrów FX z widocznymi zakresami; nie sterują master gain ani transportami.
Przejęcie gałki po makrze zaczyna od aktualnej wartości. Żaden preset ani scena
nie podnosi automatycznie głośności mastera.

## 5. Sekwencer jako instrument na żywo

### Model i wzorce początkowe

Osiem ścieżek korzysta z sampli lub monofonicznego instrumentu basowego;
początkowy zestaw to kick, clap/snare, closed hat, open hat, percussion,
bass, stab i FX. Własny bank zawiera też riser i impact do podmiany slotów.
Jedna ścieżka odtwarza jeden wzorzec naraz; wszystkie głosy, w tym bas i pady,
mieszczą się we wspólnym limicie 16. Open/closed hat może współdzielić grupę
choke, aby zamknięty hi-hat wygaszał otwarty.

Siatka startowa to 16 kroków na takt; długości 32 i 64 obejmują dwa i cztery
takty. Stałe grupy czterech kroków pokazują beaty, a strona 1–4 i playhead
pozwalają odnaleźć się w dłuższym wzorcu. Różne podziały i polimetria pozostają
rozszerzeniami po 1.0. Tempo transportu jest regulowane w zakresie 60–220 BPM;
nie jest to deklaracja typowych temp konkretnych gatunków.

| Szablon startowy | Kroki numerowane od 1 | Co ćwiczy |
| --- | --- | --- |
| Four-on-the-floor | Kick 1, 5, 9, 13 | Stały puls ćwierćnutowy |
| Backbeat | Clap/snare 5, 13 | Akcent drugiego i czwartego beatu |
| Offbeat hat | Open hat 3, 7, 11, 15 | Puls pomiędzy uderzeniami kicka |
| Rolling bass | Krótkie nuty między kickami, edytowalny przykład | Miejsce dla stopy i kontrola gate |
| Acid | Prosty bas z ręcznymi akcentami i slide | Ruch frazy przez artykulację i filtr |

Połączenie regularnej stopy z backbeatem jest opisane w
[Ableton Learning Music](https://learningmusic.ableton.com/make-beats/backbeats.html).
Akcenty i slide jako środki pracy nad acidowym basem pokazuje
[Roland](https://articles.roland.com/mastering-the-tb-303-sequencer-in-roland-cloud/).
Tworzymy własne wzorce i syntezę; nie kopiujemy rozpoznawalnych melodii ani
nie deklarujemy emulacji TB-303. Szablony otwierają się do edycji i odsłuchu cue.

### Edycja i wykonanie

Krok ma stan on/off, velocity, gate, a w basie także nutę, oktawę, akcent i slide.
Nuty można ograniczyć do wybranej skali; nie przestraja to całych utworów.
Slide łączy aktywne nuty monofonicznego basu, a nie rozciąga pliku perkusyjnego.
Swing opóźnia wybrane szesnastki w parze, zachowując czas całego taktu; domyślnie
wynosi zero i można wyłączyć go dla kicka. Klawiatura laptopa nie ma velocity,
więc dynamikę ustawia się jawnie w edytorze lub jako poziom akcentu.

W kontekście sekwencera `↑/↓` wybiera ścieżkę, `←/→` krok, `Enter` przełącza
krok, `[`/`]` zmienia stronę; panel parametrów kroku jest dostępny przez `Tab`.
Mute, odsłuch cue i wybór wzorca mają widoczne, konfigurowalne skróty.
Wyszukiwanie i wprowadzanie nut przechwytuje tekst, więc nie wyzwala padów.
Nagrywanie gry na padach wymaga jawnego uzbrojenia; domyślnie nie nadpisuje wzorca.
UNDO cofa ostatnią edycję sekwencji, bez cofania odtwarzania lub ładowania decków.

Każda ścieżka ma do ośmiu wzorców, a sesja do 32 scen. Scena zawiera referencje
do wzorców i stany mute ośmiu ścieżek. Nie zawiera mastera, transportów decków
ani wymiany instrumentów. To pozwala budować części Intro/Groove/Break/Build/Drop
bez kosztu uruchamiania wielu silników. Puste miejsce sceny oznacza „zachowaj”,
a zatrzymanie ścieżki jest osobnym, wyraźnym stanem.

Oddzielamy **edytowany wzorzec**, **grający wzorzec** i **scenę oczekującą**.
Edycje grającego wzorca są stosowane od kolejnego taktu, przez przygotowaną
wersję danych. Solo dla przygotowania działa wyłącznie na cue; wyciszenie
pozostałych ścieżek mastera wymaga jawnej komendy, nie skrótu odsłuchu.

Uruchamianie wzorca, sceny i mute może być natychmiastowe albo kwantyzowane
do beatu, taktu lub frazy. Domyślnie scena czeka na takt; licznik wskazuje
moment wykonania. Ponowna komenda zastępuje poprzednią oczekującą dla tego
samego celu; anulowanie dotyczy wybranej akcji i nie zatrzymuje innych źródeł.
Rozróżnienie wybranej i uruchomionej sceny oraz kwantyzacja są sprawdzonymi
rozwiązaniami pracy na żywo, opisanymi w
[Session View](https://www.ableton.com/en/manual/session-view/) i
[Launching Clips](https://www.ableton.com/en/manual/launching-clips/).

Zegar działa dalej podczas ładowania i zatrzymywania decka. Gdy deck będący
źródłem BPM się kończy, zegar utrzymuje ostatnie tempo; nie resetuje sceny.
Zmiana źródła zegara nie przestawia fazy grającego sekwencera. Niegotowa lub
spóźniona akcja nie jest wykonywana w przypadkowym momencie: pozostaje stary
wzorzec, a komunikat wskazuje odrzucenie. Zaplanowana zmiana nie oznacza
dodatkowego opóźnienia reakcji dźwięku — celowo czeka na granicę muzyczną.

## 6. Duże możliwości przy ograniczonym koszcie

Bogactwo wynika z kombinacji brzmień, wzorców, scen i parametrów, a nie
nieograniczonej liczby jednoczesnych procesorów. W 1.0 obowiązują dwa decki,
jeden krótkotrwały odtwarzacz preview cue, osiem ścieżek, 16 głosów, osiem
wzorców na ścieżkę i 32 sceny. Dodatkowe ratchety, probability, polimetria,
pełna automatyzacja kroków, rozbudowany syntezator i automatyczne przejścia
między utworami są rozszerzeniami po 1.0.

- Lista biblioteki renderuje tylko widoczne wiersze; filtrowanie i sortowanie
  korzystają z indeksów SQLite poza GUI i audio. Nowe zapytanie unieważnia
  wynik starego. Nie ładujemy całej kolekcji jako PCM.
- Przygotowany jest tylko następny kandydat; jego zadanie można anulować.
  Preview i przygotowanie mają niższy priorytet niż bufory grających decków.
- Łączny cache PCM ma początkowy limit 256 MiB, obejmujący okna decków,
  preview i sample. Zbyt duży sample wymaga skrócenia lub wyraźnego odrzucenia;
  nie wypiera danych potrzebnych grającemu silnikowi. Odczyt jest strumieniowy.
- Waveform używa gotowych poziomów szczegółowości, widmo ma ograniczoną
  częstotliwość odświeżania. Wizualizacje redukujemy przed funkcjami audio.
- Zdarzenia sekwencera i dane scen są przygotowywane w ograniczonych buforach.
  Callback nie kompiluje wzorca, nie analizuje tonacji i nie otwiera sampli.
  Wymiana gotowej wersji wzorca nie zwalnia starej pamięci w callbacku;
  odzyskiwanie zasobów odbywa się poza wątkiem audio.
- Czasy pracy i wypełnienie kolejek są monitorowane. Przy obciążeniu najpierw
  wstrzymujemy analizę i preload, potem ograniczamy animacje. Nie zmieniamy
  po cichu tempa, efektów, liczby aktywnych głosów ani rozmiaru bufora urządzenia.

Przed występem widok „Gotowość setu” sprawdza dostępność plików, przygotowanie
kolejki, poprawki gridu, wyjście audio, wolne miejsce na nagranie i tryb LIVE.
Braki analizy są wskazówkami; brak pliku blokuje wyłącznie jego załadowanie.
Można przygotować lokalne kopie plików setu w jawnie wybranym folderze,
bez modyfikowania oryginałów. Tryb LIVE ogranicza zmiany urządzenia i bufora
do świadomej operacji po wyciszeniu, a kosztowną analizę domyślnie wstrzymuje.

## 7. Jak potwierdzimy wygodę i wydajność

Cele do sprawdzenia na sprzęcie odniesienia, poza dotychczasowymi limitami DSP:

- Przy 10 tys. wpisów: p95 wyników wyszukiwania ≤100 ms od ostatniego klawisza,
  wraz z ewentualnym debounce; zapisujemy też czas zimnego pierwszego zapytania.
- Trzy osoby bez doświadczenia z DJejaj, po najwyżej 10 minutach instruktażu,
  każda wybiera kolejny utwór i zleca ładowanie do wolnego decka w ≤15 sekund,
  bez myszy i bez przypadkowej ingerencji w grający deck. Mierzymy czas wyboru
  i polecenia, a czas przygotowania pliku raportujemy osobno.
- Dla lokalnego przygotowanego MP3/WAV/FLAC: cel p95 „Załaduj” → „Gotowy” ≤2 s,
  z osobnym pomiarem zimnego cache; dyski zewnętrzne mają osobny wynik.
- Test dodatkowy z 50 tys. wpisów potwierdza poprawność filtrowania,
  wirtualizacji i stałość limitu cache; raportujemy wydajność bez udawania
  zaliczenia celu dla 10 tys., jeśli większy zbiór działa wolniej.
- Co najmniej dwugodzinny set: 20 zmian utworów, ciągła praca sekwencera,
  zmiany scen i wyszukiwanie, nagrywanie i aktywne FX; zero przypadkowych
  podmian, niedoborów oraz przekroczeń terminu callbacku. Co najmniej raz
  dodajemy skan w tle jako próbę przeciążenia, mimo domyślnego jego wstrzymania.

Próba intuicyjności jest bramką projektu, nie stwierdzeniem, że interfejs już
jest wygodny. Potknięcia użytkowników prowadzą do poprawy układu i skrótów.
Wyniki, konfigurację i nagrania dokumentujemy w raportach odbioru etapów.
