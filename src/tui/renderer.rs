use std::cmp::min;

use ratatui::{
    layout::Position,
    symbols::scrollbar,
    widgets::{Scrollbar, ScrollbarOrientation},
    Frame,
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
        // 处理信息块
        let st = app.index as usize;
        let ed = area.height as usize + st;
        let mut block_start_line = 0;
        let mut height = area.height as usize;
        // 计算在范围内的块、显示块
        for blk in app.blocks.iter() {
            let block_end_line = blk.line_count as usize + block_start_line;
            // 在显示范围内
            if block_end_line > st || block_start_line > ed {
                let mut blk_area = area;
                let blk_height = min(
                    height,
                    min(blk.line_count as usize + block_start_line - st, blk.line_count as usize),
                ) as u16;
                blk_area.height = blk_height;
                // debug!(
                //     "显示 {:?} {:?} {} {}",
                //     area, blk_area, height, blk.line_count
                // );
                // 如果前面的文字显示出框，挑后面的显示
                if block_start_line < st {
                    // +3 往后一点，不然显示有问题
                    blk.render_block(
                        blk_area,
                        frame.buffer_mut(),
                        (st - block_start_line) as u16,
                        blk_area.width,
                    );
                } else {
                    frame.render_widget(blk, blk_area);
                }
                height -= blk_area.height as usize;
                area.y = min(blk_area.height + area.y, area.height);
            }
            if height <= 0 {
                break;
            }
            block_start_line += blk.line_count as usize;
        }
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
}
