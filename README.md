# Cabin

![Cabin travel shelf](docs/travel.png)

A native Rust desktop library for offline flight entertainment. Browse documentary artwork, game screenshots and magazine covers, search the collection, save favorites, and return to recently opened items.

The installed collection contains 40 native puzzles, 26 Atari entries, nine documentaries (7 hours 12 minutes), and eight magazines. Travel includes Lisbon, Portugal’s Heartland, a Japanese railway documentary, two Hokkaido publications and The Travel Magazine.

## Run

Open **Cabin** from the desktop application launcher, or run `~/.local/bin/cabin`.

- Ctrl+F searches; Escape goes back or clears search.
- Space opens the current detail item; Q quits; F11 toggles fullscreen.
- Videos open in mpv with resume support; magazines open in Evince.
- Atari games open in Hatari fullscreen. F12 opens settings/disks; Right Alt+M releases the mouse. Individual game notes explain boot menus and compatibility limitations.

All browsing, artwork and playback work offline. No network requests occur in the app. Download scripts are separate, manually invoked tools. Game compatibility notes distinguish verified boots from untested entries.

## Build and install

Requires Rust stable and the locked dependencies, plus the system graphics libraries. Runtime integrations are `mpv`, `evince`, `hatari` and Simon Tatham’s `puzzles` package.

```sh
cargo build --release --locked
cargo test --offline
bin/install
cabin check --json
cabin launch puzzle-net --dry-run
```

Installation is per-user; no administrator access is needed. The app uses XWayland on this Omarchy desktop and reads the current Omarchy accent without editing desktop configuration.

## Files and preservation

- Catalog: `~/.local/share/cabin/library.json` (override with `CABIN_LIBRARY`).
- Artwork: `~/.local/share/cabin/assets/`.
- Favorites/recent/logs: `~/.local/state/cabin/`.
- Magazines: `~/magazines/flight/`, with original URLs and SHA-256 hashes in `sources.json`.
- Films: `~/videos/flight/documentaries/`, with original metadata and playlist.
- Original Atari images: `~/games/atari-st/`. Writable copies are reused in `~/.local/state/atari-games/`; original images are preserved.

`list --json` exports the catalog, `check --json` checks local content paths, and `launch ID --dry-run` prints the exact executable and argument array without launching. The app never passes content paths through a shell. `--section travel` opens the travel shelf; `--detail ITEM_ID` opens a detail page. `--screenshot PATH.png` captures the actual rendered window and exits.

The preparation scripts retain source attribution in each catalog entry. `prepare_library.py` rebuilds the base library; run `add_travel.py` afterwards to add the travel collection. They expect the media and games to have already been downloaded. Media and third-party artwork retain their original rights and are not included in the source repository.

## Native Wayland trial

`cabin --wayland` opts into native Wayland and uses the compositor's scale directly (no 0.75 zoom adjustment). On this Hyprland desktop at scale 2, the trial rendered successfully at 2880×1800 and Hyprland reported `xwayland: false`, `fullscreen: 2`.

XWayland remains the default because a physical touchscreen gesture pass has not yet been completed. A touchscreen is detected, but rendering alone does not prove tap/drag behavior. The earlier native build also stalled; the current bounded trial completed successfully. Keeping the established default follows the requirement to switch only after both fullscreen and touch are verified. Use `--wayland` for the touch playtest; no desktop configuration changes are required.
