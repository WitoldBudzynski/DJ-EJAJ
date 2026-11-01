# DJejaj — etapy realizacji i odbiór

Status: **implementacja rozwojowa 0.1.0 w toku; formalny odbiór etapów otwarty**.
Kod obejmuje moduły kilku etapów, lecz ich pełne bramki jakościowe i sprzętowe
nie są przez to automatycznie zaliczone. Bieżący zakres i ograniczenia opisuje
[stan implementacji](IMPLEMENTACJA.md). Poniżej pozostają docelowe kryteria odbioru.

## Zasady prowadzenia prac

Każdy etap kończy się demonstracją, raportem pomiarów i wynikiem: zaliczony,
niezaliczony albo zablokowany z podaniem przyczyny. Następny zależny etap
rozpoczynamy po spełnieniu kryteriów poprzedniego. Brak sprzętu do odsłuchu
nie zastępuje testu fizycznego testem programowym.

Raport `docs/odbior/etap-N.md` powstaje przy wykonaniu etapu i zawiera:
wersję kodu, model MacBooka, RAM, macOS, urządzenie audio, sample rate,
faktyczny bufor, scenariusz, oczekiwany i uzyskany wynik, błędy oraz załączniki.
Testy wydajności używają kompilacji release. Czasy callbacku zbieramy bez
logowania w jego wątku, a zestawiamy po pomiarze.

Nie ustalamy jeszcze dat realizacji. Największe niewiadome — key lock,
opóźnienie, wygląd niestandardowych kontrolek i routing urządzeń — sprawdzamy
wcześnie, przed rozbudową funkcji.

Każdy etap uwzględnia [wymagania pracy na żywo](PRACA_NA_ZYWO.md). Priorytet
to sety techno/trance/rave: stała biblioteka, szybka zmiana utworów i ciągłość
audio. Badanie wygody nie ogranicza się do oceny wyglądu konsoli.

## Etap 0 — potwierdzenie podstaw technicznych

**Cel:** sprawdzić, czy proponowana architektura rokuje na docelowym MacBooku.

Zakres przyszłego eksperymentu:

- Spisać sprzęt odniesienia, macOS i tryby dostępnego wyjścia przewodowego.
- Odtworzyć stereo przez CPAL/CoreAudio, porównać bufory 128, 256 i 512 ramek.
- Zdekodować próbki MP3 CBR/VBR, WAV i FLAC; sprawdzić seek i 44,1/48 kHz.
- Porównać zwykłą zmianę prędkości z kandydatem key lock, w tym 0,8/1,0/1,2×.
- Pokazać jedną docelowo stylizowaną gałkę, klawisze i waveform w egui.
- Sprawdzić układ decków ze stale widoczną biblioteką na 1280×800 punktów,
  z przykładowym indeksem 10 tys. wpisów i nawigacją bez myszy.
- Przypiąć wersje zależności, toolchain, minimalny macOS i zapisać decyzje.

**Odbiór:** 30 minut ciągłego odtwarzania bez niedoborów i przekroczeń terminu
callbacku w wybranym trybie; pomiar pełnej ścieżki komenda–wyjście dla obu
trybów tempa. Użytkownik ocenia kierunek wizualny oraz przykłady brzmienia.
Lista pozostaje czytelna obok transportów; edytor sekwencera ma zaplanowane
miejsce bez całkowitego zasłonięcia biblioteki. Próbę dużego indeksu wykonujemy
z syntetycznymi metadanymi, bez pozorowania testu dekodowania 10 tys. plików.
Powstaje decyzja o bibliotece key lock i jej miejscu w architekturze.
Nieudany pomiar prowadzi do zmiany rozwiązania i ponownej próby.

## Etap 1 — fundament silnika stereo

**Zależność:** etap 0.

Workspace Rust, oddzielenie GUI od audio, zegar próbek, ograniczone kolejki,
wybór urządzenia, mierniki, master gain i mechanizm PANIC. Na początku źródłem
jest sygnał testowy lub plik WAV. Powstają wykonywalne polecenia budowania,
testów i profilowania opisane w README.

**Odbiór:** lewy i prawy kanał odtwarzają niezależne sygnały; godzina stabilnej
pracy w wybranym trybie. Sztuczne zatrzymanie aktualizacji GUI na 5 sekund
nie przerywa dźwięku. Test instrumentowany wykazuje brak alokacji i blokowania
w callbacku. PANIC wygasza całą sumę w ciągu 10 ms od przyjęcia przez silnik.
Odłączenie urządzenia prowadzi do czytelnego, bezpiecznie wyciszonego stanu.

## Etap 2 — biblioteka i jeden pełny deck

**Zależność:** etap 1.

