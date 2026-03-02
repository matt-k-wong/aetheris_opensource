# Aetheris Game Engine: A Rust 2.5D Framework

A true "cleanroom" 2.5D graphics engine and mathematical framework written entirely in memory-safe Rust. Built from scratch as an exploration into game engine architecture, Aetheris provides the foundational physics, spatial logic, and rendering bridges necessary to build classic, DOOM-style "Boomer Shooters" without any GPL-licensing restrictions.

It features a unique **Dual-Renderer Architecture**, allowing games built on the engine to hot-swap between an authentic CPU-bound software rasterizer (for nostalgia) and a blazing-fast, modern hardware-accelerated WGPU pipeline (for commercial performance and 4K resolutions).

---

## 🏗️ The Framework Architecture

To support commercial engine licensing, the project separates generic mathematical framework logic from proprietary display tech and specific game rules.

### 1. `aetheris` (The Open Source Core)
* **License:** MIT OR Apache-2.0
* **Contents:** The core engine framework. Contains generic 2.5D math, BSP node traversal algorithms, sector collision physics, binary WAD parsing, 3D spatial audio (`rodio`), and the classic CPU-bound Software Rasterizer. 
* **Details:** This crate has **zero** knowledge of specific game logic (like what a "Cyberdemon" or "BFG" is), and **zero** GPU proprietary rendering tech. It simply simulates and renders the generic `AetherisEntity` trait. You are free to build and sell closed-source commercial retro-shooters using this engine core without risk.

### 2. `aetheris_pro` (The Commercial Add-On)
* **License:** Closed-Source / Proprietary
* **Contents:** The high-performance hardware WGPU rendering pipeline.
* **Details:** Safely firewalled in its own crate, this module implements the generic `aetheris::VisualBridge` trait to pipe sector geometry physically to the GPU. This isolates the advanced modern display technology from the open-source tree.
* **Commercial Licensing:** To acquire an `aetheris_pro` license for use in commercial projects with high-performance hardware WGPU rendering, please contact [matt.k.wong@gmail.com](mailto:matt.k.wong@gmail.com) for pricing and terms.



## 🛠️ Building Games with Aetheris

This repository is an engine. **It is not a playable game.** Check out the official sibling project, **[Aetheris DOOM](https://github.com/matt-k-wong/aetheris_doom)**, for a complete example of how to implement game logic, AI state machines, and weapon handlers using this engine framework!

We encourage independent developers to fork the engine, build original commercial retro-shooters, and submit bug reports or framework feature requests!

---

## 💖 Support the Engine
The core `aetheris` generic framework is provided free and open-source to foster independent game development. If you are learning from this engine, using it for a hobby project, or just want to say thanks, please consider reaching out to support the project:
*   **Contact:** [matt.k.wong@gmail.com](mailto:matt.k.wong@gmail.com)
*   **PayPal:** [Donate via PayPal](https://www.paypal.biz/mattwongnyc)
*   **Solana (SOL):** `37dvG5eTSq8GN3vXf8hpPdZeAtmiFsARPp1cpNt3kTY2`
