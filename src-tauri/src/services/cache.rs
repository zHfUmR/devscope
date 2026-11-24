//! 检测结果缓存。
//!
//! 总览、环境详情和诊断报告会重复触发同一批检测。这里用「按类型分槽 + TTL」的
//! 小缓存复用结果：
//!
//! - 默认 30 秒内复用；
//! - 用户点「重新检测」时传 `force = true` 绕过缓存；
//! - 每类检测独立加锁，某一类慢不会阻塞其他类。
//!
//! 缓存不落盘，退出应用即失效。检测失败的结果也会缓存，避免在 TTL 内反复
//! 启动注定失败的外部命令；手动刷新可以立即重试。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::models::android::AndroidEnvironment;
use crate::models::environment::FlutterEnvironment;
use crate::models::ios::IosEnvironment;
use crate::models::system::SystemInfo;
use crate::models::toolchain::ToolchainInfo;
use crate::services::{android, flutter, ios, system, toolchain};

/// 默认缓存有效期。
pub const DEFAULT_TTL: Duration = Duration::from_secs(30);

/// 单个检测结果的缓存槽。
#[derive(Debug)]
struct Slot<T> {
    captured_at: Option<Instant>,
    value: Option<T>,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self {
            captured_at: None,
            value: None,
        }
    }
}

/// 线程安全的单值缓存。
#[derive(Debug)]
pub struct Cache<T> {
    slot: Mutex<Slot<T>>,
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Cache<T> {
    pub fn new() -> Self {
        Self {
            slot: Mutex::new(Slot::default()),
        }
    }

    /// 清空缓存；预留给将来的「全部刷新」入口。
    #[allow(dead_code)]
    pub fn clear(&self) {
        let mut slot = self.slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        slot.value = None;
        slot.captured_at = None;
    }
}

impl<T: Clone> Cache<T> {
    /// 命中且未过期时返回缓存值，否则执行 `compute` 并写入缓存。
    ///
    /// 锁在 `compute` 期间保持，同一类检测的并发调用会等待而不是重复启动进程。
    pub fn get_or_compute(&self, ttl: Duration, force: bool, compute: impl FnOnce() -> T) -> T {
        let mut slot = self.slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        if !force
            && let (Some(value), Some(captured_at)) = (&slot.value, slot.captured_at)
            && captured_at.elapsed() < ttl
        {
            return value.clone();
        }

        let value = compute();
        slot.value = Some(value.clone());
        slot.captured_at = Some(Instant::now());
        value
    }
}

/// 应用级共享检测缓存。
///
/// 每个字段用 `Arc<Cache<_>>` 包一层，方便克隆后 move 进 `spawn_blocking`。
#[derive(Debug, Clone, Default)]
pub struct DetectionCache {
    system: Arc<Cache<SystemInfo>>,
    flutter: Arc<Cache<FlutterEnvironment>>,
    android: Arc<Cache<AndroidEnvironment>>,
    ios: Arc<Cache<IosEnvironment>>,
    toolchains: Arc<Cache<Vec<ToolchainInfo>>>,
}

impl DetectionCache {
    pub fn system(&self, force: bool) -> SystemInfo {
        self.system.get_or_compute(DEFAULT_TTL, force, system::detect)
    }

    pub fn flutter(&self, force: bool) -> FlutterEnvironment {
        self.flutter.get_or_compute(DEFAULT_TTL, force, flutter::detect)
    }

    pub fn android(&self, force: bool) -> AndroidEnvironment {
        self.android.get_or_compute(DEFAULT_TTL, force, android::detect)
    }

    pub fn ios(&self, force: bool) -> IosEnvironment {
        self.ios.get_or_compute(DEFAULT_TTL, force, ios::detect)
    }

    pub fn toolchains(&self, force: bool) -> Vec<ToolchainInfo> {
        self.toolchains.get_or_compute(DEFAULT_TTL, force, toolchain::detect_all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn reuses_value_within_ttl() {
        let cache: Cache<usize> = Cache::new();
        let calls = AtomicUsize::new(0);

        let first = cache.get_or_compute(DEFAULT_TTL, false, || calls.fetch_add(1, Ordering::Relaxed));
        let second = cache.get_or_compute(DEFAULT_TTL, false, || calls.fetch_add(1, Ordering::Relaxed));

        assert_eq!(first, 0);
        assert_eq!(second, 0);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn force_bypasses_cache() {
        let cache: Cache<usize> = Cache::new();
        let calls = AtomicUsize::new(0);

        let first = cache.get_or_compute(DEFAULT_TTL, true, || calls.fetch_add(1, Ordering::Relaxed));
        let second = cache.get_or_compute(DEFAULT_TTL, true, || calls.fetch_add(1, Ordering::Relaxed));

        assert_eq!(first, 0);
        assert_eq!(second, 1);
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn zero_ttl_always_recomputes() {
        let cache: Cache<usize> = Cache::new();
        let calls = AtomicUsize::new(0);

        cache.get_or_compute(Duration::ZERO, false, || calls.fetch_add(1, Ordering::Relaxed));
        cache.get_or_compute(Duration::ZERO, false, || calls.fetch_add(1, Ordering::Relaxed));

        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn clear_invalidates_value() {
        let cache: Cache<usize> = Cache::new();
        let calls = AtomicUsize::new(0);

        cache.get_or_compute(DEFAULT_TTL, false, || calls.fetch_add(1, Ordering::Relaxed));
        cache.clear();
        let value = cache.get_or_compute(DEFAULT_TTL, false, || calls.fetch_add(1, Ordering::Relaxed));

        assert_eq!(value, 1);
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }
}