Import folderu, SQLite, metadane, skan przyrostowy, anulowanie, wyszukiwanie,
dekodowanie w tle i waveform. Odtwarzanie MP3/WAV/FLAC, play/pause, seek i cue.
Obsługa plików brakujących, uszkodzonych, nazw z polskimi znakami i odłączonego
dysku. Dopiero po poprawnym przejściu bazowych kodeków dołączamy dodatkowe.
Wirtualizowana lista całej kolekcji, trwała selekcja, playlisty, kolejka
„Następne”, tagi i filtry, początkowo z ręcznymi wartościami muzycznymi.
Osobny preview cue, dostępny wyłącznie przy właściwym routingu urządzenia.

**Odbiór:** indeks 10 tys. plików można przerwać i wznowić; ponowny skan nie
tworzy duplikatów i nie analizuje ponownie niezmienionych utworów. Uszkodzony
plik nie zatrzymuje skanu. Utwór gra podczas indeksowania. Testy obejmują
MP3 CBR/VBR, mono/stereo, 44,1/48 kHz, seek blisko końca oraz brak dostępu
do katalogu. Ponowne uruchomienie zachowuje bibliotekę i wskazane cue.
Przy 10 tys. wpisów p95 wyszukiwania spełnia cel ≤100 ms od ostatniego klawisza;
raport obejmuje zimny cache. Zapytania „t” → „tr” → „trance” nie pozwalają
spóźnionym wynikom nadpisać nowego filtra. Aktualizacja metadanych nie gubi
zaznaczenia. Próba dodatkowa 50 tys. wpisów sprawdza poprawność i zużycie pamięci.
Preview nie trafia na master; bez osobnego cue jest blokowany w LIVE.

## Etap 3 — dwa decki, mikser i konsola z klawiatury

**Zależność:** etap 2.

Dwa niezależne decki, gain, EQ, fadery, crossfader o krzywej equal-power,
varispeed, widoczne przypisania i edytor klawiszy. Nawigacja obejmuje również
wybór katalogu, bibliotekę, załadowanie utworu oraz ustawienia. Konsola otrzymuje
docelowy ciemny wygląd; waveform i mierniki bazują na danych silnika.
Tryb LIVE chroni każdy grający deck przed podmianą i przypadkową pauzą;
ładowanie jest odwoływalne i transakcyjne. Dostępne są THRU, konteksty
Granie/Biblioteka/Sekwencer, oznaczenia A/B i historia rzeczywiście zagranych
utworów. Historia cue i samego ładowania pozostaje odrębna.

**Odbiór:** piętnastominutowy miks z co najmniej ośmioma zmianami utworów
wykonujemy bez myszy, naprzemiennie ładując wolny deck.
`Q/A` płynnie zmienia gałkę i jej wskazanie; przytrzymanie działa niezależnie
od ustawień powtarzania macOS. Sprawdzamy zapis profilu, konflikt skrótów,
dwa kierunki jednocześnie, zmianę decka przy wciśniętym klawiszu i utratę fokusu.
Wyszukiwarka przyjmuje Spację bez uruchamiania decka. Na 1280×800 punktów
nie znikają nazwy, wartości ani klawisze. Crossfader poprawnie izoluje kanały
na krańcach; środek badamy także na skorelowanych sygnałach i kontrolujemy poziom.
THRU pozwala prowadzić przejście faderami bez wpływu crossfadera.
Próby podmiany grającego decka, zwykłej Spacji w LIVE, błędu ładowania,
dwóch równoczesnych zleceń i zakończenia utworu nie naruszają stanu innego
decka. Nie ma niezamierzonego autoplay; brak pliku nie zeruje poprzedniego.
Przy lokalnych przygotowanych plikach mierzymy p95 ładowania do stanu „Gotowy”
z celem ≤2 s; ciepły i zimny cache oraz dysk zewnętrzny raportujemy oddzielnie.
W standardowym widoku 1280×800 widać minimum osiem wierszy biblioteki,
a po rozwinięciu sekwencera minimum cztery. Trzy osoby po krótkim instruktażu
przechodzą próbę wyboru i ładowania kolejnego utworu w ≤15 s, opisaną w LIVE.
Zmieniamy układ lub skróty, jeśli próba wygody się nie powiedzie.

## Etap 4 — podstawowe efekty o dopracowanym brzmieniu

**Zależność:** etap 3.

Filtr LP/HP, delay stereo/ping-pong, reverb stereo i saturacja z presetami
analogowego charakteru. Sendy i powroty według projektu, wygładzanie parametrów,
przejścia bypass i limiter mastera. Regulacja efektów jest dostępna z klawiatury.
Filtry wejściowe szyn przestrzennych i makra „Przejście”/„Przestrzeń” nie
zmieniają master gain ani transportu. Oceniamy zwłaszcza przekazanie basu
między utworami i długie przejścia perkusyjne.

