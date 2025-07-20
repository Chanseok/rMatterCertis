use crate::domain::matter_product::MatterProduct;
use crate::domain::product::{Product, ProductDetail, ProductWithDetails};
use anyhow::Result;

/// Adapter to convert between new MatterProduct model and existing database schema
pub struct ProductSchemaAdapter;

impl ProductSchemaAdapter {
    /// Convert MatterProduct to Product (for products table)
    pub fn matter_product_to_product(matter_product: &MatterProduct) -> Product {
        Product {
            url: matter_product.detail_url.clone(),
            manufacturer: Some(matter_product.company_name.clone()),
            model: Some(matter_product.product_name.clone()),
            certificate_id: Some(matter_product.certificate_id.clone()),
            page_id: matter_product.page_number,
            index_in_page: matter_product.position_in_page,
        }
    }

    /// Convert MatterProduct to ProductDetail (for product_details table)
    pub fn matter_product_to_product_detail(matter_product: &MatterProduct) -> ProductDetail {
        ProductDetail {
            url: matter_product.detail_url.clone(),
            page_id: matter_product.page_number,
            index_in_page: matter_product.position_in_page,
            id: matter_product.product_id.clone(),
            manufacturer: Some(matter_product.company_name.clone()),
            model: Some(matter_product.product_name.clone()),
            device_type: matter_product.device_type.clone(),
            certification_id: Some(matter_product.certificate_id.clone()),
            certification_date: matter_product.certified_date.map(|dt| dt.format("%m/%d/%Y").to_string()),
            software_version: None, // Not available in MatterProduct
            hardware_version: matter_product.hardware_version.clone(),
            vid: Self::parse_hex_to_int(&matter_product.vendor_id),
            pid: Self::parse_hex_to_int(&matter_product.product_id),
            family_sku: None, // Not available in MatterProduct
            family_variant_sku: None, // Not available in MatterProduct
            firmware_version: matter_product.firmware_version.clone(),
            family_id: None, // Not available in MatterProduct
            tis_trp_tested: matter_product.tis_trp_tested.map(|b| if b { "Yes".to_string() } else { "No".to_string() }),
            specification_version: matter_product.specification_version.clone(),
            transport_interface: matter_product.transport_interface.clone(),
            primary_device_type_id: matter_product.primary_device_type_id.clone(),
            application_categories: None, // Not available in MatterProduct, but could be derived from device_type
        }
    }

