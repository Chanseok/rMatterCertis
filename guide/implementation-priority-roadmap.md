# Implementation Priority Decision and Roadmap

## Analysis: Configuration Management vs Crawling Engine Priority

### Current State Assessment

After thorough analysis of both the configuration management requirements and the crawling engine design, here's the strategic decision on implementation priority:

## **DECISION: Prioritize Crawling Engine Core Implementation**

### Rationale

1. **Clear Technical Requirements**: The CSA-IoT page structure analysis provides concrete, well-defined requirements for the crawling engine
2. **Immediate Value**: A working crawling engine provides immediate, tangible value by collecting real Matter product data
3. **Foundation for Configuration**: Understanding the actual crawling workload helps design more effective configuration management
4. **Simpler Dependencies**: Crawling engine can start with basic configuration, while advanced config management requires complex UI/UX design

### Phase 1: Core Crawling Engine (Priority 1)
**Timeline: 1-2 weeks**

#### 1.1 Basic Infrastructure (Days 1-2)
- [ ] Create `MatterProductsCrawler` struct and core methods
- [ ] Implement HTTP client with rate limiting and retry logic
- [ ] Design basic error handling and logging framework
- [ ] Create database schema and repository for Matter products

#### 1.2 Listing Page Crawler (Days 3-4)
- [ ] Implement pagination handling (476+ pages)
- [ ] Extract product cards from listing pages
- [ ] Parse basic product information (name, company, certificate ID)
- [ ] Store listing data and extract detail page URLs

#### 1.3 Detail Page Crawler (Days 5-7)
- [ ] Implement product detail page fetching
- [ ] Parse technical specifications (firmware version, hardware version, etc.)
- [ ] Extract compliance documents and additional metadata
- [ ] Update database with complete product information

#### 1.4 Parallel Processing & Optimization (Days 8-10)
- [ ] Implement concurrent page fetching with proper rate limiting
- [ ] Add progress tracking and resumable crawling
- [ ] Optimize database operations (bulk inserts, updates)
- [ ] Add comprehensive error handling and retry mechanisms

### Phase 2: Basic Configuration Integration (Priority 2)
**Timeline: 3-5 days**

#### 2.1 Configuration Interface
- [ ] Create basic crawling configuration struct
- [ ] Implement file-based configuration (TOML/JSON)
- [ ] Add crawling parameters (rate limits, retry policy, filter options)
- [ ] Integrate configuration loading into crawler

#### 2.2 Frontend Integration
- [ ] Add basic crawling controls to frontend
- [ ] Implement start/stop/progress display
- [ ] Show basic crawling statistics and status
- [ ] Add simple error reporting

### Phase 3: Advanced Configuration Management (Priority 3)
**Timeline: 2-3 weeks**

#### 3.1 State Management
- [ ] Implement configuration locking and validation
- [ ] Add real-time configuration updates
- [ ] Create configuration change history
- [ ] Implement rollback capabilities

#### 3.2 Advanced UI/UX
- [ ] Design comprehensive settings interface
- [ ] Add configuration wizards and validation
- [ ] Implement user feedback and guidance
- [ ] Add advanced scheduling and automation options

## Technical Implementation Details

### Phase 1 Architecture

```rust
// Core crawler structure
pub struct MatterProductsCrawler {
    client: HttpClient,
    config: CrawlerConfig,
    db_pool: Arc<SqlitePool>,
    rate_limiter: RateLimiter,
}

// Configuration for crawling engine
#[derive(Serialize, Deserialize, Clone)]
pub struct CrawlerConfig {
    pub base_url: String,
    pub rate_limit_ms: u64,
    pub max_concurrent_requests: usize,
    pub max_retries: u32,
    pub timeout_seconds: u64,
    pub filters: CrawlerFilters,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CrawlerFilters {
    pub program_type: Vec<String>, // ["Matter"]
    pub product_type: Vec<String>, // ["Product"]
    pub max_pages: Option<u32>,
    pub company_filter: Option<Vec<String>>,
}
```

### Database Migration Strategy

```sql
-- Add to existing migrations or create new migration
CREATE TABLE matter_products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    certificate_id TEXT UNIQUE NOT NULL,
    company_name TEXT NOT NULL,
    product_name TEXT NOT NULL,
    description TEXT,
    firmware_version TEXT,
    hardware_version TEXT,
    specification_version TEXT,
    product_id TEXT,
    vendor_id TEXT,
    primary_device_type_id TEXT,
    transport_interface TEXT,
    certified_date DATE,
    tis_trp_tested BOOLEAN DEFAULT FALSE,
    compliance_document_url TEXT,
    program_type TEXT DEFAULT 'Matter',
    device_type TEXT,
    detail_url TEXT UNIQUE,
    listing_url TEXT,
    crawled_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_matter_certificate_id ON matter_products(certificate_id);
CREATE INDEX idx_matter_company_name ON matter_products(company_name);
CREATE INDEX idx_matter_specification_version ON matter_products(specification_version);
```

### Integration Points

1. **Database**: Extend existing infrastructure/database_connection.rs
2. **Commands**: Add new Tauri commands for crawling control
3. **Frontend**: Integrate with existing crawling dashboard components
4. **Configuration**: Start with basic file-based config, evolve to advanced management

## Success Criteria

### Phase 1 Success Metrics
- [ ] Successfully crawl and store data from 100+ Matter products
- [ ] Handle pagination across multiple pages without errors
- [ ] Maintain stable performance with rate limiting
- [ ] Provide accurate progress tracking and error reporting

### Phase 2 Success Metrics  
- [ ] Configuration changes apply without restart
- [ ] Frontend shows real-time crawling progress
- [ ] Users can start/stop/configure crawling through UI
- [ ] Basic error handling and user feedback working

### Phase 3 Success Metrics
- [ ] Advanced configuration management with validation
- [ ] Comprehensive user experience with guidance
- [ ] Configuration change history and rollback
- [ ] Production-ready scheduling and automation

## Risk Mitigation

1. **Rate Limiting**: Implement conservative defaults to avoid being blocked
2. **Data Quality**: Add validation for extracted data fields
3. **Error Recovery**: Ensure crawling can resume from interruptions
4. **Configuration Drift**: Version control configuration schema changes

## Next Actions

1. **Immediate**: Start implementing Phase 1.1 (Basic Infrastructure)
2. **Week 1**: Complete listing page crawler with pagination
3. **Week 2**: Implement detail page crawler and parallel processing
4. **Week 3**: Begin basic configuration integration

This approach provides a working, valuable system quickly while building toward comprehensive configuration management.