**Odbiór:** testy impulsu, sinusoidy, szumu, ciszy i odseparowanych kanałów;
brak NaN/Inf, nieograniczonego feedbacku oraz trzasków przy zmianach i bypass.
Powroty wybrzmiewają po zamknięciu źródła, a PANIC wygasza również ogony.
Odsłuch A/B przy wyrównanych poziomach obejmuje perkusję, wokal i pełny miks.
Godzinna próba dwóch decków ze wszystkimi podstawowymi FX przechodzi cele
stabilności i zapasu DSP z projektu.

**Rezultat etapów 0–4:** pierwszy użyteczny prototyp DJ-ski. Pozwala miksować
utwory i kreatywnie zmieniać ich brzmienie. Funkcje tworzenia z własnego banku
sampli są przedmiotem kolejnych etapów.

## Etap 5 — dopasowanie tempa i struktury utworów

**Zależność:** etap 4 oraz pozytywna decyzja o key lock z etapu 0.

Key lock, chwilowy pitch bend, tap tempo, analiza BPM, ręczna korekta beatgridu,
SYNC, cztery hot cue na deck, pętle i kwantyzacja. Wstępne buforowanie,
przewijanie i zmiana trybu uwzględniają opóźnienie algorytmu. Słaba pewność
analizy jest widoczna; automatyczna analiza nie jest warunkiem odtworzenia pliku.
Downbeat i początek frazy są oddzielnymi punktami. Dochodzą ręczne markery
Intro/Break/Build/Drop/Outro, licznik fraz 8/16/32 taktów, detekcja tonacji
z pewnością i korektą oraz wyjaśniane sugestie kolejnego utworu.

**Odbiór:** fixture o znanym stałym BPM i poprawnym gridzie utrzymuje odchylenie
fazowe decków poniżej 10 ms przez 10 minut. Wyniku nie przenosimy na utwory
ze zmiennym tempem. Dla 0,8/1,0/1,2× key lock sprawdzamy wysokość tonu testowego
(cel odchylenia ≤5 centów), stereo, transjenty, bas i wokal oraz cel opóźnienia
≤50 ms. Sto kolejnych obiegów pętli nie dodaje ani nie gubi beatów; granice
przechodzą odsłuch. Seek i przełączenia trybu nie odtwarzają starych buforów.
Fixture 4/4 o znanym downbeacie poprawnie odlicza takty i granice fraz,
również gdy pierwszy beat pliku nie rozpoczyna frazy. Manualna korekta
pozostaje nadrzędna wobec analizy. Na co najmniej 30 legalnych fragmentach
tonalnych z referencyjnymi etykietami cel detekcji tonacji to ≥80% poprawnych
wyników; „nieznana” w tym zbiorze liczy się jako błąd. Osobno testujemy
materiał perkusyjny i niejednoznaczny, który może otrzymać stan „nieznana tonacja”.
Sugestie pokazują powód dopasowania, uwzględniają varispeed i nie ukrywają
nieprzeanalizowanych utworów z widoku całej biblioteki. Porównujemy przejście
perkusyjne techno, melodyczne trance i krótką wariację rave bez wymuszania
jednego stylu miksowania.

## Etap 6 — sampler, generowanie brzmień i sekwencer

**Zależność:** etap 5.

Synteza ośmiu oryginalnych brzmień z recepturami i manifestem, eksport WAV,
osiem padów, 16 głosów i osiem ścieżek sekwencera. Obsługa one-shot/gate/loop,
kwantyzacji, panoramy i obwiedni. Chorus i flanger uzupełniają paletę efektów.

**6a — rytm i edycja:** wzorce 16-krokowe, velocity, gate, mute, choke hi-hatów,
krótkie szablony four-on-the-floor/backbeat/offbeat i cofanie edycji.
Najpierw odbieramy czytelność siatki i pełną obsługę z klawiatury.

**6b — wariacje i sceny:** długości 32/64 kroków na stronach po 16, do ośmiu
wzorców na ścieżkę, 32 sceny, bas nutowy z akcentem i slide, skala, swing,
makro „Acid” oraz kwantyzowane zmiany z odliczaniem i anulowaniem.
Stan edytowany, grający i oczekujący jest rozróżniony; edycje aktywnego wzorca
wchodzą od kolejnego taktu. Transport i master decków nie należą do scen.

