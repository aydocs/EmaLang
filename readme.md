# EMA: THE UNIVERSAL FULL-STACK ENGINE 💎
### *Unified HTML, CSS, JS & PHP Ecosystem*

<div align="center">
  <img src="docs/assets/logo.png" width="200" height="200" />
  <br />
  
  [![V1.0 Stable](https://img.shields.io/badge/Version-V1.0-818cf8?style=for-the-badge&logo=cargo)](https://github.com/aydocs/EmaLang)
  [![Developer: aydocs](https://img.shields.io/badge/Developer-aydocs-38bdf8?style=for-the-badge&logo=github)](https://github.com/aydocs)
  [![License: Private](https://img.shields.io/badge/License-Proprietary-f43f5e?style=for-the-badge)]()
</div>

---

EMA is a high-performance programming language and compiler architecture designed to unify the entire web stack into a single, cohesive syntax. It allows developers to build systems ranging from native database engines to reactive web dashboards without switching languages or managing fragmented dependencies.

## 🏗️ THE UNIFIED ARCHITECTURE
EMA replaces the traditional overhead of decoupled tech stacks with a native, unified execution model.

- **Frontend (`@client`)**: Compiles to optimized WebAssembly or lifted JavaScript. Features Scoped CSS-in-EMA for zero-conflict styling.
- **Backend (`@server`)**: Direct Rust-speed machine code. Integrated with EMA-DB (SQLite/PostgreSQL) for seamless data persistence.
- **V1.0 Robustness**: Hardened lexer and parser logic ensuring professional-grade reliability for enterprise applications.

---

## 🚀 GETTING STARTED
With the EMA V1.0 CLI, you can initialize and deploy "World-Standard" applications in seconds.

```powershell
# Create a professional project structure
ema create my-awesome-app

# Compile and Deploy with zero configuration
ema --deploy Web/app.ema

# Access the interactive development shell
ema repl
```

---

## 🎨 CODE EXAMPLE: FULL-STACK UNIFICATION
A single `.ema` file manages your state, database, and UI logic simultaneously.

```ema
# Define a database model (Auto-Migration)
model Product { id: int, name: str, price: float }

@server {
    print "Ema Server Initialized: Port 3000";
    std::http::serve(3000);
}

@client {
    state cart_total = 0.0;

    <div style: "background: #0f172a; padding: 40px; border-radius: 20px; color: white;">
        <h1 style: "font-size: 2.5rem;"> "EMA V1.0 Ecosystem" </h1>
        <p> "Current Balance: $" {{ cart_total }} </p>
        
        @js {
            // Native JavaScript blocks directly in your UI logic
            async function triggerCelebration() {
                console.log("Ema Engine: Processing world-class event...");
            }
            triggerCelebration();
        }
    </div>
}
```

---

## 🛠️ CORE CAPABILITIES
- **`ema create`**: Professional project scaffolding with built-in best practices.
- **Scoped Styling**: Automatic CSS isolation for every component.
- **Hot-Port Discovery**: Automatically handles port conflicts during development.
- **Zero-Dependency Bundle**: Compiles down to a minimal, high-efficiency production artifact.

---

## ⚠️ PROJECT STATUS: UNDER ACTIVE DEVELOPMENT
EMA is an **expressive programming language** currently in its **active alpha/beta phase**. While V1.0 provides a powerful and robust core, users should expect ongoing updates, experimental features, and potential bugs. We invite the global developer community to explore, test, and provide feedback as we move toward full-scale production stability.

---

<div align="center">
  <strong>Developed with 🚀 by aydocs.</strong>
  <br />
  *The future of the web is unified. The future of the web is EMA.*
</div>