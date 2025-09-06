//! StageBatcher: 배치 정책 헬퍼 (액터가 아님)
//!
//! 목적: Chunking/동시성/타임아웃 등 배치 정책 결정을 StageActor 밖의 작은 모듈로 분리할 수 있는 훅을 제공.
//! 현재는 동작 변경 없이 그대로 통과시키는 기본 구현만 제공한다.

use std::time::Duration;

use crate::crawl_engine::actors::types::StageType;
use crate::crawl_engine::system_config::StageBatcherSettings;
use crate::crawl_engine::channels::types::StageItem;

/// 배치 실행 전 사전 계획 결과
#[derive(Debug, Clone)]
pub struct PlannedBatch {
    pub items: Vec<StageItem>,
    pub concurrency_limit: u32,
    pub overall_timeout: Duration,
}

/// 배치 정책 인터페이스(헬퍼)
pub trait StageBatcher: Send + Sync {
    fn plan(
        &self,
        _stage_type: &StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        overall_timeout: Duration,
    ) -> PlannedBatch;
}

/// 기본 정책: 입력을 변경하지 않고 그대로 통과
pub struct DefaultStageBatcher;

impl StageBatcher for DefaultStageBatcher {
    fn plan(
        &self,
        _stage_type: &StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        overall_timeout: Duration,
    ) -> PlannedBatch {
        PlannedBatch {
            items,
            concurrency_limit,
            overall_timeout,
        }
    }
}

/// 설정 기반 정책 배처 (현재는 pass-through, 설정만 보유)
pub struct ConfigurableStageBatcher {
    settings: StageBatcherSettings,
}

impl ConfigurableStageBatcher {
    #[must_use]
    pub fn from_settings(settings: StageBatcherSettings) -> Self {
        Self { settings }
    }
}

impl StageBatcher for ConfigurableStageBatcher {
    fn plan(
        &self,
        _stage_type: &StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        overall_timeout: Duration,
    ) -> PlannedBatch {
        // 설정 기반으로 최소한의 정책을 적용하되, 기본값은 무해한(pass-through) 값으로 유지된다.
        // - prefer_reverse_order: 아이템 처리 순서를 역전(페이지 기반 단계에서 유용)
        // - max_chunk_size: 동시성 상한을 cap 하여 과도한 동시 실행 방지(아이템 자체는 잘라내지 않음)
        // - enforce_timeout_ms: 전체 타임아웃을 더 엄격하게 강제(입력보다 짧은 경우에만 적용)

        // 아이템 순서 정책
        let mut planned_items = items;
        if self.settings.prefer_reverse_order {
            planned_items.reverse();
        }

        // 동시성 상한 정책 (0은 비활성화)
        let planned_concurrency = if self.settings.max_chunk_size > 0 {
            concurrency_limit.min(self.settings.max_chunk_size)
        } else {
            concurrency_limit
        };

        // 타임아웃 강제 정책(None이면 비활성화)
        let planned_timeout = if let Some(ms) = self.settings.enforce_timeout_ms {
            let enforced = Duration::from_millis(ms);
            if enforced < overall_timeout { enforced } else { overall_timeout }
        } else {
            overall_timeout
        };

        PlannedBatch {
            items: planned_items,
            concurrency_limit: planned_concurrency,
            overall_timeout: planned_timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl_engine::channels::types::StageItem;

    fn mk_items(n: u32) -> Vec<StageItem> {
        (1..=n).map(StageItem::Page).collect()
    }

    #[test]
    fn test_reverse_order() {
        let settings = StageBatcherSettings { max_chunk_size: 0, prefer_reverse_order: true, enforce_timeout_ms: None };
        let batcher = ConfigurableStageBatcher::from_settings(settings);
    let out = batcher.plan(&StageType::ListPageCrawling, mk_items(3), 5, Duration::from_secs(10));
        let mut ids: Vec<u32> = vec![];
        for it in out.items {
            if let StageItem::Page(p) = it { ids.push(p); }
        }
        assert_eq!(ids, vec![3,2,1]);
    }

    #[test]
    fn test_concurrency_cap() {
        let settings = StageBatcherSettings { max_chunk_size: 3, prefer_reverse_order: false, enforce_timeout_ms: None };
        let batcher = ConfigurableStageBatcher::from_settings(settings);
    let out = batcher.plan(&StageType::ProductDetailCrawling, mk_items(5), 10, Duration::from_secs(5));
        assert_eq!(out.concurrency_limit, 3);
    }

    #[test]
    fn test_timeout_enforcement_only_if_stricter() {
        let settings = StageBatcherSettings { max_chunk_size: 0, prefer_reverse_order: false, enforce_timeout_ms: Some(1500) };
        let batcher = ConfigurableStageBatcher::from_settings(settings);
        // Input overall_timeout 2s -> enforcement 1.5s should apply
        let out = batcher.plan(&StageType::DataValidation, mk_items(1), 2, Duration::from_secs(2));
        assert_eq!(out.overall_timeout, Duration::from_millis(1500));

        // Input overall_timeout 1s -> enforcement 1.5s should NOT shorten (keep 1s)
        let out2 = batcher.plan(&StageType::DataValidation, mk_items(1), 2, Duration::from_secs(1));
        assert_eq!(out2.overall_timeout, Duration::from_secs(1));
    }
}
