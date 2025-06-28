# rMatterCertis - Tauri + Rust + SolidJS

E-commerce crawling application built with Tauri, Rust backend, and SolidJS frontend.

## Project Structure

### Frontend (TypeScript + SolidJS)
```
src/
├── components/     # UI components
├── services/       # API and Tauri communication
│   ├── services.ts # Main services export
│   ├── tauri.ts    # Tauri command wrappers
│   └── api.ts      # Higher-level API functions
├── stores/         # State management
│   └── stores.ts   # Main stores export
├── types/          # TypeScript type definitions
├── utils/          # Utility functions
│   └── formatters.ts # Formatting utilities
└── App.tsx         # Main application component
```

### Backend (Rust)
```
src-tauri/src/
├── domain/         # Business logic layer
├── application/    # Use cases and DTOs
├── infrastructure/ # External services
├── commands.rs     # Tauri command definitions
└── lib.rs         # Main library file
```

## Naming Conventions

- **No generic `index.ts` files**: Use descriptive names instead
  - ✅ `services.ts`, `formatters.ts`, `stores.ts`
  - ❌ `index.ts`, `index.ts`, `index.ts`
- **Clear module organization**: Each file has a specific purpose
- **Explicit imports**: Use named imports for better IDE support

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
