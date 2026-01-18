//! 性能监控模块
//!
//! 提供执行时间监控功能，用于识别和诊断性能瓶颈。

use log::{debug, warn};
use std::time::Instant;

/// 性能监控器
#[derive(Debug, Clone)]
pub struct PerfMonitor {
    /// 监控器名称
    name: String,
    /// 开始时间
    start_time: Option<Instant>,
    /// 阈值（毫秒），超过此阈值会记录警告
    warning_threshold_ms: u128,
}

impl PerfMonitor {
    /// 创建一个新的性能监控器
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            start_time: None,
            warning_threshold_ms: 100, // 默认100毫秒阈值
        }
    }

    /// 设置警告阈值（毫秒）
    pub fn with_warning_threshold(mut self, threshold_ms: u128) -> Self {
        self.warning_threshold_ms = threshold_ms;
        self
    }

    /// 开始计时
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
    }

    /// 结束计时并记录结果
    pub fn end(&self) -> u128 {
        if let Some(start) = self.start_time {
            let duration = start.elapsed();
            let ms = duration.as_millis();

            if ms > self.warning_threshold_ms {
                warn!(
                    "[PERF WARNING] {} 执行时间: {}ms (超过阈值 {}ms)",
                    self.name, ms, self.warning_threshold_ms
                );
            } else {
                debug!("[PERF] {} 执行时间: {}ms", self.name, ms);
            }

            ms
        } else {
            warn!("[PERF ERROR] {} 监控器未启动", self.name);
            0
        }
    }

    /// 执行一个闭包并监控其执行时间
    #[allow(unused)]
    pub fn measure<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.start();
        let result = f();
        self.end();
        result
    }
}

/// 创建一个性能监控器并开始计时
#[macro_export]
macro_rules! perf_start {
    ($name:expr) => {{
        let mut monitor = $crate::tui::perf_monitor::PerfMonitor::new($name);
        monitor.start();
        monitor
    }};
    ($name:expr, $threshold:expr) => {{
        let mut monitor =
            $crate::tui::perf_monitor::PerfMonitor::new($name).with_warning_threshold($threshold);
        monitor.start();
        monitor
    }};
}

/// 结束性能监控并记录结果
#[macro_export]
macro_rules! perf_end {
    ($monitor:expr) => {{
        $monitor.end();
    }};
}

/// 监控一个代码块的执行时间
#[macro_export]
macro_rules! perf_block {
    ($name:expr, $block:block) => {{
        let mut monitor = $crate::tui::perf_monitor::PerfMonitor::new($name);
        monitor.start();
        let result = { $block };
        monitor.end();
        result
    }};
    ($name:expr, $threshold:expr, $block:block) => {{
        let mut monitor =
            $crate::tui::perf_monitor::PerfMonitor::new($name).with_warning_threshold($threshold);
        monitor.start();
        let result = { $block };
        monitor.end();
        result
    }};
}
