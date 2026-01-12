use crate::acp::types::{AcpError, AcpResult};
use crate::acp::transport::Transport;
use async_trait::async_trait;
use futures::Stream;
use std::io::{self, BufRead, Write};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Stdio传输层实现
pub struct StdioTransport {
    _sender: mpsc::UnboundedSender<String>,
    receiver: Arc<Mutex<mpsc::UnboundedReceiver<String>>>,
}

impl StdioTransport {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            _sender: sender,
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send_response(&self, response: &str) -> AcpResult<()> {
        // 写入stdout
        if let Err(e) = writeln!(io::stdout(), "{}", response) {
            return Err(AcpError::TransportError(format!("写入stdout失败: {}", e)));
        }
        Ok(())
    }

    async fn send_notification(&self, notification: &str) -> AcpResult<()> {
        self.send_response(notification).await
    }

    fn receive_stream(&self) -> Pin<Box<dyn Stream<Item = Result<String, AcpError>> + Send + '_>> {
        let receiver = self.receiver.clone();
        
        // 在后台线程中读取stdin并发送到channel
        let stdin = io::stdin();
        std::thread::spawn(move || {
            for line in stdin.lock().lines() {
                match line {
                    Ok(text) => {
                        if !text.trim().is_empty() {
                            let rec = receiver.lock().unwrap();
                            // 注意：这里我们实际上需要另一个channel来转发消息
                            // 由于设计问题，我们需要重新考虑这个实现
                            drop(rec);
                        }
                    }
                    Err(e) => {
                        eprintln!("读取stdin错误: {}", e);
                        break;
                    }
                }
            }
        });

        Box::pin(async_stream::try_stream! {
            // 使用tokio::sync::mpsc来实现异步接收
            let (tx, mut rx) = mpsc::unbounded_channel::<String>();
            
            // 在独立任务中读取stdin
            tokio::spawn(async move {
                let stdin = io::stdin();
                for line in stdin.lock().lines() {
                    match line {
                        Ok(text) => {
                            if !text.trim().is_empty() {
                                let _ = tx.send(text);
                            }
                        }
                        Err(e) => {
                            eprintln!("读取stdin错误: {}", e);
                            break;
                        }
                    }
                }
            });
            
            while let Some(msg) = rx.recv().await {
                yield msg;
            }
        })
    }

    async fn close(&self) -> AcpResult<()> {
        // Stdio不需要显式关闭
        Ok(())
    }
}
