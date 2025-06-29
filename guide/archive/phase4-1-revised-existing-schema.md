# Phase 4.1 Revised: Leverage Existing Database Schema

## Decision: Use Existing Production-Proven Schema

Based on analysis of the existing database with 5,582 Matter products, we'll use the current schema instead of creating a new one.

### Current Schema Structure (Keeping As-Is)

```sql
-- Product listing (basic info from listing pages)
CREATE TABLE products (
    url TEXT PRIMARY KEY,
    manufacturer TEXT,
    model TEXT,
    certificateId TEXT,
    pageId INTEGER,
    indexInPage INTEGER
);

-- Product details (complete specs from detail pages)  
CREATE TABLE product_details (
    url TEXT PRIMARY KEY,
    pageId INTEGER,
    indexInPage INTEGER,
    id TEXT,
    manufacturer TEXT,
    model TEXT,
    deviceType TEXT,
    certificationId TEXT,
    certificationDate TEXT,
    softwareVersion TEXT,
    hardwareVersion TEXT,
    vid INTEGER,
    pid INTEGER,
    familySku TEXT,
    familyVariantSku TEXT,
    firmwareVersion TEXT,
    familyId TEXT,
    tisTrpTested TEXT,
    specificationVersion TEXT,
    transportInterface TEXT,
    primaryDeviceTypeId TEXT,
    applicationCategories TEXT
);

-- Vendor management
CREATE TABLE vendors (
    vendorId INTEGER PRIMARY KEY,
    vendorName TEXT,
    companyLegalName TEXT  
);
```

### Enhanced Domain Models for Existing Schema

#### 1. Product Domain Model
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub url: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub certificate_id: Option<String>,
    pub page_id: Option<i32>,
    pub index_in_page: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductDetail {
    pub url: String,
    pub page_id: Option<i32>,
    pub index_in_page: Option<i32>,
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub certification_id: Option<String>,
    pub certification_date: Option<String>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub vid: Option<i32>,
    pub pid: Option<i32>,
    pub family_sku: Option<String>,
    pub family_variant_sku: Option<String>,
    pub firmware_version: Option<String>,
    pub family_id: Option<String>,
    pub tis_trp_tested: Option<String>,
    pub specification_version: Option<String>,
    pub transport_interface: Option<String>,
    pub primary_device_type_id: Option<String>,
    pub application_categories: Option<String>,
}
```

#### 2. Repository Layer for Existing Schema
```rust
pub struct ProductRepository {
    pool: Arc<SqlitePool>,
}

impl ProductRepository {
    // Insert basic product from listing page
    pub async fn create_product(&self, product: &Product) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT OR REPLACE INTO products 
            (url, manufacturer, model, certificateId, pageId, indexInPage)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            product.url,
            product.manufacturer,
            product.model,
            product.certificate_id,
            product.page_id,
            product.index_in_page
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    // Insert detailed product specifications
    pub async fn create_product_detail(&self, detail: &ProductDetail) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT OR REPLACE INTO product_details 
            (url, pageId, indexInPage, id, manufacturer, model, deviceType,
             certificationId, certificationDate, softwareVersion, hardwareVersion,
             vid, pid, familySku, familyVariantSku, firmwareVersion, familyId,
             tisTrpTested, specificationVersion, transportInterface, 
             primaryDeviceTypeId, applicationCategories)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            detail.url,
            detail.page_id,
            detail.index_in_page,
            detail.id,
            detail.manufacturer,
            detail.model,
            detail.device_type,
            detail.certification_id,
            detail.certification_date,
            detail.software_version,
            detail.hardware_version,
            detail.vid,
            detail.pid,
            detail.family_sku,
            detail.family_variant_sku,
            detail.firmware_version,
            detail.family_id,
            detail.tis_trp_tested,
            detail.specification_version,
            detail.transport_interface,
            detail.primary_device_type_id,
            detail.application_categories
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    // Get products with pagination
    pub async fn get_products_paginated(&self, page: i32, limit: i32) -> Result<Vec<Product>> {
        let offset = (page - 1) * limit;
        let products = sqlx::query_as!(
            Product,
            r#"
            SELECT url, manufacturer, model, certificateId as certificate_id, 
                   pageId as page_id, indexInPage as index_in_page
            FROM products 
            ORDER BY pageId DESC, indexInPage ASC 
            LIMIT ? OFFSET ?
            "#,
            limit,
            offset
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(products)
    }

    // Get product with details
    pub async fn get_product_with_details(&self, url: &str) -> Result<Option<(Product, Option<ProductDetail>)>> {
        let product = sqlx::query_as!(
            Product,
            r#"
            SELECT url, manufacturer, model, certificateId as certificate_id,
                   pageId as page_id, indexInPage as index_in_page
            FROM products WHERE url = ?
            "#,
            url
        )
        .fetch_optional(&*self.pool)
        .await?;

        if let Some(product) = product {
            let detail = sqlx::query_as!(
                ProductDetail,
                r#"
                SELECT url, pageId as page_id, indexInPage as index_in_page, id,
                       manufacturer, model, deviceType as device_type, 
                       certificationId as certification_id, certificationDate as certification_date,
                       softwareVersion as software_version, hardwareVersion as hardware_version,
                       vid, pid, familySku as family_sku, familyVariantSku as family_variant_sku,
                       firmwareVersion as firmware_version, familyId as family_id,
                       tisTrpTested as tis_trp_tested, specificationVersion as specification_version,
                       transportInterface as transport_interface, primaryDeviceTypeId as primary_device_type_id,
                       applicationCategories as application_categories
                FROM product_details WHERE url = ?
                "#,
                url
            )
            .fetch_optional(&*self.pool)
            .await?;

            Ok(Some((product, detail)))
        } else {
            Ok(None)
        }
    }
}
```

### Implementation Benefits

1. **Zero Migration Risk**: Use existing 5,582 products immediately
2. **Proven Field Coverage**: All CSA-IoT fields already captured
3. **Performance Optimized**: INTEGER types for vid/pid
4. **Frontend Compatible**: Existing UI expects this schema
5. **Immediate Testing**: Can test crawler logic with real data

### Performance Enhancements

```sql
-- Add these indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_products_manufacturer ON products(manufacturer);
CREATE INDEX IF NOT EXISTS idx_products_certificateId ON products(certificateId);
CREATE INDEX IF NOT EXISTS idx_product_details_certificationId ON product_details(certificationId);
CREATE INDEX IF NOT EXISTS idx_product_details_deviceType ON product_details(deviceType);
```

## Next Implementation Steps

1. ✅ **Skip new migration creation** - use existing schema
2. 🔄 **Create domain models** for existing schema
3. 🔄 **Implement repository layer** for current tables
4. 🔄 **Build HTTP client and crawler** using proven structure
5. 🔄 **Test with existing 5,582 products** for validation

This approach maximizes development velocity while leveraging the proven, production-ready database structure.
