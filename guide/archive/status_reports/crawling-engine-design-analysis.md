# Crawling Engine Design Analysis

## CSA-IoT Certified Products Page Structure Analysis

### Overview
Based on the analysis of the CSA-IoT certified products page with Matter filter applied (`https://csa-iot.org/csa-iot_products/?p_keywords=&p_type%5B%5D=14&p_program_type%5B%5D=1049&p_certificate=&p_family=&p_firmware_ver=`), this document outlines the technical requirements for the crawling engine.

### URL Structure and Filtering Parameters

#### Base URL
- Base: `https://csa-iot.org/csa-iot_products/`

#### Filter Parameters
- `p_keywords`: Search keywords (empty for all)
- `p_type[]`: Product type filter (14 = Product, as opposed to Platform or Software Component)
- `p_program_type[]`: Program type filter (1049 = Matter)
- `p_certificate`: Certification ID filter
- `p_family`: Family ID filter  
- `p_firmware_ver`: Firmware version filter

#### Pagination Structure
- Pagination follows pattern: `/page/{page_number}/`
- Example: `https://csa-iot.org/csa-iot_products/page/2/?...filters...`
- The page analysis shows pagination goes up to at least page 476, indicating thousands of products

### Product Card Structure

#### Individual Product Information
Each product appears to have the following structure:
- **Company Name**: Manufacturer (e.g., "ZHENGZHOU DEWENWILS NETWORK TECHNOLOGY CO., LTD.")
- **Product Name**: Device name (e.g., "Dewenwils Smart Device")
- **Program Type**: "Matter" for Matter-certified products
- **Certificate ID**: Unique identifier (e.g., "CSA2542CMAT45950-24")
- **Description**: Brief product description
- **Detail Link**: Link to individual product page (e.g., `/csa_product/dewenwils-smart-device/`)

#### Sample Product Entry Pattern
```
[COMPANY NAME] PRODUCT NAME Matter Certificate ID: CERTIFICATE_ID 
Product description text... Learn More](link-to-detail-page)
```

### Filter Options Available

#### Product Type Options
- Platform
- Product  
- Software Component

#### Device Type Categories (Matter-specific)
Extensive list including:
- Smart home devices (lights, switches, sensors)
- Appliances (dishwasher, laundry, air purifier)
- Security devices (door locks, cameras, alarms)
- HVAC equipment (thermostats, air conditioners)
- And many more specialized device types

#### Program Type
- Green Power Device
- Manufacturer Specific
- **Matter** (primary focus)
- Zigbee variants (3.0, Home Automation, etc.)

#### Company Filter
- Extensive dropdown with hundreds of manufacturers
- Alphabetically sorted list

#### Specification Version
- 1.0, 1.1, 1.2, 1.3, 1.4

### Crawling Strategy Requirements

#### 1. Multi-Page Crawling
- Need to handle pagination (476+ pages for Matter products)
- Must extract "next page" links or construct pagination URLs
- Implement parallel page fetching with rate limiting

#### 2. Product Link Extraction
- Extract individual product detail page URLs from each listing page
- Pattern: `https://csa-iot.org/csa_product/{product-slug}/`

#### 3. Product Detail Page Crawling
- Follow each product link to extract detailed information
- May contain additional technical specifications not visible on listing page

#### 4. Data Extraction Points
**From Listing Pages:**
- Company name
- Product name
- Certificate ID
- Brief description
- Detail page URL

**From Detail Pages (to be analyzed):**
- Full technical specifications
- Device type classifications
- Compliance details
- Additional metadata

### Rate Limiting and Politeness
- Implement delays between requests (recommended: 1-2 seconds)
- Use concurrent requests with appropriate limits (max 3-5 simultaneous)
- Respect robots.txt and implement exponential backoff for errors

### Error Handling Requirements
- Handle HTTP errors (404, 500, timeout)
- Retry mechanism with exponential backoff
- Graceful degradation for missing data fields
- Logging of failed URLs for manual review

### Configuration Parameters
The crawling engine should support configurable parameters:
- **Filter Settings**: Program type, device type, company filters
- **Pagination Limits**: Max pages to crawl, starting page
- **Rate Limiting**: Request delay, concurrent request limits
- **Retry Policy**: Max retries, backoff multiplier
- **Output Format**: Database storage, JSON export options

### Product Detail Page Structure

#### Detailed Information Available
Based on analysis of product detail pages (example: https://csa-iot.org/csa_product/dewenwils-smart-device/):

**Core Product Information:**
- Product Name: "Dewenwils Smart Device"
- Manufacturer: "ZHENGZHOU DEWENWILS NETWORK TECHNOLOGY CO., LTD."
- Description: Product overview and capabilities

**Technical Specifications:**
- **Firmware Version**: e.g., "1"
- **Hardware Version**: e.g., "1" 
- **Certificate ID**: e.g., "CSA2542CMAT45950-24"
- **Certified Date**: e.g., "06/27/2025"
- **Product ID**: e.g., "0x0D02" (hexadecimal identifier)
- **Vendor ID**: e.g., "0x1552" (hexadecimal identifier)
- **Specification Version**: e.g., "1.1"
- **Transport Interface**: e.g., "Wi-Fi, Bluetooth"
- **Primary Device Type ID**: e.g., "0x010A"

**Additional Resources:**
- **Compliance Document**: Downloadable ZIP file with technical documentation
- **TIS/TRP Tested**: Boolean indicating test completion

**Related Products:**
- "More by [Company]" section showing other products from same manufacturer

### Database Schema Requirements
Based on the complete data structure from both listing and detail pages:

```sql
-- Enhanced matter_products table with all available fields
CREATE TABLE matter_products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    
    -- Basic Information
    certificate_id TEXT UNIQUE NOT NULL,
    company_name TEXT NOT NULL,
    product_name TEXT NOT NULL,
    description TEXT,
    
    -- Technical Specifications  
    firmware_version TEXT,
    hardware_version TEXT,
    specification_version TEXT,
    product_id TEXT, -- Hexadecimal ID
    vendor_id TEXT,  -- Hexadecimal ID
    primary_device_type_id TEXT, -- Hexadecimal ID
    transport_interface TEXT, -- Comma-separated list
    
    -- Certification Details
    certified_date DATE,
    tis_trp_tested BOOLEAN DEFAULT FALSE,
    compliance_document_url TEXT,
    
    -- Program Classification
    program_type TEXT DEFAULT 'Matter',
    device_type TEXT, -- May need to be extracted/inferred
    
    -- URLs and Metadata
    detail_url TEXT UNIQUE,
    listing_url TEXT,
    crawled_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    
    -- Indexing for efficient queries
    INDEX idx_certificate_id (certificate_id),
    INDEX idx_company_name (company_name),
    INDEX idx_specification_version (specification_version),
    INDEX idx_certified_date (certified_date)
);
```

### Implementation Priority
1. **Basic Listing Crawler**: Extract product cards from paginated listings
2. **Parallel Page Processing**: Implement concurrent page fetching
3. **Product Detail Crawler**: Follow links to extract detailed specs
4. **Database Integration**: Store and update product information
5. **Configuration Management**: Make crawling parameters configurable
6. **Monitoring & Logging**: Comprehensive error handling and progress tracking

### Next Steps
1. Analyze a sample product detail page to understand the complete data structure
2. Implement the basic listing page crawler with pagination support
3. Design the product detail page extraction logic
4. Integrate with the existing configuration management system
5. Add comprehensive error handling and logging