**Odbiór:** z pustych decków tworzymy rytm i bas, dokładamy efekty, a następnie
dołączamy utwór dopasowany do zegara. Każdy sample ma plik stereo, recepturę,
pochodzenie i powtarzalny wynik z ustalonym ziarnem generatora. Eksport banku
działa offline. Sprawdzamy 16 głosów, przewidywalne przekroczenie limitu,
zwolnienia padów po utracie fokusu oraz test mono efektów modulacyjnych.
Offline wyzwalanie sekwencera trafia z dokładnością do jednej próbki względem
siatki silnika; pomiar przewodowy osobno potwierdza rzeczywistą responsywność.
Testy obejmują wszystkie cztery strony wzorca 64-krokowego, velocity bez
klawiatury dynamicznej, slide między nutami, choke i swing bez zmiany długości
taktu. Bas wlicza się do limitu 16 głosów. UNDO nie cofa transportu.
Przełączenie 32 scen nie dryfuje względem zegara; zastąpiona lub anulowana
akcja nie wykonuje się później. Niegotowa scena pozostawia poprzedni wzorzec.
Wybór sceny nie wycisza decka ani nie podnosi master gain. Stop decka będącego
źródłem BPM pozostawia ciągły zegar w ostatnim tempie. Dwóch użytkowników
układa wariację, robi break i wraca do rytmu, obserwując aktualny stan i moment
oczekującej zmiany; niezrozumiałe stany wymagają poprawy interfejsu.

## Etap 7 — nagrywanie, sesje i wydanie 1.0

**Zależność:** etap 6.

Nagrywanie mastera WAV stereo 24-bit z ditheringiem przy redukcji z float,
wybór folderu, zapis/odtwarzanie sesji, migracje, dopracowanie błędów i pakiet `.app`.
Oddzielny cue stereo testujemy, jeśli dostępny jest interfejs czterokanałowy;
brak takiego testu oznacza jawne wyłączenie deklarowanego wsparcia w wydaniu.
Ustalamy podpisywanie i notaryzację dla dystrybucji poza lokalnym MacBookiem.

**Odbiór:** godzinna sesja obejmuje dwa decki, aktywny key lock, osiem ścieżek
sekwencera, 16 głosów w szczytach, wszystkie typy FX, skan biblioteki,
waveform i nagrywanie. Obowiązują cele opóźnienia, pamięci i DSP z projektu.
WAV odtwarza się w innym programie, ma właściwą długość i kanały; raport
zawiera pomiar szczytów próbek i true peak oraz odsłuch nagrania.

Testujemy pełny dysk, błąd zapisu, przepełnienie kolejki nagrania, brakujące
utwory i wznowienie sesji w pauzie. Migracja zachowuje profile, cue i playlisty
na kopii istniejących danych. Instalacja, uprawnienia do katalogów, wyjście
audio, zmiana urządzenia i uśpienie/wybudzenie przechodzą próbę na rzeczywistym
macOS. Nieprzetestowana konfiguracja pozostaje jawnie oznaczona.

Dodatkowo obowiązuje dwugodzinna próba imprezowa z minimum 20 zmianami
utworów, wyszukiwaniem, preview cue przy dostępnym sprzęcie, aktywnym key lock,
sekwencerem i zmianami scen, efektami oraz nagrywaniem. Brak cue sprzętowego
oznacza jawnie nieprzetestowaną część, nie zaliczenie odsłuchu.
Co najmniej jedna część próby obejmuje skan w tle, aby potwierdzić priorytet
audio. Odbieramy limit cache PCM 256 MiB, brak przekroczeń budżetu callbacku,
cel p99/maksimum DSP oraz brak niezamierzonych podmian i przerw. Próba zbliżania
do limitu CPU najpierw zatrzymuje analizę/preload i redukuje animacje;
nie zmienia ukradkiem brzmienia, tempa ani bufora urządzenia.
Widok „Gotowość setu” zgłasza brakujące pliki i miejsce na nagranie,
zachowując możliwość ręcznego miksowania utworów bez analizy.

## Scenariusz końcowy dla użytkownika

1. Z klawiatury otwórz całą bibliotekę, odfiltruj kandydatów i ułóż kolejkę setu.
2. Załaduj A/B, skoryguj BPM, tonację, downbeat i frazę; ustaw pętle.
3. Uruchom rytm z wygenerowanego banku, dołącz pierwszy deck i zmieniaj sceny.
4. Używając gałek sterowanych parami klawiszy, zbuduj przejście z filtrem,
   echo, pogłosem i saturacją; przejdź do drugiego decka.
5. Do zwolnionego decka załaduj następny utwór bez przerywania muzyki;
   kontynuuj set, zachowaj czytelny master stereo, zapisz nagranie i sesję.
6. Otwórz sesję ponownie, sprawdź przypisania i odsłuchaj WAV poza aplikacją.

Wersja 1.0 jest gotowa, gdy scenariusz działa na sprzęcie odniesienia,
wszystkie obowiązkowe kryteria etapów są zaliczone, a raporty opisują także
realne ograniczenia. Kompilacja lub testy jednostkowe same nie stanowią odbioru.
