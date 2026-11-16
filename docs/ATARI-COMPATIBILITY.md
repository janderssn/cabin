# Atari boot audit

26 entries tested with Hatari 2.6.1 and EmuTOS 1.4. 13 reached an identifiable game screen; 13 remain hidden by default. Originals were write-protected. Existing save copies were not used.

| Game | Boot verified | Result |
|---|---|---|
| Armour-Geddon | Yes | Boot audit 2026-11-16: Press 1. Reached the 3D flight demonstration. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Atomino | No | Boot audit 2026-11-16: Automation 429 remains on compilation artwork after F2 and Space; game startup not confirmed. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| BattleTech: The Crescent Hawk's Inception | No | Boot audit 2026-11-16: Boot loader produces an illegal-instruction panic in EmuTOS. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Breach II | No | Boot audit 2026-11-16: Boot loader produces an illegal-instruction panic in EmuTOS. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Conflict: Middle East Political Simulator | Yes | Boot audit 2026-11-16: Press 2. Reached Conflict title / new-game menu. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Dogs of War | No | Boot audit 2026-11-16: Boot loader produces a bus-error panic in EmuTOS. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Dungeon Master | No | Boot audit 2026-11-16: Boot loader produces an illegal-instruction panic in EmuTOS. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Hunter | Yes | Boot audit 2026-11-16: Press 1, then Space twice. Reached the in-game 3D view. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Klax | No | Boot audit 2026-11-16: Pressing 5 leaves a black screen; no identifiable game screen reached. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Laser Squad | Yes | Boot audit 2026-11-16: Press 1. Reached player-selection screen; existing arrow mapping retained. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Lemmings | Yes | Boot audit 2026-11-16: Press 1, then Space twice. Reached the Lemmings main menu. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Loopz | Yes | Boot audit 2026-11-16: Press 2, then Space. Reached LOOPZ HAS LOADED screen. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Lords of Chaos | No | Boot audit 2026-11-16: Boot loader produces a bus-error panic in EmuTOS. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Midwinter | Yes | Boot audit 2026-11-16: Space, then 1. Reached the Midwinter title screen. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Midwinter II: Flames of Freedom | Yes | Boot audit 2026-11-16: Space through the loader. Reached the mouse/joystick/keyboard selection screen. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Phantasie III: The Wrath of Nikademus | No | Boot audit 2026-11-16: Boot loader produces a bus-error panic in EmuTOS. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Puzznic | Yes | Boot audit 2026-11-16: Press 2. Reached the animated game demonstration. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Sabre Team | Yes | Boot audit 2026-11-16: F2 (or Tab with the installed mapping), then Space. Reached Sabre Team title. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Space Crusade | Yes | Boot audit 2026-11-16: Press 1, then Space twice. Reached game language selection (F1 for English). Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Space Crusade: The Voyage Beyond | No | Boot audit 2026-11-16: Base game reaches language selection with the expansion in drive B; the expansion mission itself has not been verified. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Starflight | No | Boot audit 2026-11-16: Compilation menu boots; selecting 1 produces an address-error panic with the 192K ROM. Existing launch ROM retained. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Stunt Car Racer | No | Boot audit 2026-11-16: Loader displays “A VIRUS IS LURKING IN MEMORY”; startup stops. This is the loader’s compatibility warning, not an antivirus scan result. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| SunDog: Frozen Legacy | No | Boot audit 2026-11-16: Boot produces a Line-F emulator panic / corrupt title display. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Super Cars II | Yes | Boot audit 2026-11-16: Space, then 2. Reached the Super Cars II introduction. Entry 1 launches Moonshine Racers, not this game. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Supremacy | No | Boot audit 2026-11-16: Compilation boots, but tested entry 1 reaches a Justice Central terminal rather than an identifiable Supremacy screen. Correct selection remains unverified. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
| Tetris | Yes | Boot audit 2026-11-16: Press 6, then Space. Reached Tetris Hall of Fame / start screen. Tested headlessly with 1 MB RAM and write-protected disks. Boot verification only; not a full playthrough. |
