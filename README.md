# Aetheris DOOM Engine: A Rust 2.5D Rendering Framework

A true "cleanroom" 2.5D graphics engine written entirely in memory-safe Rust. Built from scratch as an exploration into AI-assisted game engine architecture, Aetheris natively loads and plays classic DOOM (`.WAD`) asset files. It features a unique **Dual-Renderer Architecture**, allowing you to hot-swap between an authentic CPU-bound software rasterizer (for nostalgia) and a blazing-fast, modern hardware-accelerated WGPU pipeline (for commercial performance and 4K resolutions).

---

## 🏗️ The Aetheris Workspace Architecture

To support both game development and commercial engine licensing, the repository is organized into a modular **Cargo Virtual Workspace** containing three distinct crates. This physically separates the generic mathematical framework from proprietary display tech and specific game rules.

### 1. `aetheris` (The Open Source Core)
* **License:** MIT OR Apache-2.0
* **Contents:** The core engine framework. Contains the generic 2.5D math, BSP node traversal algorithms, sector collision physics, binary WAD parsing, 3D spatial audio (`rodio`), and the classic CPU-bound Software Rasterizer. 
* **Details:** This crate has **zero** knowledge of specific game logic (like what a "Cyberdemon" or "BFG" is), and **zero** GPU proprietary rendering tech. It simply simulates and renders the generic `AetherisEntity` trait. You are free to build and sell closed-source commercial retro-shooters using this engine core without risk.

### 2. `aetheris_pro` (The Commercial Add-On)
* **License:** Closed-Source / Proprietary
* **Contents:** The high-performance hardware WGPU rendering pipeline.
* **Details:** Safely firewalled in its own crate, this module implements the generic `aetheris::VisualBridge` trait to pipe sector geometry physically to the GPU. This isolates the advanced modern display technology from the open-source tree.
* **Commercial Licensing:** To acquire an `aetheris_pro` license for use in commercial projects with high-performance hardware WGPU rendering, please contact [matt.k.wong@gmail.com](mailto:matt.k.wong@gmail.com) for pricing and terms.

### 3. `aetheris_doom` (The Target Game)
* **License:** Subject to DOOM mapping data
* **Contents:** The playable game port executable.
* **Details:** This bin consumes the `aetheris` framework like a plugin. It contains all the hardcoded DOOM weapon animation states (`states/`), monster AI logic, projectile trajectories, and look-up tables (`thing_defs`). It uses Rust Extension Traits (`DoomWorldExt` and `DoomThingExt`) to securely wrap raw `aetheris` entities with specific DOOM behavior rules entirely from the exterior codebase.

---

## 🧼 A True Clean-Room Recreation

It is important to emphasize that `aetheris_doom` is **not a source port**. It does not contain or derive from any of the original C source code released by Id Software in 1997. 

This engine and game logic implementation is a 100% ground-up, clean-room recreation written from scratch in Rust. It aims for strict logic and physics parity with the original vanilla executable (*Doom v1.9*) through black-box observation and documentation of the engine's behavior. 

Because no original copyrighted code was used, this framework is legally unencumbered and free to be used as a foundation for your own commercial projects under the MIT License.

### The RNG Exception
There is exactly one intentional exception to the clean-room rule: **The Random Number Generator (RNG) Lookup Table.** 

Original Doom relied on a hardcoded 256-byte array to provide pseudo-randomness for bullet spread, damage calculation, and monster AI. To retain perfect synchronization with classic recorded `.lmp` demo files, a physics engine *must* use this exact sequence of bytes. Because a raw, non-algorithmic array of numbers used purely for synchronized interoperability functions as a mathematical constant rather than expressive logic, it is legally permissible and necessary to include this 256-byte sequence exactly as it appeared in the original executable.

*(Disclaimer: While this engine was built using generative AI assistance, the AI was strictly prompted to generate logic based on black-box behavioral observation and mathematical first principles, rather than translating existing C source ports. Because Doom's algorithms represent highly optimized solutions to specific mathematical problems (e.g., 2D BSP traversal, raycasting), any structural similarities between this Rust code and the original 1997 C source release are the result of convergent functional design and the AI's generalized training, not intentional copying or derivation of copyrighted material.)*

---

## 💾 Getting the Game Data (.WAD)

