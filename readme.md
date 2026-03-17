# EmaLang
<div align="center">

<img src="docs/assets/logo.png" width="200" height="200" />

# 💎 EMA: THE UNIVERSAL FULL-STACK ECOSYSTEM
### *HTML, CSS, JS & PHP Replacement Language*

[![GitHub Stars](https://img.shields.io/github/stars/emadocstechnologies/emalangLang?style=for-the-badge&color=818cf8)](https://github.com/emadocstechnologies/emalangLang/stargazers)
[![GitHub License](https://img.shields.io/github/license/emadocstechnologies/emalangLang?style=for-the-badge&color=38bdf8)](https://opensource.org/licenses/MIT)
[![Build](https://img.shields.io/badge/Build-Stable-10b981?style=for-the-badge)]()
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-f1f5f9?style=for-the-badge)]()
[![Documentation](https://img.shields.io/badge/Docs-emadocs.com-a78bfa?style=for-the-badge&logo=docsdotrs)](https://emadocs.com)

**One Language. One Compiler. Zero HTML/JS Dependencies.**

🚀 **[Official Repository](https://github.com/emadocstechnologies/emalangLang)**

EMA is a high-performance programming language designed to unify the entire web stack into a single, cohesive syntax. It allows developers to build systems ranging from native database engines to reactive web dashboards without switching languages.

</div>

---

## 🏗️ THE UNIFIED STACK
EMA replaces the traditional web stack (HTML/CSS/JS/PHP) with a native, type-safe architecture.

- **Frontend (`@client`)**: Compiles to optimized WebAssembly. Supports declarative UI with native CSS-in-EMA styling.
- **Backend (`@server`)**: Direct LLVM machine code. Integrated with EMA-DB (SQLite) for zero-latency persistence.
- **Live Hydration**: Automatic state synchronization between server models and client-side UI.

---

## 🌍 CODE EXAMPLE (Full-Stack Dashboard)
A single `.ema` file manages your database, network, and UI simultaneously.

```ema
model User { name: str, level: int }

@server {
    print "Server logic: Initializing database...";
    User::insert("Evren Mimar", 99);
}

@client {
    state user_name = "Evren Mimar";
    state version = 4.0;

    <div style: "background: #0f2027; color: white; padding: 40px; border-radius: 20px;">
        <h1> "EMA-LIVE DASHBOARD" </h1>
        <p> "Welcome, " {{ user_name }} </p>
        <p> "Running on EMA v" {{ version }} </p>
        <button onclick: "alert('EMA Active!')"> "Launch System" </button>
    </div>;
}
```

---

## 🛠️ CORE FEATURES
- **Reactive State**: Use the `state` keyword for automatic UI updates.
- **Interpolation**: Double-brace `{{ ... }}` syntax for dynamic data binding.
- **Integrated DB**: Models are first-class citizens with auto-migration.
- **std::http**: Built-in high-performance web server capability.
- **Static Analysis**: Semantic checker ensures type safety across the entire stack.

---

## 🚀 QUICK START
```bash
# Clone the repository
git clone https://github.com/emadocstechnologies/emalangLang.git
cd EmaDocs

# Run the unified dashboard example
cargo run -- ema/example.ema
```

---

## 🔮 THE ROADMAP
- [x] **Phase 1-3**: Core Compiler & Declarative UI Infrastructure.
- [x] **Phase 4**: EMA-Live (SSR, Hydration & Reactive State).
- [/] **Phase 5**: Ownership & Borrowing (Rust-like memory safety).
- [ ] **Phase 6**: Advanced LLVM Link-Time Optimizations.

---

<div align="center">

**Developed with 💜 by the EmaDocs Team.**
*HTML is history. JS is a choice. EMA is the future.*

</div>
