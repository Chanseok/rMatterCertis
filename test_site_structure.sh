#!/bin/bash
# Test script to analyze CSA-IoT website structure

echo "=== CSA-IoT 사이트 구조 분석 ==="

# 첫 번째 페이지의 HTML을 가져와서 제품 링크 구조 확인
curl -s "https://csa-iot.org/csa-iot_products/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver" \
  | grep -E "href.*csa_product" \
  | head -10 \
  | sed 's/.*href="\([^"]*\)".*/\1/' \
  | sort | uniq

echo ""
echo "=== article.type-product 확인 ==="
curl -s "https://csa-iot.org/csa-iot_products/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver" \
  | grep -A 5 -B 5 "article.*type-product" \
  | head -20
