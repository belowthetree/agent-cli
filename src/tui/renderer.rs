use std::cmp::min;

use ratatui::{
    Frame,
    layout::Position,
    symbols::scrollbar,
    widgets::{Scrollbar, ScrollbarOrientation},
};

use crate::tui::app::App;

/// 渲染器，负责处理应用程序界面的渲染逻辑
pub struct Renderer;

impl Renderer {
    /// 渲染应用程序界面
    ///
    /// 将应用程序状态渲染到终端帧中，包括：
    /// - 消息块显示区域
    /// - 垂直滚动条
    /// - 文本输入区域
    /// - 光标位置
    ///
    /// 此方法根据当前滚动位置和窗口大小计算哪些消息块需要显示，
    /// 并处理部分消息块被截断的情况。
    pub fn render(app: &mut App, frame: &mut Frame<'_>) {
        let _perf_monitor = crate::perf_start!("Renderer::render", 50);

        let mut area = frame.area();
        // 计算滚动条区域，减去滚动条宽度
        let mut scroll_area = area;
        scroll_area.x = area.width - 1;
        scroll_area.width = 1;
        area.width -= 1;
        app.width = area.width;
        app.refresh();
        // 先计算输入区域
        let mut input_area = area;
        input_area.y = area.height - app.input.height();
        area.height -= app.input.height();
        app.window_height = area.height;
        // 绘制光标
        frame.set_cursor_position(Position::new(
            input_area.x + app.cursor_offset + 1,
            input_area.y + 1,
        ));

        // 优化：快速找到第一个可见的块
        Self::render_visible_blocks(app, frame, area);

        // 渲染滚动条
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalLeft)
                .symbols(scrollbar::VERTICAL)
                .begin_symbol(None)
                .track_symbol(None)
                .end_symbol(None),
            scroll_area,
            &mut app.vertical_scroll_state,
        );
        // 最后渲染输入
        frame.render_widget(&app.input, input_area);

        // 渲染选项对话框（如果可见）
        if app.option_dialog.visible {
            frame.render_widget(&app.option_dialog, frame.area());
        }
    }

    /// 渲染可见的消息块（优化版本）
    fn render_visible_blocks(
        app: &mut App,
        frame: &mut Frame<'_>,
        mut area: ratatui::layout::Rect,
    ) {
        let _perf_monitor = crate::perf_start!("Renderer::render_visible_blocks", 30);

        let st = app.index as usize;
        let ed = area.height as usize + st;
        let mut block_start_line = 0;
        let mut height = area.height as usize;

        // 快速跳过不可见的块
        let mut block_index = 0;
        let blocks = &app.blocks;
        let blocks_len = blocks.len();

        // 找到第一个可能可见的块
        while block_index < blocks_len {
            let blk = &blocks[block_index];
            let block_end_line = block_start_line + blk.line_count as usize;

            // 检查块是否在可见范围内
            if block_end_line > st && block_start_line < ed {
                break;
            }

            block_start_line = block_end_line;
            block_index += 1;
        }

        // 渲染可见的块（使用直接索引访问）
        for i in block_index..blocks_len {
            let blk = &blocks[i];
            let block_end_line = block_start_line + blk.line_count as usize;

            // 如果块完全在可见范围之上，继续下一个
            if block_end_line <= st {
                block_start_line = block_end_line;
                continue;
            }

            // 如果块完全在可见范围之下，停止渲染
            if block_start_line >= ed {
                break;
            }

            // 计算块的高度（可能部分可见）
            let visible_start = if block_start_line < st {
                st - block_start_line
            } else {
                0
            };

            let visible_end = if block_end_line > ed {
                ed - block_start_line
            } else {
                blk.line_count as usize
            };

            let visible_height = visible_end - visible_start;
            let blk_height = min(height, visible_height) as u16;

            let mut blk_area = area;
            blk_area.height = blk_height;

            // 渲染块
            if visible_start > 0 {
                // 块部分可见（从中间开始）
                blk.render_block(
                    blk_area,
                    frame.buffer_mut(),
                    visible_start as u16,
                    blk_area.width,
                );
            } else {
                // 块完全可见
                frame.render_widget(blk, blk_area);
            }

            height -= blk_height as usize;
            area.y = min(blk_area.height + area.y, area.height);

            if height <= 0 {
                break;
            }

            block_start_line = block_end_line;
        }
    }
}
