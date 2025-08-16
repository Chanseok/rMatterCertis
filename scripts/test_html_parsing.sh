#!/bin/bash

# CSA-IoT 사이트에서 실제 HTML을 가져와 파싱 테스트
echo "=== Matter Certis HTML Parsing Test ==="
echo "Testing CSA-IoT site structure..."

# 1페이지 테스트
echo "1. Testing page 1..."
curl -s "https://csa-iot.org/csa-iot_products/page/1/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver" > /tmp/test_page1.html

# 제품 개수 확인
product_count=$(grep -c "div.post-feed article.type-product" /tmp/test_page1.html)
echo "   Products found with 'div.post-feed article.type-product': $product_count"

# 실제 article 요소 확인
article_count=$(grep -c "<article" /tmp/test_page1.html)
echo "   Total article elements: $article_count"

# 제품 링크 확인
product_links=$(grep -o "/csa_product/[^"]*" /tmp/test_page1.html | wc -l)
echo "   Product links found: $product_links"

# 476페이지 테스트
echo "2. Testing page 476..."
curl -s "https://csa-iot.org/csa-iot_products/page/476/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver" > /tmp/test_page476.html

# 제품 개수 확인
product_count_476=$(grep -c "div.post-feed article.type-product" /tmp/test_page476.html)
echo "   Products found on page 476: $product_count_476"

# 482페이지 테스트 (실제 마지막 페이지)
echo "3. Testing page 482..."
curl -s "https://csa-iot.org/csa-iot_products/page/482/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver" > /tmp/test_page482.html

# 제품 개수 확인
product_count_482=$(grep -c "div.post-feed article.type-product" /tmp/test_page482.html)
echo "   Products found on page 482: $product_count_482"

# 실제 마지막 페이지 확인
echo "4. Testing pages beyond 482..."
curl -s "https://csa-iot.org/csa-iot_products/page/483/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver" > /tmp/test_page483.html

product_count_483=$(grep -c "div.post-feed article.type-product" /tmp/test_page483.html)
echo "   Products found on page 483: $product_count_483"

echo "=== Test Results ==="
echo "Page 1: $product_count products"
echo "Page 476: $product_count_476 products"
echo "Page 482: $product_count_482 products"
echo "Page 483: $product_count_483 products"

if [ $product_count_482 -gt 0 ] && [ $product_count_483 -eq 0 ]; then
    echo "✅ Page 482 appears to be the actual last page!"
else
    echo "❓ Need to investigate further..."
fi

# 페이지네이션 분석
echo "5. Analyzing pagination on page 476..."
grep -o "page-numbers[^>]*>[0-9]*" /tmp/test_page476.html | sort -n | tail -10

echo "=== Test Complete ==="