    /// Convert Product and ProductDetail back to MatterProduct
    pub fn legacy_to_matter_product(product: &Product, detail: Option<&ProductDetail>) -> MatterProduct {
        let detail = detail.unwrap_or(&ProductDetail {
            url: product.url.clone(),
            page_id: product.page_id,
            index_in_page: product.index_in_page,
            id: None,
            manufacturer: product.manufacturer.clone(),
            model: product.model.clone(),
            device_type: None,
            certification_id: product.certificate_id.clone(),
            certification_date: None,
            software_version: None,
            hardware_version: None,
            vid: None,
            pid: None,
            family_sku: None,
            family_variant_sku: None,
            firmware_version: None,
            family_id: None,
            tis_trp_tested: None,
            specification_version: None,
            transport_interface: None,
            primary_device_type_id: None,
            application_categories: None,
        });

        MatterProduct {
            id: None,
            certificate_id: product.certificate_id.clone().unwrap_or_default(),
            company_name: product.manufacturer.clone().unwrap_or_default(),
            product_name: product.model.clone().unwrap_or_default(),
            description: None,
            firmware_version: detail.firmware_version.clone(),
            hardware_version: detail.hardware_version.clone(),
            specification_version: detail.specification_version.clone(),
            product_id: detail.id.clone(),
            vendor_id: detail.vid.map(|v| format!("0x{:X}", v)),
            primary_device_type_id: detail.primary_device_type_id.clone(),
            transport_interface: detail.transport_interface.clone(),
            certified_date: Self::parse_date_string(&detail.certification_date),
            tis_trp_tested: detail.tis_trp_tested.as_ref().map(|s| s == "Yes"),
            compliance_document_url: None,
            program_type: "Matter".to_string(),
            device_type: detail.device_type.clone(),
            detail_url: product.url.clone(),
            listing_url: None,
            page_number: product.page_id,
            position_in_page: product.index_in_page,
            crawled_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    /// Parse hexadecimal string to integer
    fn parse_hex_to_int(hex_str: &Option<String>) -> Option<i32> {
        hex_str.as_ref().and_then(|s| {
            let cleaned = s.trim_start_matches("0x").trim_start_matches("0X");
            i32::from_str_radix(cleaned, 16).ok()
        })
    }

    /// Parse date string in various formats
    fn parse_date_string(date_str: &Option<String>) -> Option<chrono::DateTime<chrono::Utc>> {
        date_str.as_ref().and_then(|s| {
            // Try MM/DD/YYYY format first
            if let Ok(dt) = chrono::NaiveDate::parse_from_str(s, "%m/%d/%Y") {
                return Some(dt.and_hms_opt(0, 0, 0)?.and_utc());
            }
            
            // Try other common formats
            if let Ok(dt) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                return Some(dt.and_hms_opt(0, 0, 0)?.and_utc());
            }
            
            None
        })
    }

    /// Convert device type to application categories (best effort mapping)
    pub fn device_type_to_application_categories(device_type: &Option<String>) -> Option<String> {
        device_type.as_ref().map(|dt| {
            match dt.as_str() {
                "Light Bulb" | "Dimmable Light" | "Color Light" => r"["Light Bulb"]".to_string(),
                "Smart Plug" | "On/Off Plug In Unit" => r"["Smart Plug"]".to_string(),
                "Door Lock" => r"["Door Lock"]".to_string(),
                "Thermostat" => r"["Thermostat"]".to_string(),
                "Motion Sensor" | "Contact Sensor" => r"["Sensor"]".to_string(),
                _ => format!(r"["{dt}"]"),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_matter_product_to_product_conversion() {
        let matter_product = MatterProduct {
            id: None,
            certificate_id: "CSA12345".to_string(),
            company_name: "Test Company".to_string(),
            product_name: "Test Product".to_string(),
            description: Some("Test description".to_string()),
            firmware_version: Some("1.0".to_string()),
            hardware_version: Some("1.0".to_string()),
            specification_version: Some("1.1".to_string()),
            product_id: Some("0x1234".to_string()),
            vendor_id: Some("0x5678".to_string()),
            primary_device_type_id: Some("0x010A".to_string()),
            transport_interface: Some("Wi-Fi".to_string()),
            certified_date: Some(Utc::now()),
            tis_trp_tested: Some(true),
            compliance_document_url: None,
            program_type: "Matter".to_string(),
            device_type: Some("Light Bulb".to_string()),
            detail_url: "https://example.com/product".to_string(),
            listing_url: None,
            page_number: Some(1),
            position_in_page: Some(5),
            crawled_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let product = ProductSchemaAdapter::matter_product_to_product(&matter_product);
        assert_eq!(product.url, "https://example.com/product");
        assert_eq!(product.manufacturer, Some("Test Company".to_string()));
        assert_eq!(product.certificate_id, Some("CSA12345".to_string()));
        assert_eq!(product.page_id, Some(1));
        assert_eq!(product.index_in_page, Some(5));
    }

    #[test]
    fn test_hex_parsing() {
        assert_eq!(ProductSchemaAdapter::parse_hex_to_int(&Some("0x1234".to_string())), Some(0x1234));
        assert_eq!(ProductSchemaAdapter::parse_hex_to_int(&Some("1234".to_string())), Some(0x1234));
        assert_eq!(ProductSchemaAdapter::parse_hex_to_int(&None), None);
    }

    #[test]
    fn test_device_type_mapping() {
        assert_eq!(
            ProductSchemaAdapter::device_type_to_application_categories(&Some("Light Bulb".to_string())),
            Some(r"["Light Bulb"]".to_string())
        );
        assert_eq!(
            ProductSchemaAdapter::device_type_to_application_categories(&Some("Smart Plug".to_string())),
            Some(r"["Smart Plug"]".to_string())
        );
    }
}
