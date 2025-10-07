# Database Snapshot - Before Excel Restore Test

**Date**: 2025-10-07
**Purpose**: 정상 크롤링 데이터 기록 (Excel 복원 기능 테스트 전 참조용)

## Database Statistics

- **Products**: 12 records
- **Product_Details**: 12 records
- **Device_Types**: 75 records
- **Vendors**: 350 records

## Sample Record: product_details (ID: p0594i05)

```
url                           = https://csa-iot.org/csa_product/smart-central-control-gateway/
page_id                       = 594
index_in_page                 = 5
id                            = p0594i05
manufacturer                  = By Ultimate IOT (Shanghai) Technology Ltd.
model                         = Smart Central Control Gateway
device_type                   = Matter
certificate_id                = CSA259E5MAT47415-24
certification_date            = 2025-10-01
hardware_version              = 1
firmware_version              = 1
specification_version         = 1.3
vid                           = 5534
pid                           = 5
family_sku                    = YQ02-1-PA6
family_variant_sku            = WF09-1-PA6-U4
family_id                     = FAM226349
transport_interface           = Ethernet
primary_device_type_ids       = [14]
application_categories        = (empty)
created_at                    = 2025-10-07T05:55:41.489113+00:00
updated_at                    = 2025-10-07T05:55:41.489113+00:00
certification_date_normalized = (empty)
```

## Sample Record: product_details (ID: p0594i06)

```
url                           = https://csa-iot.org/csa_product/super-intelligent-server-2/
page_id                       = 594
index_in_page                 = 6
id                            = p0594i06
manufacturer                  = By Ultimate IOT (Shanghai) Technology Ltd.
model                         = Super Intelligent Server
device_type                   = Matter
certificate_id                = CSA259E4MAT47414-24
certification_date            = 2025-10-01
hardware_version              = 1
firmware_version              = 1
specification_version         = 1.3
vid                           = 5534
pid                           = 4
family_sku                    = YQ02-1-PA6
family_variant_sku            = U3011507
family_id                     = FAM226349
transport_interface           = Ethernet
primary_device_type_ids       = [14]
application_categories        = (empty)
created_at                    = 2025-10-07T05:55:41.489731+00:00
updated_at                    = 2025-10-07T05:55:41.489731+00:00
certification_date_normalized = (empty)
```

## Sample Record: product_details (ID: p0594i11)

```
url                           = https://csa-iot.org/csa_product/integrated-shutter-controller/
page_id                       = 594
index_in_page                 = 11
id                            = p0594i11
manufacturer                  = By Trust International BV
model                         = Integrated Shutter Controller
device_type                   = Matter
certificate_id                = CSA259DDMAT47407-24
certification_date            = 2025-10-01
hardware_version              = 010-0W4JLR2000000-00000
firmware_version              = 1
specification_version         = 1.4
vid                           = 4724
pid                           = 10904
family_sku                    = (empty)
family_variant_sku            = (empty)
family_id                     = (empty)
transport_interface           = Wi-Fi, Bluetooth
primary_device_type_ids       = [515]
application_categories        = (empty)
created_at                    = 2025-10-07T05:55:41.489877+00:00
updated_at                    = 2025-10-07T05:55:41.489877+00:00
certification_date_normalized = (empty)
```

## Field Type Notes

### Numeric Fields
- `page_id`: INTEGER (594)
- `index_in_page`: INTEGER (5, 6, 11)
- `vid`: INTEGER (5534, 4724)
- `pid`: INTEGER (5, 4, 10904)

### Text Fields
- `url`: TEXT (full URL)
- `id`: TEXT (p0594i05 format)
- `manufacturer`: TEXT (company name)
- `model`: TEXT (product model)
- `device_type`: TEXT (typically "Matter")
- `certificate_id`: TEXT (CSA certification ID)
- `certification_date`: DATE (2025-10-01)
- `hardware_version`: TEXT (version string or number)
- `firmware_version`: TEXT (version number)
- `specification_version`: TEXT (1.3, 1.4)
- `family_sku`: TEXT (optional)
- `family_variant_sku`: TEXT (optional)
- `family_id`: TEXT (optional, e.g., FAM226349)
- `transport_interface`: TEXT (e.g., "Ethernet", "Wi-Fi, Bluetooth")
- `primary_device_type_ids`: TEXT (JSON array format, e.g., [14], [515])
- `application_categories`: TEXT (usually empty in samples)

### Timestamp Fields
- `created_at`: DATETIME (ISO 8601 format with timezone)
- `updated_at`: DATETIME (ISO 8601 format with timezone)

## Next Steps

1. ✅ Excel 내보내기 실행 (full_database_YYYYMMDDHHMMSS.xlsx)
2. ⏳ 로컬 DB 전체 삭제 실행
3. ⏳ Excel 파일로 복원 실행
4. ⏳ 복원 후 데이터 비교 (필드별 정확성 검증)

## Expected Excel File Structure

### Sheet: product_details
22 columns in order:
1. url
2. page_id (NUMBER)
3. index_in_page (NUMBER)
4. id
5. manufacturer
6. model
7. device_type
8. certificate_id
9. certification_date
10. hardware_version
11. firmware_version
12. specification_version
13. vid (NUMBER)
14. pid (NUMBER)
15. family_sku
16. family_variant_sku
17. family_id
18. transport_interface
19. primary_device_type_ids
20. application_categories
21. created_at
22. updated_at
