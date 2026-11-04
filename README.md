# FO3ID Private Server (Rust Port)

A local private server for **FIFA Online 3 Indonesia** (Garena, client build 654420), written in
**Rust**.

> **Credits & Origin:**  
> This project is a complete Rust port and rewrite based on the original Python implementation by **SoloPlayer**: [soloplayer24/FIFA-ONLINE-3-Private-Server](https://github.com/soloplayer24/FIFA-ONLINE-3-Private-Server).

> Not affiliated with EA, Spearhead or Garena. This repository contains **no game files** (no
> client, database, assets or music) — you need your own FO3 ID install. It does **not** include,
> modify or circumvent the client's anti-cheat (XIGNCODE3 / HackShield); that is out of scope.
> Built for game preservation and research.

## What works

Tested in the real client (Indonesian UI):

- Login → manager select / create (club picker over 501 licensed clubs) → lobby (FREEROAM).
- **Single → Liga** (full season: calendar, matches, table, rewards, promotion) and **Cup**
  (draw, groups, knock-out, extra time, penalties).
- **Market** (buy / sell; a card's price follows its rating and grade: the median fifaaddict
  price of all cards with that rating), **Upgrade** (grade +1…+10), **Tukar** and
  **Tukar Scout** player trades (every Market card, not only the region's licensed ones),
  Tim inti / Tim Cadangan, squad and tactics saving.
- **Shop** with every tab (player packs, consumables, packages, MP Shop, monthly and chained
  packages, special bonus), pack opening with random grades, Item saya, Bag, daily login reward.
  Extra packs: **All Season +8** (any season, +1…+8) and one pack for '07 and every season
  fifaaddict lists.
- Always VIP + official PC warnet (IGR) perks, manager delete / restore, season '17 cards.
- Formation list (the club's own + the game's 33 common formations, read from the client's
  formations table) and manager / player levels: EXP after every match, player LV.1-23
  (+1 to every attribute per level), manager LV.1-100.
- Game sound and the EA Trax menu music. `tools/eatrax_song.py` puts your own song first in it,
  but the game then crashes after matches (see the known issue below).
- Admin web page on http://127.0.0.1:8088/ (EP, players, packs, gifts, daily rewards, log).
- Real club kits in matches (kit id = `teamid << 5 | type`; the old `<< 10` gave a checkerboard).
- Matches play through the second half with **team color** on (season colors used to crash it at
  half time), then show the EXP and reward screens.

Added, passing the tests but not yet checked in the client:

- **Team color** bonus itself: the starting XI gets an OVR bonus for players from the same club
  (4-5 players +1 up to 10-11 +4), nation or season (5-6 +1 up to 9-11 +3); per player only the
  best one counts.
- **Opsi > Kontrol** (keyboard and gamepad keys) is saved: the server always sends the game's own
  default key layout, so the screen can apply and save changes (it used to drop them).
- **Kit choice** on the game setting screen (home / away per side) is kept for the match.
- **Manager select** shows the club's most valuable player in the club's kit (it showed the
  game's default Barcelona player).
- Fixed crashes: the startup notice popup (empty web URL), the Beginner's Guide league tutorial
  (missing script).
- **Upgrade chance**: a base chance by target grade, times the "Rasio bonus keberhasilan" label
  the Upgrade tab shows from the material's grade (Kosong x1, Low x1.1, Normal x1.25,
  High x1.5). A failed upgrade from +5 or higher drops one grade.
- **Top 200 packs**: the extra packs now give their season's 200 best cards ("17 Top 200",
  "07 Top 200", …); **All Season Top 300 +8** gives any season's cards rated 81 and up.
- **Upgrade with a +0 card** (issue #5): a club's own +0 card keeps the target's grade at 100%
  (+4 with +0 stays +4) and a +10 may take one; the kept card leaves Tim Cadangan when it takes
  the used card's squad slot.
- **Upgrade 2.5** (the client's newer Upgrade screen, `features...enhancement.v2_5`):
  - **upgrade points**: a failed upgrade grants points; "Gunakan poin" spends them for +10%;
    buy more free in Shop > Paket (Upgrade Point 100 / 500 / 1.000);
  - **boost items** Enhancement Boost Gold / Silver / Bronze (+15 / +10 / +5%), free in
    Shop > Dapat dikonsumsi;
  - a permanent **upgrade event** (+10% for targets +1…+9, `GET /events`);
  - "Coba pengembalian lagi" gives back a grade lost to a failure, for points.
- **Tukar v2** (`trade2`): a trade is rated by its value (TP) in 15 levels, each with a maximum
  offer grade; the tab shows what another manager got "yesterday" at that level; legend-season
  cards count 1.5x; no daily or value limit. Tukar Scout keeps working beside it.
- **Tukar Scout**: the client's two other scouts, Scout Khusus (event; legend-season quest) and
  VVIP Scout (best offers; 1 billion EP quest). Scout Amatir is gone so the list fits its 5 rows
  (once it scrolled, a met quest lit another scout's card or none).
- **Upgrade team colors** (the client's "Upgraded Team"): starters of grade +2, +5 or +8 and
  higher; 5-6 / 7-8 / 9-11 of them give +1 / +2 / +3, +2 / +3 / +4 and +3 / +4 / +5 OVR. The
  single-grade ones ("+3 club" … "+10 Golden summit") need all 11 starters at that grade or higher:
  +4 (club +3/+4), +5 (silver +5…+7), +6 (Golden passion +8/+9), +7 (Golden summit +10). A
  player gets his best upgrade color on top of his best club / nation / season color.
- **Every team color the client names** (188 now):
  - 27 clubs (Real Madrid … Jeonbuk): Single team's tiers plus the club's own boost from the
    game's tooltip, e.g. Barcelona MF agility +3;
  - 30 nations: Satu Negara's tiers plus e.g. Spanyol MF ball control +2;
  - season colors under the game's names (World Best up to +5, World Legend, 2014 WC All-Star, …);
  - about 75 specials: '14 WC and EC per nation, nation BEST, MSN / BBC Trio, Arsenal
    Invincibles, Galácticos, LEGENDS, the TOP 100s, Power Header, New Star, ….

  The original boosts and player lists were server data and are gone. Player lists come from the
  player database (season, club, named players, stat limits); boosts come from the one tooltip
  line the game still has. Left out: the [Test] colors, "+2~7", "14 WC Single Nation" and four
  duplicate club entries.
- **Formasi** no longer opens with "Tidak cukup pemain" (it loads preset squad 1, which the
  server never sent). A hole in the XI (issue #7, an upgrade of two starters of one player) is
  filled from the bench, and the bench never holds a second card of a player. Every starter gets
  his `starting` data (workrate and pitch spot): without it the game shows him on the bench.
- **Tim > Peran** saves the captain and each set-piece taker (issue #8: `PUT /game/my/team/captain`
  and `/game/my/team/kickers/<0-4>` were missing). The five kicker slots (left corner, right
  corner, free kick, penalty, free kick 2) keep their place; an empty one takes a starter.
- **Market search** finds cards of every season: the game searches only the seasons in
  `/static/roster`, which lacked 29 seasons with Market cards (World Legend, World Best,
  '14 TOTS, …), so e.g. WL R. Gullit never showed up.
- **Every tradeable card** is on the Market, in the trades and in a pack: a card counts when
  fifaaddict lists it or the game itself marks it tradeable (`permid.transferable`). fifaaddict
  alone left out 6,599 cards (4,313 of them '17); the Market now holds 44,703. The new
  **Semua Pemain** pack can give any of them.
- **Tim > Klub > Informasi tim** (issue #9) shows the start date, home stadium and "year built /
  capacity" (`coach.career`, `coach.stadium`, `GET /stadium/<id>` from the client's own stadium
  table), and its player panel (stepper, 3D player, price, grade), which stayed empty without a
  stadium. Names from the client's database read as UTF-8 (`tools/re/eadb.py` read latin-1).
- **Pack opening glow** follows the card's value: none below 10 million EP, then levels 1-3 from
  10 million, 100 million and 1 billion EP.

Known issue, not solved: with a custom EA Trax song installed by `tools/eatrax_song.py` the game
runs out of its RAssets memory pool after every match, while the Cup screen or the lobby loads
again (crash in `FifaRNA::Crowd::PlacementData::Seats[]`); with the game's own music it does not.
It crashed the same way with the song as raw PCM (37.7 MB), as EALayer3 (6.5 MB,
`tools/re/ealayer3.py`) added as a 10th track, and as EALayer3 replacing TrackID 1 with the music
bank smaller than the shipped one, so the size of the music is not the cause. Keep the game's own
music: `python -I tools/eatrax_song.py --restore` puts it back.

Not done: online modes (ranked, friendly, King of the Hill), Road to World, league simulation.

## Requirements

- Windows, Rust toolchain (Rust 1.80+ / stable).
- An FO3 ID client install (default path `D:\FO3ID\32795`; elsewhere, set `FO3_GAME` to its
  folder; the server warns at startup when it cannot find the client's image cache there).

## Setup

1. Extract the player database from your client (if not already extracted):
   `cargo run --release --bin extract_fifadb -- D:\FO3ID\32795` → `gamedata/fifadb.sqlite`.
2. Build the project:
   `cargo build --release`
3. Hosts file (as admin): `127.0.0.1 APSEODESK172`.
4. Build the launcher once (`launcher/fifalauncher.cs`, build line inside), then double-click
   **`play.bat`**: it opens the Rust server window (`target
elease\start_all.exe` running
   HTTPS :443 + Admin :8088 + Messenger :28002 + Blaze :28000/:28001) and starts the game.

5. By hand instead: `cargo run --release --bin start_all`, then start the client with the server arguments
   (what `launcher/fifalauncher.exe` and `launch_game.ps1` pass):

   ```
   fifazf.exe -QUICKMATCH/SERVER=127.0.0.1:28000 -QUICKMATCH/PROTOCOL=tcp -QUICKMATCH/NOTISERVER=127.0.0.1:28001 -FIFA/LAUNCHER/PID=<pid> -PUBLISHER/GARENA/TOKEN=testtoken -LOCALE/PUBLISHING_REGION=ID -DUMPTRUCK/SERVER=127.0.0.1 region id token testtoken
   ```

## Files

| File / Module | Role |
|---|---|
| `src/http/` | The REST backend the client calls (`https://APSEODESK172`): login, account, coaches, club creation, squad, league, cup, market, upgrade, trades, shop, items, Bag, daily reward, static tables, and `/cdn/`. |
| `src/messenger/` | Notification server on `:28002`. Blowfish-CBC encryption (`kueec`); answers `auth`, `ping`, `signon`. |
| `src/admin/` + `admin/index.html` | Admin dashboard on `:8088` (localhost only, `X-FO3-Admin` security). |
| `src/blaze/` | EA Blaze protocol engine (Fire framing + TDF Tagged Data Format codec) and TCP redirector / notification server. |
| `src/tools/` | RefPack codec, BIg4 archive directory parser, EA DB reader, EALayer3 audio parser, capture analyzer, fifadb extractor. |
| `src/bin/` | Standalone CLI binaries (`start_all`, `http_config`, `messenger`, `server`, `test_client`, `analyze_capture`, `extract_fifadb`, `refpack`, `bigdir`, `eadb`, `ealayer3`). |
| `responses/` | Per-endpoint JSON bodies (`/a/b/c` → `responses/a_b_c.json`, re-read on every request). |
| `gamedata/` | State file `state.json` and client database `fifadb.sqlite`. |

## Tests

```
cargo test
```

Runs all 44 unit and integration tests covering Blaze protocol framing, TDF tags, Messenger encryption, RefPack decompression, League scheduling & simulation, Cup tournaments, Market, Upgrades, Team Colors, Admin API, and loopback clients.

## Contributors & Acknowledgements

- **SoloPlayer** ([soloplayer24/FIFA-ONLINE-3-Private-Server](https://github.com/soloplayer24/FIFA-ONLINE-3-Private-Server)) — Original project creator, reverse engineering, and Python implementation.
- **Claude** (Anthropic's AI assistant) — Reverse engineering and original Python codebase contribution.
- **ngnsusinn** & **Hermes Agent** (Nous Research) — Complete port and migration of the entire server suite, tools, and algorithms to high-performance Rust.

## License

[GNU GPL v3](LICENSE) for the code and documentation in this repository. FIFA Online 3, its
client, data, assets and music belong to their respective owners and are not covered.
