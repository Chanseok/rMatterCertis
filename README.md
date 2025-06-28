# rMatterCertis - Tauri + Rust + SolidJS

E-commerce crawling application built with Tauri, Rust backend, and SolidJS frontend, following modern Rust 2024 conventions.

## 📊 Current Project Status

**Phase 2 Complete, Phase 3 actively in progress (60% complete)**

For detailed project status, progress tracking, and architectural decisions, see: **[📋 PROJECT STATUS](./guide/PROJECT_STATUS.md)**

## Project Structure (Rust 2024 Modern)

### Backend (Rust) - mod.rs Free Structure
```
src-tauri/src/
├── main.rs
├── lib.rs
├── commands.rs         # Tauri command definitions
├── domain.rs          # Domain layer entry point (no mod.rs)
├── domain/
│   ├── entities.rs     # Business entities
│   ├── repositories.rs # Repository trait definitions
│   └── services.rs     # Domain services
├── application.rs     # Application layer entry point (no mod.rs)
├── application/
│   ├── dto.rs         # Data Transfer Objects
│   └── use_cases.rs   # Use case implementations
├── infrastructure.rs  # Infrastructure layer entry point (no mod.rs)
├── infrastructure/
│   ├── repositories.rs    # Repository implementations (consolidated)
│   ├── database_connection.rs # Database connection management
│   ├── config.rs         # Configuration management
│   ├── database.rs       # Database utilities
│   └── http.rs          # HTTP client
└── bin/
    └── test_db.rs       # Database testing CLI tool
```

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

## Modern Rust Architecture

### ✅ Key Improvements Implemented
- **All mod.rs files removed** - Following Rust 2024 best practices
- **Module entry points** - Using `module_name.rs` instead of `module_name/mod.rs`
- **Consolidated implementations** - Related code grouped in single files
- **Clean Architecture** - Clear separation of concerns
- **Build performance optimized** - 90% faster incremental builds

### Naming Conventions

- **No generic `index.ts` files**: Use descriptive names instead
  - ✅ `services.ts`, `formatters.ts`, `stores.ts`
  - ❌ `index.ts`, `index.ts`, `index.ts`
- **No mod.rs files**: Use modern Rust module structure
  - ✅ `infrastructure.rs` (entry point), `infrastructure/repositories.rs`
  - ❌ `infrastructure/mod.rs`, `infrastructure/repositories/mod.rs`
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
- ✅ Build performance optimization (90% improvement)
- ✅ Basic database connection and testing

### ✅ Phase 2: Modern Module Structure (Completed)
- ✅ **All mod.rs files removed** - Rust 2024 conventions applied
- ✅ **Module consolidation** - Related implementations unified
- ✅ **Repository pattern foundation** - Traits and basic implementations
- ✅ **Domain entities completed** - Product, Vendor, CrawlingSession
- ✅ **Database schema and migrations** - SQLite with proper constraints
- ✅ **Tauri commands integration** - Backend-frontend communication

### 🔄 Phase 3: Backend Logic Completion (Current)
- 🔄 Repository test stabilization (fixing DB permission issues)
- 🔄 Use case implementations
- 🔄 Error handling and logging system
- 🔄 Extended Tauri commands

### 📋 Upcoming Phases
- **Phase 4**: Web crawling engine implementation
- **Phase 5**: Frontend UI development
- **Phase 6**: Integration testing and optimization

## Documentation

📚 **Comprehensive guides available in `/guide/` directory:**

- [� Development Guide](guide/matter-certis-v2-development-guide.md) - Step-by-step implementation guide
- [📋 Project Setup Checklist](guide/matter-certis-v2-project-setup-checklist.md) - Initial setup instructions
- [📄 Requirements](guide/matter-certis-v2-requirements.md) - Technical specifications
- [� Build Optimization](guide/rust-build-optimization.md) - Performance tuning guide
- [🏗️ Modern Module Structure](guide/rust-modern-module-structure.md) - Rust 2024 conventions
- [📅 Phase 2 Plan](guide/phase2-implementation-plan.md) - Current phase details

## 📋 Development Guides

### 📊 Project Status & Planning
- **[📋 PROJECT STATUS](./guide/PROJECT_STATUS.md)** - Current progress and architecture overview
- **[🚀 Phase 3 Implementation Plan](./guide/phase3-implementation-plan.md)** - Detailed 4-week implementation roadmap
- **[📋 Development Guide](./guide/matter-certis-v2-development-guide.md)** - Complete development documentation

### 🏗️ Architecture & Design
- **[🧠 Core Domain Knowledge](./guide/matter-certis-v2-core-domain-knowledge.md)** - Domain logic and database design
- **[🔧 Memory-based State Management](./guide/memory-based-state-management.md)** - Session management architecture
- **[🧪 Test Best Practices](./guide/test-best-practices.md)** - Testing strategies and utilities

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

## 📚 개발 가이드 문서

본 프로젝트의 상세한 개발 가이드는 다음 문서들을 참고하세요:

- **[프로젝트 셋업 체크리스트](guide/matter-certis-v2-project-setup-checklist.md)** - 처음부터 프로젝트를 생성하는 방법
- **[단계별 개발 가이드](guide/matter-certis-v2-development-guide.md)** - 실제 구현 과정과 검증된 방법들
- **[프로젝트 요구사항](guide/matter-certis-v2-requirements.md)** - 기술 스택과 아키텍처 결정사항
- **[Rust 빌드 최적화 가이드](guide/rust-build-optimization.md)** - 개발 생산성 향상을 위한 빌드 성능 최적화

모든 가이드는 실제 구현 과정에서 검증된 내용들로 구성되어 있어 신뢰할 수 있습니다.

## Database

The application uses SQLite for local data storage with the following schema:
- **vendors**: E-commerce site configurations
- **products**: Crawled product data
- **crawling_sessions**: Crawling operation tracking

## Testing Guide

### ⚡ Fast Development & Testing

For optimal development experience, this project includes build optimizations:

```bash
# Load optimized development environment
source .env.development

# Run specific tests quickly
./scripts/test-fast.sh database_connection

# Run all tests
./scripts/test-fast.sh
```

**Performance improvements:**
- Initial build: ~1 minute (from ~3 minutes)
- Incremental builds: ~0.5 seconds (from ~30 seconds)  
- Small changes: ~2.6 seconds (from ~60 seconds)

For detailed optimization information, see [Rust Build Optimization Guide](docs/rust-build-optimization.md).

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
