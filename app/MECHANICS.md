# Study Library — facts taken from the original (JW Library 15.9.54, Windows Store bundle)

The original is a .NET Native UWP app (JWLibrary.dll, 123 MB of native x86-64 code). Ghidra could not
open it and the built-in engine stops at 50,000 functions, so each function below was found by scanning
the code for `lea reg,[rip+…]` instructions that load a frozen string object (UTF-16 text − 12 bytes),
mapping the instruction to its function through the `.pdata` unwind table, cutting that function out
and decompiling it as raw x86-64 at its real address (native_function_pseudocode).
Addresses written `0x18…` are virtual addresses in JWLibrary.dll (image base 0x180000000); `0x4…` are file offsets.

## Decompiled functions and the rules taken from them
| Function | What it does | Rule used in this app |
|---|---|---|
| 0x185a824c0 (settings migration; stores at 0x185a827eb and 0x185a82d8e) | Writes setting `DefaultHighlightColor` = "0" and `FontSizePercentage` = "121" | default highlight colour = first colour (yellow); text size default 121 % |
| 0x186399b30 (getter) | Reads `FontSizePercentage` (default "121"), parses it and multiplies (`mulsd`) by the caller's base | reading size = base × percent / 100 |
| 0x186399a80 (setter) | Compares new size with current; if different stores it as text in `FontSizePercentage` and notifies | size changes are saved immediately |
| 0x185e1dd40 (highlight picker view-model) | Builds colour commands: Yellow→1, Green→2, Blue→3, Purple→6, Pink→4, Orange→5 (calls sub_185e1d920(n)), in that display order | picker order Yellow, Green, Blue, Purple, Pink, Orange; ColorIndex values 1–6 |
| 0x185ab2610 (pub-media request) | `/apis/pub-media/GETPUBMEDIALINKS?` + `langwritten` (default `univ` when no language) + `jwlversion=windows:<ver>` | same endpoint and `langwritten`; this app does not send `jwlversion` (it isn't the original) |
| 0x186d16110 / 0x186d161a0 (catalog URL) | environment 3 (production) → domain + `/catalogs/publications`; other environments → draft domains + `/assets` or `/catalogs` | production catalog address |
| 0x1863fe4e0 (media) | builds `/apis/mediator/v1/categories/` + language + key | same |

Base text size: `body{font-size:12pt; line-height:1.5}` (PrimaryPane.bundle.js) → 16 px, line height 1.5.
Heading steps: `du-fontSize--basePlus1/2/3` = 1.205 / 1.452025 / 1.749690125 em → H3 / H2 / H1.
Text-size step (10 %) and limits (60–300 %): **estimated** — the slider's limits were not found in the code.
The last colour picked becomes the default (Enter highlights the selection with it), matching the setting's name; the update rule itself was not located (estimated).

## Sections (resources.pri view names)
Home (DailyTextSectionView, FavoritesSectionView, MeetingsSectionView, WhatsNewSectionView,
TeachingToolboxSectionView, OnlineSectionView), Bible (BibleBooksView, BibleChapterGridView),
Publications (PublicationTocPage), Media (MediaGalleryView), Meetings (MeetingsPage),
Personal Study (NotesAndTagsPage, PlaylistsPage, RestoreBackupView), Search (LibrarySearchPage), Settings.

## Services (same public endpoints)
- Catalog: `https://app.jw-cdn.org` + `/catalogs/publications` (0x4c1cdee), `v4/manifest.json` → `current` → `catalog.db.gz`.
  Cover pictures: `https://app.jw-cdn.org/catalogs/publications/<ImageAsset.NameFragment>`.
- Publication files: `/apis/pub-media/GETPUBMEDIALINKS?` with `langwritten` (0x4b2ccfc). This app asks for EPUB (then PDF):
  the original's `.jwpub` files are encrypted and are **not** opened.
- Media: `/apis/mediator/v1/categories/` (0x4b8a9f0), roots `VideoOnDemand` and `Audio` (0x4b2d7fc).
- Online library: `https://wol.jw.org` (WolPrefix), language lookup through `/wol/finder?` (string table), daily text JSON `/wol/dt/{r}/{lp}/Y/M/D`, meetings `/{locale}/wol/meetings/{r}/{lp}/{year}/{isoweek}`.

## Catalog (Shipped/Catalog/catalog.db schema)
- PublicationTypeId: 1 Bibles, 2 Books, 4 Brochures and Booklets, 7 Kingdom Ministry, 10 Tracts and Invitations,
  13 Awake!, 14 The Watchtower, 22 Web Series, 30 Meeting Workbook, 31 Programs (label keys `pub_type_*`).
- DatedText.Class: 4 = daily text (es), 68 and 106 = meeting weeks. CuratedAsset.ListType 2 = Teaching Toolbox.
- Issue tag 20261100 → API issue 202611.

## Bible (Data/mepsunit.db)
- 66 books, chapter counts from BibleRange; groups from BibleBookGroup: 0 Pentateuch (1–5), 1 history (6–17),
  2 poetic (18–22), 3 prophets (23–39), 4 Gospels (40–43), 5 Acts, 6 letters (45–65), 7 Revelation.
- First Greek book 40 (BibleInfo.FirstBookOfGreekScripturesNumber).
- Group tile colours: **estimated** (in compiled XAML `bible_books_block1..3`).

## Highlights, notes, bookmarks (userData.db schema strings at 0x3fe3fdb–0x3ff57c8)
- UserMark(ColorIndex 1–6, LocationId, StyleIndex) + BlockRange(BlockType 1 = paragraph / 2 = Bible verse,
  Identifier, StartToken, EndToken). This app stores the same token ranges, so `.jwlibrary` backups import.
- Highlight colours (PrimaryPane.bundle.js `.cc-theme--light .notemarker-N`): yellow #fff37a, green #b7e492,
  blue #98d8ff, pink #f698bc, orange #ffba8a, purple #c1a7e2; dark theme #e9c600 #66a333 #4aa1dd #c64677 #e96d00 #7b57a7.
- Bookmark: 10 slots per publication (Bookmark.Slot; images bookmark0..9). Colours measured from the images:
  #9c194b #664194 #345996 #37a5af #1d8642 #47c681 #bbd72e #c3673c #a43d3b #814900.
- Note(Title, Content, LocationId, BlockType, BlockIdentifier); Tag(Type 0 Favorite, 1 user tag, 2 playlist); TagMap.
- Backup file extension `.jwlibrary` (zip with userData.db + manifest.json), playlist `.jwlplaylist` (0x2babb14).
- Accent colour #4a6da7 (reader link colour `jwac-footnote-link-color`).

## Differences
- Text is read from official EPUB files and online-library pages instead of encrypted .jwpub files.
- Videos/audio play in the computer's own media player.
- Highlight selection works by dragging across text (words snap), like the original; clicking selects a verse/paragraph.

## Media player (built in, like the original's MediaPlayerView / Miniplayer)
- Skip back 5 s and forward 15 s: the original's player icons `mediaplayer_skip_back_5` / `mediaplayer_skip_forward_15` (JWLibrary.Ui.Uwp/Assets).
- Speed menu (speedometer icon `media_player_settings_menu_speedometer`): 0.5×–2× steps are **estimated**.
- Large player over the page that minimizes to a mini player bar; full screen; playlists play in order.
- Playback uses the system's libmpv (loaded at run time); without it the app explains which package to install.

## Bibles
- Every translation the online library offers for any language (Bible list `/wol/binav/{r}/{lp}`, books
  `/wol/binav/{r}/{lp}/{symbol}`, chapters `/wol/b/{r}/{lp}/{symbol}/{book}/{chapter}`), including the study edition.
  "Download for offline" saves all 1,189 chapters. Verse markers `v{book}-{chapter}-{verse}-{n}` give the verse
  identifiers used for highlights and notes (BlockType 2).

## Publications without EPUB
- The study Bible, Insight and other works exist only in the original's protected format or as PDF. They are
  installed as their online-library pages (found through `/wol/finder?pub=…`, as the original does) and read in the app.

## Built-in playback (no outside player)
- MP4 (H.264 Baseline/Main + AAC) and MP3, streamed with range requests; own MP4 index reader
  (the publisher's files put a `pasp` entry before `avcC`); openh264 decoder without per-frame
  flushing (the 480p/720p files reorder pictures); sound via PulseAudio/PipeWire is the clock.
- Bible recordings: pub-media `fileformat=MP3` lists every chapter (book, chapter, address); the
  audio mark appears only on books that have recordings (e.g. Armenian has none for Leviticus–Deuteronomy).
- Chapter grid: grey (#757575) squares, 2–3 px gaps, 5 or 10 per row (person's reference picture).
- Home: What's New cards with square pictures; "Latest Videos" row from the mediator category `LatestVideos`.
