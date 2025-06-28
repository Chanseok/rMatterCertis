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

## Development Progress

### ✅ Phase 1: Project Initialization (Completed)
- ✅ Tauri project setup with Rust + SolidJS
- ✅ Project structure and folder organization
- ✅ Modern Rust module structure (no mod.rs)
- ✅ TypeScript configuration and build pipeline
- ✅ Naming conventions and code organization

### 🚧 Phase 2: Backend Domain Implementation (In Progress)
- ✅ Core domain entities (Product, Vendor, CrawlingSession)
- ✅ Repository interfaces and traits
- ✅ Database schema and migration scripts
- ✅ Application use cases and business logic
- 🔄 Database repository implementations
- 🔄 Basic Tauri commands

## Development Scripts

```bash
# Development
npm run dev          # Start development server
npm run tauri dev    # Start Tauri development mode

# Building
npm run build        # Build frontend
npm run tauri build  # Build complete application

# Type checking
npm run type-check   # TypeScript type checking
cargo check          # Rust compilation check (in src-tauri/)

# Database
# Note: Database migrations will run automatically on first launch
```

## Database

The application uses SQLite for local data storage with the following schema:
- **vendors**: E-commerce site configurations
- **products**: Crawled product data
- **crawling_sessions**: Crawling operation tracking

## Testing Guide

### 🧪 Database Testing

The application includes several ways to test database functionality:

#### 1. Unit Tests
```bash
cd src-tauri
cargo test
```

#### 2. CLI Test Tool
```bash
cd src-tauri
cargo run --bin test_db
```

#### 3. UI Testing (Recommended)
```bash
# Start development server
npm run tauri dev

# In the application UI, click:
# - "Test Database Connection" - Tests connection and migrations
# - "Get Database Info" - Shows created tables
```

#### 4. Production Build Test
```bash
npm run tauri build
```

The application will create a SQLite database in `./data/matter_certis.db` and automatically run migrations on first launch.
