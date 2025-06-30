# 백엔드 핵심 동작 설계 (Sequence Diagram)

이 문서는 rMatterCertis 애플리케이션 백엔드의 주요 동작 흐름을 시퀀스 다이어그램을 통해 설명합니다. 백엔드는 Rust 언어와 Tauri 프레임워크를 기반으로 구축되었으며, 프론트엔드(React)와 `tauri::command`를 통해 통신하는 구조를 가집니다.

## 1. 애플리케이션 시작 (Application Startup)

사용자가 애플리케이션을 실행하면, Rust 진입점(`main.rs`)이 Tauri 애플리케이션을 빌드하고 실행합니다. 이 과정에서 데이터베이스 연결, 상태 관리 설정, 프론트엔드와 통신할 커맨드 등록 등이 이루어집니다.

```mermaid
sequenceDiagram
    participant User as 사용자
    participant OS as 운영체제
    participant MainRs as "main.rs (Rust)"
    participant Tauri as "Tauri Runtime"
    participant Frontend as "Frontend (WebView)"

    User->>OS: 애플리케이션 실행
    OS->>MainRs: 프로세스 시작
    MainRs->>Tauri: Tauri 애플리케이션 빌드 및 설정
    note right of MainRs: - 데이터베이스 연결(Sqlite) 초기화<br>- AppState 설정<br>- invoke_handler로 커맨드 등록
    Tauri->>MainRs: 설정 기반으로 런타임 실행
    Tauri->>OS: 메인 윈도우 생성
    Tauri->>Frontend: index.html 로드 및 UI 렌더링
    Frontend-->>Tauri: UI 로드 완료, 이벤트 수신 대기
    Tauri-->>User: 애플리케이션 화면 표시
```

## 2. 데이터 크롤링 실행 (Crawling Execution)

크롤링은 이 애플리케이션의 핵심 기능입니다. 사용자가 프론트엔드 UI를 통해 크롤링을 요청하면, Tauri 커맨드를 통해 백엔드의 크롤링 유스케이스가 실행됩니다.

- **파일 경로 추적**: `src/components/CrawlingDashboard.tsx` -> `src/services/crawlingService.ts` -> `(Tauri API)` -> `src-tauri/src/commands_crawling.rs` -> `src-tauri/src/application/crawling_use_cases.rs`

```mermaid
sequenceDiagram
    participant FE_Component as "React 컴포넌트<br>(CrawlingDashboard)"
    participant FE_Service as "Tauri API 호출<br>(crawlingService.ts)"
    participant Rust_Command as "Rust Command<br>(commands_crawling.rs)"
    participant Rust_UseCase as "Application Use Case<br>(crawling_use_cases.rs)"
    participant Rust_Infra as "Infrastructure<br>(HTTP Client, DB)"
    participant Domain as "Domain Logic"

    FE_Component->>FE_Service: 크롤링 시작 요청 (ex: startCrawling)
    FE_Service->>Rust_Command: tauri.invoke('start_crawling', { ... })
    Rust_Command->>Rust_UseCase: execute_crawling(params) 호출
    note right of Rust_Command: Tauri 커맨드가<br>애플리케이션 계층의<br>유스케이스를 호출
    
    activate Rust_UseCase
    Rust_UseCase->>Domain: 비즈니스 로직 처리 요청
    Rust_UseCase->>Rust_Infra: 외부 사이트 HTTP 요청 (크롤링)
    Rust_Infra-->>Rust_UseCase: HTML 데이터 반환
    
    Rust_UseCase->>Rust_Infra: 크롤링 결과 데이터베이스에 저장
    Rust_Infra-->>Rust_UseCase: 저장 성공/실패 반환
    deactivate Rust_UseCase
    
    Rust_UseCase-->>Rust_Command: 크롤링 결과(성공/실패) 반환
    Rust_Command-->>FE_Service: Promise 결과 반환
    FE_Service-->>FE_Component: 크롤링 완료 및 결과 전달
    FE_Component->>FE_Component: UI 상태 업데이트 (결과 표시)
```

## 3. 통합 데이터 조회 (Integrated Data Query)

크롤링을 통해 수집된 데이터는 통합된 스키마를 통해 관리됩니다. 프론트엔드에서 이 데이터를 조회하는 요청 또한 Tauri 커맨드를 통해 처리됩니다.

- **파일 경로 추적**: `src/components/features/products/ProductList.tsx` -> `src/services/api.ts` -> `(Tauri API)` -> `src-tauri/src/commands_integrated.rs` -> `src-tauri/src/application/integrated_use_cases.rs`

```mermaid
sequenceDiagram
    participant FE_Component as "React 컴포넌트<br>(ProductList)"
    participant FE_Service as "Tauri API 호출<br>(api.ts)"
    participant Rust_Command as "Rust Command<br>(commands_integrated.rs)"
    participant Rust_UseCase as "Application Use Case<br>(integrated_use_cases.rs)"
    participant Rust_Infra as "Infrastructure<br>(Database Repository)"
    participant Database as "SQLite DB"

    FE_Component->>FE_Service: 제품 데이터 요청 (ex: getProducts)
    FE_Service->>Rust_Command: tauri.invoke('get_integrated_products', { ... })
    
    Rust_Command->>Rust_UseCase: find_products(params) 호출
    note right of Rust_Command: 데이터 조회 커맨드가<br>통합 유스케이스 호출
    
    activate Rust_UseCase
    Rust_UseCase->>Rust_Infra: DB에서 제품 데이터 조회 요청
    activate Rust_Infra
    Rust_Infra->>Database: SELECT * FROM products ...
    Database-->>Rust_Infra: 제품 데이터(raw)
    Rust_Infra-->>Rust_UseCase: DTO 등으로 변환된 데이터 반환
    deactivate Rust_Infra
    deactivate Rust_UseCase
    
    Rust_UseCase-->>Rust_Command: 조회 결과(제품 목록) 반환
    Rust_Command-->>FE_Service: Promise 결과 반환
    FE_Service-->>FE_Component: 데이터 조회 완료 및 결과 전달
    FE_Component->>FE_Component: UI 상태 업데이트 (데이터 테이블 렌더링)
```