Aetheris DOOM Engine is a framework and does not include copyrighted game assets. To run the engine, you must provide a `.WAD` (Where's All the Data) file. 

Place your chosen `.WAD` file directly in the root of the workspace directory (`aetheris_opensource/`).

**Where to get a WAD:**
1. **[Freedoom (Recommended & Included)](https://freedoom.github.io/download.html):** A completely free, open-source set of assets compatible with the DOOM engine. We have included `freedoom1.wad` in this repository for out-of-the-box testing!
2. **[DOOM 1 Shareware](https://www.doomworld.com/idgames/idstuff/doom/doom19s):** The original, legally free shareware version of DOOM (`DOOM1.WAD`) containing the first episode (Knee-Deep in the Dead).
3. **Commercial DOOM:** If you own DOOM on Steam or GOG, you can navigate to the installation folder and copy `DOOM.WAD` or `DOOM2.WAD`.

---

## 🚀 Running the Engine

```bash
cargo run -p aetheris_doom --release
```

### Specifying a WAD
By default, the engine loads `freedoom1.wad`. You can specify a different WAD using the `--wad` flag:

```bash
cargo run -p aetheris_doom --release -- --wad DOOM1.WAD
```

### Stacking Advanced Features
Cargo feature flags and runtime arguments are completely stackable. For example, if you want to play a custom WAD (e.g., `DOOM1.WAD`) and concurrently opt-in to the GPL-licensed authentic OPL3 music synthesizer, you can combine flags like so:

```bash
cargo run -p aetheris_doom --release --features opl_music -- --wad DOOM1.WAD
```

---

## 💾 WAD Files and Game Assets

It is crucial to understand that **Aetheris is an engine framework**, not a single game. The core framework can theoretically load and simulate data from any compatible `.WAD` (Where's All the Data?) file.

*   **Free Testing Asset:** To allow developers to immediately test the framework out-of-the-box without requiring commercial game assets, we bundle `freedoom1.wad`. Freedoom is a fully open, free asset replacement explicitly designed for engine recreation projects like Aetheris.
*   **Commercial Games:** If you wish to play the original, commercial Doom games, **you must legally purchase and provide your own copies of the commercial WAD files** (e.g., `DOOM.WAD`, `DOOM2.WAD`, `PLUTONIA.WAD`). Simply place them in the workspace root directory and load them using the `--wad` flag.

---

## 🎸 The Sound System (OPL3 Retro Mode & Licensing)

By default, the engine uses the MIT-licensed `rodio` library to spatialize the 3D Sound Effects (SFX) for monsters and weapons natively, keeping your binaries 100% clean for commercial use. Currently, this open-source release does not include a default MIT-licensed music synthesizer (though a `rodio` based one may be added in the future).

**⚠️ CRITICAL LEGAL IMPLICATIONS (GPL Contamination):**
The `opl_music` feature flag compiles the `opl-emu` crate (derived from the **Chocolate Doom** source port) to render the `GENMIDI` patch instruments. Because Chocolate Doom is licensed under the **GNU General Public License (GPLv2)**, enabling this feature flag legally infects your resulting compiled binary, converting the entire executable into a GPL-licensed product. 

* **Hobbyists:** Turn this on! It sounds incredible, and the GPL license is completely fine for free fan games.
* **Commercial Studios:** **DO NOT** compile with this flag if you are building a closed-source game. The GPL will force you to open-source your entire proprietary game repository. You have been warned!

---

## 🛠️ High-Value Engine Components

This repository is designed to be studied, stripped, and repurposed by Rust developers building "Boomer Shooters":

1. **The `.WAD` Parser Extractor:** Memory-safe bridging logic to ingest raw binary level lumps, BSP trees, palette topologies, and PCM wave audio formats straight out of Ultimate Doom Builder.
2. **The 2.5D Math/Collision Framework:** Standard 3D physics engines completely ruin the "feel" of retro shooters. This engine contains math for authentic sector-line sliding physics (`DoomWorldExt::apply_commands`), ledge step-ups, and cylinder intersections exactly replicating original vanilla rulesides.

## �️ Beyond Shooters
While this repository demonstrates a classic FPS port, **Aetheris is a generalized framework**. Because the core engine is decoupled from specific game rules, it can be adapted to entirely different genres. 

As a proof-of-concept, the core `aetheris` framework currently also powers a full **2.5D Flight Simulator**, demonstrating the engine's capability to handle complex Z-axis velocity, pitch/roll mathematics, and vast open-world coordinate spaces far beyond the scope of a standard corridor shooter (*Note: The flight simulator code is currently proprietary and not yet available in this public repository*).

## �💖 Support Aetheris DOOM Engine
The core `aetheris` generic framework is provided free and open-source to foster independent game development. If you are learning from this engine, using it for a hobby project, or just want to say thanks, please consider reaching out to support the project:
*   **Contact:** [matt.k.wong@gmail.com](mailto:matt.k.wong@gmail.com)
*   **PayPal:** [Donate via PayPal](https://www.paypal.biz/mattwongnyc)
*   **Solana (SOL):** `37dvG5eTSq8GN3vXf8hpPdZeAtmiFsARPp1cpNt3kTY2`

## 🤝 Contributing
For the reasons explicitly detailed in the audio licensing section above, **we do not accept Pull Requests that add GPL-licensed code to the core repository structure.** If you would like to contribute major features, a Contributor License Agreement (CLA) may be required to protect the MIT-safe commercial viability of the engine for the indie dev community.

---

## 🚧 Project Status & Known Limitations

Aetheris DOOM Engine is an exploration into modern Rust game architecture and is actively in development. While the core 2.5D math, BSP rendering, and WAD parsing are highly functional and the engine is playable, some gameplay and quality-of-life features are still being implemented.

**What works great:**
* Authentic CPU Software Rasterization (True 1993 feel)
* Full WAD file parsing (Levels, Textures, Flats, Sprites)
* Spatial 3D Audio (`rodio`) and OPL3 Music Synth (Chocolate DOOM emulation)
* Core AI state machines for DOOM monsters

**Known Issues / Roadmap:**
* **Game Menus:** The main menu is currently minimal (supporting only 'New Game' and 'Quit'). Save/Load functionality and deeper options menus are planned but not yet implemented.
* **Advanced Modding (DeHackEd):** Support for advanced DOOM modding capabilities and custom PWAD logic (like DeHackEd or ZScript) is currently stubbed out or only partially implemented.
* **Visual Artifacts:** You may encounter minor visual bugs or texture popping during intense gameplay or when viewing complex architecture. We are still actively stress-testing the renderer against community megawads.
* **Physics Edge-Cases:** Highly specific DOOM engine quirks (like wall-running, specific explosion knockbacks, or Okuplok geometry) may not be 100% physically identical to the original vanilla engine.

We encourage developers to clone it, build something cool, and submit bug reports or feature requests!
