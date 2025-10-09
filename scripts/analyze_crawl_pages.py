#!/usr/bin/env python3
"""
좌표 갱신/크롤링 로그 분석 스크립트

로그 파일에서 페이지별 제품 수집 결과를 분석하여:
- 총 처리된 페이지 수
- 완전/불완전 페이지 통계
- 페이지 번호 범위 및 base 확인
- 제품 수집 성공률

Usage:
    python3 scripts/analyze_crawl_pages.py [log_file_path]
    
Examples:
    python3 scripts/analyze_crawl_pages.py
    python3 scripts/analyze_crawl_pages.py src-tauri/target/debug/logs/back_front.log
"""

import re
import sys
from pathlib import Path

def analyze_log(log_file_path: str):
    """로그 파일을 분석하여 페이지별 제품 수집 결과를 출력"""
    
    # 패턴: 📄 Page 123 (page_123): 12/12 products collected ✅ (retry: 0)
    pattern = r'📄 Page (\d+) \(page_\d+\): (\d+)/(\d+) products collected'
    
    pages_data = []
    incomplete_pages = []
    
    try:
        with open(log_file_path, 'r', encoding='utf-8') as f:
            for line in f:
                match = re.search(pattern, line)
                if match:
                    page_num = int(match.group(1))
                    collected = int(match.group(2))
                    expected = int(match.group(3))
                    
                    pages_data.append({
                        'page': page_num,
                        'collected': collected,
                        'expected': expected,
                        'complete': collected == expected
                    })
                    
                    if collected != expected:
                        incomplete_pages.append(f"Page {page_num}: {collected}/{expected}")
    
    except FileNotFoundError:
        print(f"❌ 로그 파일을 찾을 수 없습니다: {log_file_path}")
        sys.exit(1)
    except Exception as e:
        print(f"❌ 로그 파일 읽기 실패: {e}")
        sys.exit(1)
    
    if not pages_data:
        print(f"⚠️  로그 파일에서 페이지 수집 데이터를 찾을 수 없습니다.")
        print(f"   파일: {log_file_path}")
        sys.exit(0)
    
    # 통계 계산
    total_pages = len(pages_data)
    pages_12 = sum(1 for p in pages_data if p['expected'] == 12 and p['complete'])
    pages_3 = sum(1 for p in pages_data if p['expected'] == 3 and p['complete'])
    pages_other = sum(1 for p in pages_data if p['expected'] not in [3, 12] and p['complete'])
    pages_incomplete = len(incomplete_pages)
    
    # 페이지 번호 범위
    min_page = min(p['page'] for p in pages_data)
    max_page = max(p['page'] for p in pages_data)
    
    # 총 제품 수 계산
    total_products = sum(p['collected'] for p in pages_data)
    expected_products = sum(p['expected'] for p in pages_data)
    
    # 결과 출력
    print(f"\n📊 좌표 갱신/크롤링 분석 결과")
    print(f"{'='*60}")
    print(f"📁 로그 파일: {log_file_path}")
    print(f"\n📈 페이지 통계:")
    print(f"  총 처리된 페이지: {total_pages}")
    print(f"  페이지 범위: {min_page} ~ {max_page}")
    
    print(f"\n✅ 완전 페이지:")
    print(f"  12개 제품 페이지: {pages_12}")
    if pages_3 > 0:
        print(f"  3개 제품 페이지 (마지막): {pages_3}")
    if pages_other > 0:
        print(f"  기타 제품 페이지: {pages_other}")
    
    print(f"\n📦 제품 통계:")
    print(f"  총 수집된 제품: {total_products:,}")
    print(f"  예상 제품 수: {expected_products:,}")
    if total_products == expected_products:
        print(f"  ✅ 수집률: 100% (완벽!)")
    else:
        collection_rate = (total_products / expected_products * 100) if expected_products > 0 else 0
        print(f"  ⚠️  수집률: {collection_rate:.1f}%")
    
    if pages_incomplete > 0:
        print(f"\n❌ 불완전 페이지: {pages_incomplete}")
        print(f"\n⚠️  불완전 페이지 목록:")
        for p in incomplete_pages[:20]:
            print(f"  - {p}")
        if len(incomplete_pages) > 20:
            print(f"  ... 외 {len(incomplete_pages)-20}개")
    else:
        print(f"\n✅ 불완전 페이지: 0")
        print(f"✅ 모든 페이지에서 예상된 개수의 제품을 성공적으로 수집!")
    
    # 페이지 번호 기준 확인
    print(f"\n📍 페이지 번호 기준:")
    print(f"  가장 작은 페이지: {min_page}")
    print(f"  가장 큰 페이지: {max_page}")
    
    if min_page == 0:
        print(f"  ⚠️  페이지 번호가 0부터 시작 (0-based)")
        print(f"      → 물리 페이지는 1-based이므로 주의 필요!")
    elif min_page == 1:
        print(f"  ✅ 페이지 번호가 1부터 시작 (1-based, 정상)")
    else:
        print(f"  ℹ️  페이지 번호가 {min_page}부터 시작 (부분 크롤링)")
    
    print()  # 마지막 빈 줄

def main():
    """메인 함수"""
    # 기본 로그 파일 경로
    default_log = "src-tauri/target/debug/logs/back_front.log"
    
    # 명령줄 인자로 로그 파일 경로 받기
    if len(sys.argv) > 1:
        log_file = sys.argv[1]
    else:
        log_file = default_log
    
    # 상대 경로를 절대 경로로 변환
    log_path = Path(log_file)
    if not log_path.is_absolute():
        log_path = Path.cwd() / log_path
    
    analyze_log(str(log_path))

if __name__ == "__main__":
    main()
