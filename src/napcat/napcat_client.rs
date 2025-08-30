use crate::{Args, mcp};
use crate::{
    chat::Chat,
    client::get_output_tostring,
    napcat::{self, napcatconfig::NapCatConfig},
};
use clap::Parser;
use log::info;
use onebot_v11::{
    MessageSegment,
    api::payload::{ApiPayload, SendGroupForwardMsg, SendPrivateForwardMsg},
    connect::ws_reverse::{self, ReverseWsConfig},
};

pub struct NapCatClient {
    config: NapCatConfig,
    chat: Chat,
}

impl NapCatClient {
    pub fn new(config: NapCatConfig) -> Self {
        let mut chat = Chat::default();
        let args = Args::parse();
        if Some(true) == args.use_tool {
            chat = chat.tools(mcp::get_config_tools());
        }
        Self { config, chat }
    }

    pub async fn start(&mut self) {
        println!("开始监听 napcat");
        let catconfig = napcat::napcatconfig::NapCatConfig::local().expect("未配置 napcat.toml");
        let mut cfg = ReverseWsConfig::default();
        cfg.access_token = Some(catconfig.token.clone());
        cfg.port = catconfig.port.unwrap_or(8080);
        println!(
            "地址配置：{}:{}/{} token:{}",
            cfg.host,
            cfg.port,
            cfg.suffix,
            cfg.access_token.clone().unwrap()
        );
        let conn = ws_reverse::ReverseWsConnect::new(cfg).await.unwrap();
        let mut revc = conn.subscribe().await;
        loop {
            let res = revc.recv().await;
            if res.is_err() {
                log::error!("接收信息错误 {:?}", res.unwrap_err());
                continue;
            }
            let res = res.unwrap();
            match res {
                onebot_v11::Event::Message(message) => match message {
                    onebot_v11::event::message::Message::PrivateMessage(private_msg) => {
                        if self.config.is_target_user(private_msg.user_id) {
                            let stream = self.chat.chat(&get_text_msg(private_msg.message));
                            let response = get_output_tostring(stream).await;
                            let payload =
                                ApiPayload::SendPrivateForwardMsg(SendPrivateForwardMsg {
                                    user_id: private_msg.user_id,
                                    messages: vec![MessageSegment::text(response)],
                                });
                            let res = conn.clone().call_api(payload).await;
                            info!("{:?}", res);
                        }
                    }
                    onebot_v11::event::message::Message::GroupMessage(group_message) => {
                        if self.config.is_group_at_self(group_message.clone()) {
                            let stream = self.chat.chat(&get_text_msg(group_message.message));
                            let response = get_output_tostring(stream).await;
                            let payload = ApiPayload::SendGroupForwardMsg(SendGroupForwardMsg {
                                group_id: group_message.group_id,
                                messages: vec![MessageSegment::text(response)],
                            });
                            let res = conn.clone().call_api(payload).await;
                            info!("{:?}", res);
                        }
                    }
                },
                _ => {} // onebot_v11::Event::Meta(meta) => ,
                        // onebot_v11::Event::Notice(notice) => todo!(),
                        // onebot_v11::Event::Request(request) => todo!(),
                        // onebot_v11::Event::ApiRespBuilder(api_resp_builder) => todo!(),
            }
        }
    }
}

pub fn get_text_msg(messages: Vec<MessageSegment>) -> String {
    let mut res = String::new();
    for msg in messages {
        if let MessageSegment::Text { data } = msg {
            res += &data.text;
        }
    }
    res
}
