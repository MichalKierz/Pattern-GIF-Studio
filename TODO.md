# TODO

## Otwarte

Brak otwartych punktow z ostatniej rozmowy.

## Zrealizowane 2026-05-24

1. Finalny `.exe` startuje jako Windows GUI bez konsoli przez `windows_subsystem = "windows"`.
2. Build osadza ikone z `assets/icon.ico` w zasobach Windows executable.
3. `Load Workflow` nie korzysta juz z pustego root `workflows`.
4. Bundled workflow presety nie sa juz bezposrednio w `presets`.
5. Root folder `workflows` zostal usuniety.
6. Utworzono `presets/workflows`.
7. Workflow presety przeniesiono do `presets/workflows`.
8. `Load Workflow` otwiera `presets/workflows`.
9. `Save Workflow` i portable setup nie tworza pustego root `workflows`.
10. Portable release kopiuje workflow presety jako `presets/workflows`.
11. Nie utrzymujemy juz problematycznego parytetu CPU/GPU.
12. Usunieto wybor CPU/GPU jako model produktu.
13. Usunieto wybor renderingu z UI.
14. Aplikacja renderuje przez GPU.
15. Usunieto `src/render/cpu_renderer.rs` i sciezke CPU backendu.
16. Usuniecie CPU objelo preview, export GIF, backend, session/workflow, testy i README.
17. Portable release nie zawiera martwego root `workflows`, `custom_assets`, README ani opisow CPU.
18. Poprzednia decyzja o przyciskach `On` / `Off` zostala zastapiona checkboxami przy nazwach pattern/effect.
19. `Save Source`, `Load Source` i `Remove` maja stabilne rozmiary w panelach pattern/effect/source.
20. Poprawki UI sa w kodzie aplikacji, wiec obejmuja finalny portable release.
21. Usunieto z UI suwaki `Symmetry`, `Distortion` i `Detail`; pola zostaly w formacie/renderze dla kompatybilnosci presetow.
22. Usunieto z UI sekcje `Formula controls p1..p4`; istniejace wartosci `controls` zostaly w assetach/renderze dla kompatybilnosci presetow.
23. `kaleidoscope.json` przemianowano na `liquid-neon.json` i ustawiono nazwe `Liquid Neon`.
24. `mandelbrot.json` przemianowano na `outrun-liquid.json` i ustawiono nazwe `Outrun Liquid`.
25. `orbit-trap-infrared.json` przemianowano na `ghastly-mandelbrot.json` i ustawiono nazwe `Ghastly Mandelbrot`.
26. `plasma.json` przemianowano na `plasma-waves.json` i ustawiono nazwe `Plasma Waves`.
27. `tunnel.json` przemianowano na `infrared-mandelbrot.json` i ustawiono nazwe `Infrared Mandelbrot`.
28. Portable build zawiera nowe nazwy workflow presetow i nie zawiera starych nazw plikow.
29. Dodano kanoniczny preset kolorow `presets/color_sets/rainbow.json`.
30. Patterny maja checkbox zamiast przyciskow `On` / `Off`.
31. Checkbox patternu jest przed nazwa typu `Pattern 1`, relatywnie na poczatku wiersza warstwy.
32. Effecty maja checkbox zamiast przyciskow `On` / `Off`.
33. Checkbox effectu jest przed nazwa typu `Effect 1`, analogicznie do patternow.
34. Usunieto przycisk `Remove` dla pierwszego patternu zamiast zostawiac dezaktywowany przycisk.
35. `Load Source` aktualizuje nazwe patternu z nazwy zaladowanego presetu/source assetu.
36. `Load Source` aktualizuje nazwe effectu z nazwy zaladowanego presetu/source assetu.
37. Ladowanie source z workflow presetu zwraca nazwe warstwy z workflow zamiast ogolnego fallbacku.
38. Pod `Formula Source` usunieto napis `Pattern source`.
39. Pod `Formula Source` usunieto napis `Effect source`.
40. Pod `Formula Source` usunieto tekst `Mode: ...`.
41. Niebieskie przyciski dostaly stabilna minimalna wysokosc, zeby hover nie powodowal wizualnego zmniejszania.
42. Usunieto suwak `Color transition`, bo nie byl aktywna kontrolka produktu.
43. `Color phase` zachowuje wartosc `1.0` w stanie UI zamiast sanitizowac ja do `0.0`.
44. Odtworzono portable release po zmianach UI i logiki load source.
45. Niebieskie przyciski nie wymuszaja juz stalej wlasnej obwodki przez `.stroke()`.
46. Niebieskie przyciski korzystaja z tego samego mechanizmu hover/active co zwykle szare buttony.
47. Obramowka niebieskich przyciskow pojawia sie dopiero na hover/active przez standardowy styl egui.
48. Stan nacisniecia niebieskich przyciskow dziala przez standardowy styl buttona, z podmienionym kolorem tla.
49. `Save GIF` korzysta z tego samego helpera primary button co reszta niebieskich przyciskow.
50. Odtworzono portable release po poprawce stylu niebieskich przyciskow.
51. Dawne niebieskie przyciski sa teraz zwyklymi szarymi przyciskami egui.
52. Helper `primary_button` nie podmienia juz kolorow tla ani obwodki.
53. Usunieto napis `Export renderer: GPU` z obszaru podgladu/exportu.
54. Tytul `Pattern GIF Studio` ma lekki padding od lewej i od gory.
55. Sukcesowe operacje `Loaded/Saved ...` czyszcza status zamiast pokazywac `Status: ...`.
56. `Status: ...` pozostaje tylko dla bledow albo blokujacych komunikatow runtime.
57. Sprawdzono, ze `README.md` opisuje aktualny stan: Pattern GIF Studio, GPU-only, portable `presets/workflows`, brak root `workflows` i `custom_assets`.
58. Po tytule `README.md` dodano `showcase/showcase.png`.
59. Pod obrazem showcase dodano w jednym rzedzie GIF-y `burning-mandelbrot.gif`, `frozen-lava.gif`, `golden-cloth.gif` i `overexposure.gif`.
60. Dodano wyjatek `.gitignore` dla `showcase/*.gif`, zeby GIF-y z README mogly trafic na GitHub.
61. Usunieto z `README.md` sekcje `Repository Layout`.
62. Usunieto z `README.md` sekcje `Portable Data Policy`.
63. Usunieto z `README.md` sekcje `Development`.
64. Skrocono `Technology` do najwazniejszych pozycji: Rust, egui/eframe, wgpu, gif i serde_json.
65. Usunieto z `Features` wzmianke o GPU jako osobnym featurze.
66. Usunieto z `Features` wzmianke o kanonicznych presetach jako oczywistosc.
67. Usunieto z `Features` wzmianke o testach automatycznych.
68. Usunieto z opisu `README.md` oczywistosc o zapisie calego projektu jako JSON.
69. Dopisano do `README.md` sekcje `How to use`.
70. `How to use` opisuje workflow w aplikacji zamiast komend typu `cargo build`.
71. `How to use` unika oczywistych instrukcji i skupia sie na kolejnosci pracy: workflow/source presets, GIF output, pattern stack, effects, formula/domain, colors i save workflow/source.
