use std::{cmp::min, io, sync::{Arc, Mutex}};

use futures::{pin_mut, StreamExt};
use log::{info};
use ratatui::{ crossterm::event::{self, Event, KeyCode, KeyEventKind}, symbols::scrollbar, widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState}, DefaultTerminal, Frame};

use crate::{chat::{Chat, StreamedChatResponse}, model::param::ModelMessage, tui::{inputarea::InputArea, messageblock::MessageBlock}};


pub struct App {
    chat: Arc<Mutex<Chat>>,
    should_exit: bool,
    index: u16,
    input: InputArea,
    window_height: u16,
    blocks: Vec<MessageBlock>,
    width: u16,
    max_line: u16,
    vertical_scroll_state: ScrollbarState,
}

impl App {
    pub fn new()->Self {
        let chat = Chat::default();
        // chat.context.push(ModelMessage::user("<think>\n嗯，用户让我背诵《静夜思》。首先，我需要确认这首诗的内容。《静夜思》是唐代诗人李白的代表作之一，属于五言绝句。正确的诗句应该是“床前明月光，疑是地上霜。举头望明月，低头思故乡。”。\n\n不过，用户可能有不同的版本或者有其他需求。比如，有时候可能会有错别字或者不同的翻译。我需要先确保自己正确无误地背诵出来。\n\n接下来，我应该用中文回答，因为用户的问题是用中文提出的。可能用户需要这首诗的完整内容，或者希望我解释一下这首诗的背景、含义，或者只是简单的背诵。\n\n用户说“背诵静夜思”，可能只需要我直接背出诗句。不过作为助理，我应该先确认用户的需求。比如，用户可能想让我用中文把这首诗背出来，或者可能想让我用其他方式呈现。\n\n另外，考虑到用户可能不太熟悉这首诗，可能需要一些简要的解释，比如作者、创作背景，或者为什么这首诗著名。不过用户明确说“背诵”，所以可能只需要诗句。\n\n我需要检查一下《静夜思》的正确诗句，避免出错。比如，有没有可能用户指的是其他诗？不过“静夜思”通常就是李白的这首。\n\n确认后，我应该用友好的语气，先背出诗句，然后可能加一点解释，比如“这首诗是唐代诗人李白的名作，表达了诗人对故乡的思念之情。”这样既回答了问题，又提供了额外信息，帮助用户更好地理解。\n\n另外，用户可能希望用某种格式，比如分段或者标点。原诗的标点可能有不同，但通常现代标点是“床前明月光，疑是地上霜。举头望明。低头思故乡。”不过正确的标点应该是“床前明月光，疑是地上霜。举头望明月，低头思故乡。”。\n\n可能用户需要的是正确的诗句，所以我要确保没有错误。比如，有没有可能“望明月”和“思故乡”之间有没有逗号，或者有没有其他变化。\n\n查证一下，标准的《静夜思》是：\n\n床前明月光，\n疑是地上霜。\n举头望明月，\n低头思故乡。\n\n所以，正确的诗句是四句，每句五字，押韵。\n\n现在，我需要用中文回复用户，背诵这首诗。可能用户希望我用现代的标点，或者保留古诗的格式。这里可能用现代的标点，四句，每句逗号分隔，最后一句句号。\n\n另外，作为助理，可能需要确认用户是否需要翻译或者更多解释，但用户说“背诵”，所以可能只需要诗句。不过，为了更友好，可以加一句解释，比如“这首诗是唐代诗人李白的名作，表达了诗人对故乡的深切思念。”\n\n所以，回复的结构可能是：\n\n首先，背诵诗句，然后简要解释。\n\n用户可能只是需要诗句，但作为助理，主动提供一点信息会更好，避免用户需要多次提问。\n\n另外，用户的语言是中文，所以回复要使用中文，避免使用英文或其他语言。\n\n检查是否有其他可能的错误，比如有没有可能用户把“静夜思”和其他诗混淆了。比如，有没有其他诗也叫“静夜思”？但通常来说，李白的这首是最著名的，所以可能性不大。\n\n所以，最终的回复应该是：\n\n“床前明月光，疑是地上霜。举头望明月，低头思故乡。”\n\n然后加一句简短的解释，比如“这首诗是唐代诗人李白的名作，表达了对故乡的深切思念。”\n\n这样既准确又友好，符合用户的需求。\n</think>\n\n床前明月光，  \n疑是地上霜。  \n举头望明月，  \n低头思故乡。  \n\n这是唐代诗人李白的名作《静夜思》，以简洁的意象表达了游子对故乡的深切思念。✨ 需要我进一步解释或翻译吗？".into()));
        Self {
            chat: Arc::new(Mutex::new(chat)),
            should_exit: false,
            index: 0,
            input: InputArea::default(),
            window_height: 20,
            blocks: vec![],
            width: 20,
            max_line: 100,
            vertical_scroll_state: ScrollbarState::new(1),
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        let mut area = frame.area();
        // 计算滚动条区域，减去滚动条宽度
        let mut scroll_area = area;
        scroll_area.x = area.width - 1;
        scroll_area.width = 1;
        area.width -= 1;
        self.width = area.width;
        self.refresh();
        // 先计算输入区域
        let mut input_area = area;
        input_area.y = area.height - self.input.height();
        area.height -= self.input.height();
        self.window_height = area.height;
        // 处理信息块
        let st = self.index;
        let ed = area.height + st;
        info!("{} {}", st, ed);
        let mut block_start_line = 0;
        let mut height = area.height;
        // 计算在范围内的块、显示块
        for blk in self.blocks.iter() {
            let block_end_line = blk.line_count + block_start_line;
            // 在显示范围内
            if block_end_line > st || block_start_line > ed {
                let mut blk_area = area;
                let blk_height = min(height, min(blk.line_count + block_start_line - st, blk.line_count));
                blk_area.height = blk_height;
                info!("显示 {:?} {:?} {} {}", area, blk_area, height, blk.line_count);
                // 如果前面的文字显示出框，挑后面的显示
                if block_start_line < st {
                    // +3 往后一点，不然显示有问题
                    blk.render_block(blk_area, frame.buffer_mut(), st - block_start_line, blk_area.width);
                }
                else {
                    frame.render_widget(blk, blk_area);
                }
                height -= blk_area.height;
                area.y = min(blk_area.height + area.y, area.height);
            }
            if height <= 0 {
                break;
            }
            block_start_line += blk.line_count;
        }
        // 渲染滚动条
        frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalLeft)
                .symbols(scrollbar::VERTICAL)
                .begin_symbol(None)
                .track_symbol(None)
                .end_symbol(None),
                scroll_area,
                &mut self.vertical_scroll_state,
        );
        // 最后渲染输入
        frame.render_widget(&self.input, input_area);
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        while !self.should_exit {
            terminal.draw(|frame| {
                self.render(frame);
            })?;
            self.handle_events(&mut terminal).await?;
        }
        Ok(())
    }

    async fn handle_events(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if key.code == KeyCode::Esc {
                    self.should_exit = true;
                }
                else if key.code == KeyCode::Down {
                    if self.max_line > self.window_height {
                        self.index = min(self.max_line - self.window_height, self.index + 1);
                    }
                    else {
                        self.index = 0;
                    }
                }
                else if key.code == KeyCode::Up && self.index > 0 {
                    self.index = std::cmp::max(0, self.index - 1);
                }
                else if key.code == KeyCode::Delete || key.code == KeyCode::Backspace {
                    self.input.backspace();
                }
                else if key.code == KeyCode::Enter {
                    let mut chat = self.chat.lock().unwrap().clone();
                    {
                        let stream = chat.stream_chat(&self.input.content);
                        self.chat.lock().unwrap().context.push(ModelMessage::user(self.input.content.clone()));
                        self.input.clear();
                        self.chat.lock().unwrap().lock();
                        pin_mut!(stream);
                        self.chat.lock().unwrap().context.push(ModelMessage::assistant("".into(), "".into(), vec![]));
                        loop {
                            if self.max_line > self.window_height {
                                self.index = self.max_line - self.window_height;
                            }
                            terminal.draw(|frame| {
                                self.render(frame);
                            })?;
                            if let Some(result) = stream.next().await {
                                let idx = self.chat.lock().unwrap().context.len() - 1;
                                if let Ok(res) = result {
                                    match res {
                                            StreamedChatResponse::Text(text) => self.chat.lock().unwrap().context[idx].add_content(text),
                                        StreamedChatResponse::ToolCall(tool_call) => self.chat.lock().unwrap().context[idx].add_tool(tool_call),
                                        StreamedChatResponse::Reasoning(think) => self.chat.lock().unwrap().context[idx].add_think(think),
                                        StreamedChatResponse::ToolResponse(tool) => self.chat.lock().unwrap().context.push(tool),
                                        StreamedChatResponse::End => {
                                            self.chat.lock().unwrap().context.push(ModelMessage::assistant("".into(), "".into(), vec![]));
                                        }
                                    }
                                }
                            }
                            else {
                                break;
                            }
                        }
                    }
                    self.chat.lock().unwrap().context = chat.context;
                    self.chat.lock().unwrap().unlock();
                }
                else {
                    match key.code {
                        KeyCode::Char(c) => {
                            if !self.chat.lock().unwrap().is_running() {
                                info!("input");
                                self.input.add(c);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn refresh(&mut self) {
        info!("refresh");
        // 先初始化显示结构
        self.blocks.clear();
        self.max_line = 0;
        let ctx = self.chat.lock().unwrap();
        info!("{:?}", ctx.context);
        for msg in ctx.context.iter() {
            let block = MessageBlock::new(msg.clone(), self.width);
            self.max_line += block.line_count;
            self.blocks.push(block);
        }
        if self.max_line > self.window_height {
            self.vertical_scroll_state = self.vertical_scroll_state.content_length((self.max_line - self.window_height) as usize);
        }
        else {
            self.vertical_scroll_state = self.vertical_scroll_state.content_length(1);
        }
        self.vertical_scroll_state = self.vertical_scroll_state.position(self.index as usize);
    }
}