use reqwest::Client;
use stylist::{css, StyleSource};
use tracing::debug;
use validated::Validated;
use yew::{html, AttrValue, Callback, Component, Context, Html, NodeRef, Properties};

use super::{
    advanced_mode::AdvancedMode,
    expiration_input::ExpirationInput,
    link_input::{LinkInput, LinkInputMessage},
    TEXT_INPUT,
};
use crate::{
    app::index::IndexMessage,
    endpoint,
    types::{error::RequestError, link_config::LinkConfig},
    util::{generate_id, server_config, AsClasses},
    INPUT_WIDTH,
};

thread_local! {
    static LABEL: StyleSource = css!(r#"
        display: block;
        font-size: 12px;
        margin-bottom: 3px;
        padding-left: 5px;
    "#);

    static CONTAINER: StyleSource = css!(r#"
        flex: 1;
        display: flex;
        flex-direction: column;
        margin-top: 2px;
    "#);

    static EXPIRATION_CONTAINER: StyleSource = css!(r#"
            display: flex;
            flex-direction: column;
            flex: 1;
            min-width: ${iw};
    "#, iw = INPUT_WIDTH);

    static HEADING: StyleSource = css!(r#"
        margin: 0 0 4px;
    "#);
}

async fn make_request(link_config: LinkConfig) -> Result<AttrValue, RequestError> {
    let json = serde_json::to_string(&link_config).expect("Json could not be serialized");

    if let Some(config) = server_config() {
        if json.len() > config.max_json_size {
            return Err(RequestError::JsonSizeExceeded);
        }
    }

    let client = Client::new();
    let result = client
        .post(endpoint!("custom"))
        .header("content-type", "application/json")
        .body(json)
        .send()
        .await;

    if let Err(e) = result {
        return Err(RequestError::UnsuccessfulRequest { error: e });
    }

    let response = result.unwrap();
    let status = response.status();

    let text = response
        .text()
        .await
        .expect("Expected a text/plain response");

    debug!(
        "Received: {:#?}\n from /custom with code {}",
        text,
        status.as_u16()
    );

    if status.is_success() {
        Ok(AttrValue::from(text))
    } else {
        Err(match status.as_u16() {
            // TODO use concrete backend error enum to match
            400 => RequestError::Backend400,
            409 => RequestError::IdInUse {
                id: link_config
                    .id
                    .expect("Server returned an in use id, even though no custom id was provided"),
            },
            code => panic!("Unexpected return code: {}", code),
        })
    }
}

#[derive(Default, Clone)]
pub struct LinkFormRefs {
    pub link_input: NodeRef,
    pub advanced_mode: NodeRef,
    pub max_usage_input: NodeRef,
    pub custom_id_input: NodeRef,
    pub expiration_input: NodeRef,
    pub expiration_type: NodeRef,
}

#[derive(Clone, Debug, Default)]
pub enum LinkFormMessage {
    #[default]
    Input,
    Display(AttrValue),
}

#[derive(Properties, PartialEq)]
pub struct LinkFormPros {
    pub manage_messages: Callback<IndexMessage>,
}

#[derive(Default)]
pub struct LinkForm {
    state: LinkFormMessage,
    refs: LinkFormRefs,
}

impl Component for LinkForm {
    type Message = LinkFormMessage;
    type Properties = LinkFormPros;

    fn create(_: &Context<Self>) -> Self {
        Self {
            state: LinkFormMessage::Input,
            refs: LinkFormRefs::default(),
        }
    }

    fn update(&mut self, _: &Context<Self>, msg: Self::Message) -> bool {
        self.state = msg;

        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let manage_messages = ctx.props().manage_messages.clone();
        let scope = ctx.link().clone();
        let refs = self.refs.clone();

        let onclick = Callback::from(move |_| {
            let link_config = LinkConfig::try_from(&refs);

            match link_config {
                Validated::Good(config) => {
                    let manage_messages = manage_messages.clone();

                    debug!("Sending: {:#?}\n to /custom", config);

                    scope.send_future(async move {
                        match make_request(config).await {
                            Ok(link) => LinkFormMessage::Display(link),
                            Err(e) => {
                                manage_messages.emit(IndexMessage::AddMessage(e.into()));
                                LinkFormMessage::Input
                            },
                        }
                    });
                },
                Validated::Fail(errors) => {
                    for error in errors {
                        manage_messages.emit(IndexMessage::AddMessage(error.into()));
                    }
                },
            }
        });

        let clear_callback = ctx.link().callback(|_| LinkFormMessage::Input);
        // TODO rerender if server_config is fetched or not if it failed
        // let size = server_config().map(|c| c.max_custom_id_length.to_string());

        let ids = [generate_id(), generate_id(), generate_id()];

        // TODO remove code duplication
        html! {
            <>
                <h1 class={ HEADING.as_classes() }>{ "[WIP] Link Shortener" }</h1>
                <LinkInput { onclick } input_ref={ self.refs.link_input.clone() } message={ LinkInputMessage::from(self.state.clone()) } manage_messages={ ctx.props().manage_messages.clone() } { clear_callback }/>
                <AdvancedMode toggle_ref={ self.refs.advanced_mode.clone() }>
                    <div class={ CONTAINER.as_classes() }>
                        <label class={ LABEL.as_classes() } for={ ids[0].clone() }>{ "Max. usages" }</label>
                        <input id={ ids[0].clone() } class={ TEXT_INPUT.as_classes() } ref={ self.refs.max_usage_input.clone() } type="number" min="0" placeholder=""/>
                    </div>
                    <div class={ CONTAINER.as_classes() }>
                        <label class={ LABEL.as_classes() } for={ ids[1].clone() }>{ "Custom id" }</label>
                        <input id={ ids[1].clone() } class={ TEXT_INPUT.as_classes() } ref={ self.refs.custom_id_input.clone() } type="text" placeholder=""/>
                    </div>
                    <div class={ CONTAINER.as_classes() }>
                        <label class={ LABEL.as_classes() } for={ ids[2].clone() }>{ "Expire after" }</label>
                        <div class={ EXPIRATION_CONTAINER.as_classes() }>
                            <ExpirationInput id={ ids[2].clone() } toggle_ref={ self.refs.expiration_type.clone() } input_ref={ self.refs.expiration_input.clone() }/>
                        </div>
                    </div>
                </AdvancedMode>
            </>
        }
    }
}
