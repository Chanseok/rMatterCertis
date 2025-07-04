# URL 구조 변경 확인

새로운 URL 구조가 적용되었습니다:

## 변경 전 (예시)
- 페이지 1: `https://csa-iot.org/csa-iot_products/?p_keywords=&p_type%5B%5D=14&p_program_type%5B%5D=1049&p_certificate=&p_family=&p_firmware_ver=`
- 페이지 2+: `https://csa-iot.org/csa-iot_products/?p_keywords=&p_type%5B%5D=14&p_program_type%5B%5D=1049&p_certificate=&p_family=&p_firmware_ver=&page=464`

## 변경 후 (새로운 구조)
- 페이지 1: `https://csa-iot.org/csa-iot_products?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver`
- 페이지 464: `https://csa-iot.org/csa-iot_products/page/464?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver`
- 페이지 480: `https://csa-iot.org/csa-iot_products/page/480?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver`

## 주요 변경사항
1. 페이지 번호가 URL 경로에 포함: `/page/{페이지번호}`
2. 쿼리 파라미터 형식 변경: `p_type[0]` 대신 `p_type%5B0%5D`
3. 고정된 쿼리 스트링이 항상 URL 끝에 붙음
4. 페이지 1은 `/page/1` 없이 기본 URL 사용

## 코드 변경 위치
- `src-tauri/src/infrastructure/config.rs`
  - `PRODUCTS_BASE` 상수 추가
  - `MATTER_QUERY_PARAMS` 상수 추가  
  - `matter_products_page_url()` 함수 수정
  - `matter_products_page_url_simple()` 함수 수정
