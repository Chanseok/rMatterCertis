use serde::{Deserialize, Serialize};
use std::fmt;
use ts_rs::TS;

/// URL과 함께 페이지 위치 정보를 담는 구조체
/// ProductListCollector에서 ProductDetailCollector로 메타데이터를 전달하기 위해 사용
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProductUrl {
    /// 제품 상세 페이지 URL
    pub url: String,
    /// 이 제품이 발견된 리스트 페이지 번호
    pub page_id: i32,
    /// 해당 페이지 내에서의 순서 (0부터 시작)
    pub index_in_page: i32,
}

impl ProductUrl {
    /// 새로운 ProductUrl 생성
    pub fn new(url: String, page_id: i32, index_in_page: i32) -> Self {
        Self {
            url,
            page_id,
            index_in_page,
        }
    }

    /// URL만 추출 (기존 코드와의 호환성을 위해)
    pub fn get_url(&self) -> &str {
        &self.url
    }

    /// 메타데이터를 튜플로 반환
    pub fn get_position(&self) -> (i32, i32) {
        (self.page_id, self.index_in_page)
    }
}

impl From<ProductUrl> for String {
    fn from(product_url: ProductUrl) -> Self {
        product_url.url
    }
}

impl AsRef<str> for ProductUrl {
    fn as_ref(&self) -> &str {
        &self.url
    }
}

impl fmt::Display for ProductUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (page: {}, index: {})",
            self.url, self.page_id, self.index_in_page
        )
    }
}
