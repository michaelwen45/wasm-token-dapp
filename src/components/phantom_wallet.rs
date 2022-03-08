use std::str::FromStr;

use crate::{
    error::Error,
    store::{reducer, Action},
};
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, transaction::Transaction};
use sycamore::prelude::*;
use wasm_bindgen::{prelude::*, JsCast};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PhantomResult {
    #[serde(rename_all = "camelCase")]
    Connect {
        public_key: String,
    },
    Disconnect,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhantomResponse {
    id: u32,
    jsonrpc: String,
    result: PhantomResult,
}

#[derive(Debug)]
pub struct PhantomWallet {
    pub is_connected: bool,
    pub public_key: Pubkey,
}

impl Default for PhantomWallet {
    fn default() -> PhantomWallet {
        PhantomWallet {
            is_connected: false,
            public_key: Pubkey::default(),
        }
    }
}

impl PhantomWallet {
    fn connect() -> Result<PhantomWallet, Error> {
        let window = web_sys::window().unwrap();
        if let Some(solana) = window.get("solana") {
            let is_phantom =
                js_sys::Reflect::get(&*solana, &wasm_bindgen::JsValue::from_str("isPhantom"))
                    .unwrap();
            if is_phantom == JsValue::from(true) {
                let this = JsValue::null();
                let connect_str = wasm_bindgen::JsValue::from_str("connect");
                let connect: js_sys::Function =
                    js_sys::Reflect::get(&*solana, &connect_str).unwrap().into();
                connect.call0(&this).unwrap();
                Ok(PhantomWallet {
                    is_connected: false,
                    public_key: Pubkey::default(),
                })
            } else {
                Err(Error::PhantomWalletNotFound)
            }
        } else {
            Err(Error::PhantomWalletNotFound)
        }
    }
    fn disconnect(&self) -> Result<PhantomWallet, Error> {
        let window = web_sys::window().unwrap();
        if let Some(solana) = window.get("solana") {
            let this = JsValue::null();
            let disconnect_str = wasm_bindgen::JsValue::from_str("disconnect");
            let disconnect: js_sys::Function = js_sys::Reflect::get(&*solana, &disconnect_str)
                .unwrap()
                .into();
            disconnect.call0(&this).unwrap();
            Ok(PhantomWallet::default())
        } else {
            Err(Error::PhantomWalletNotFound)
        }
    }
    fn pubkey(&self) -> Result<PhantomWallet, Error> {
        #[allow(unused_assignments)]
        let mut is_connected = false;
        let mut public_key = Pubkey::default();
        let window = web_sys::window().unwrap();
        if let Some(solana) = window.get("solana") {
            let is_connected_str = wasm_bindgen::JsValue::from_str("isConnected");
            is_connected = js_sys::Reflect::get(&solana, &is_connected_str)
                .unwrap()
                .as_bool()
                .unwrap();
            log::debug!("is_connected: {:?}", is_connected);
            if is_connected {
                let pubkey_str = wasm_bindgen::JsValue::from_str("publicKey");
                let pubkey_obj: js_sys::Object =
                    js_sys::Reflect::get(&solana, &pubkey_str).unwrap().into();

                let bn_str = wasm_bindgen::JsValue::from_str("toString");
                let to_string_fn: js_sys::Function =
                    js_sys::Reflect::get(&pubkey_obj, &bn_str).unwrap().into();

                log::debug!("pubkey_obj: {:?}", to_string_fn.call0(&pubkey_obj));
                if let Ok(pubkey) = to_string_fn.call0(&pubkey_obj) {
                    public_key = Pubkey::from_str(&pubkey.as_string().unwrap()).unwrap();
                    log::debug!("pubkey: {:?}", public_key);
                };
            }
        } else {
            return Err(Error::PhantomWalletNotFound);
        }

        Ok(PhantomWallet {
            is_connected,
            public_key,
        })
    }

    pub async fn sign_transaction(transaction: Transaction) {}
}

#[component]
pub fn Wallet<G: Html>(ctx: ScopeRef) -> View<G> {
    let window = web_sys::window().expect("should have a window in this context");
    let document = window.document().expect("window should have a document");
    let a = Closure::wrap(Box::new(move |message_event: web_sys::MessageEvent| {
        // log::debug!("message event: {:?}", message_event.data());
        let data = message_event.data();
        if let Ok(value) = serde_wasm_bindgen::from_value::<PhantomResponse>(data) {
            match value.result {
                PhantomResult::Connect { public_key } => {
                    log::debug!("public_key: {:?}", public_key);
                    let new_event = web_sys::Event::new("connect").unwrap();
                    document
                        .get_element_by_id("message-target")
                        .expect("#message-target should exist")
                        .dyn_ref::<web_sys::HtmlElement>()
                        .expect("message-target should be an html element")
                        .dispatch_event(&new_event)
                        .unwrap();
                }
                PhantomResult::Disconnect => {
                    let new_event = web_sys::Event::new("disconnect").unwrap();
                    log::debug!("disconnected");
                    document
                        .get_element_by_id("message-target")
                        .expect("#message-target should exist")
                        .dyn_ref::<web_sys::HtmlElement>()
                        .expect("message-target should be an html element")
                        .dispatch_event(&new_event)
                        .unwrap();
                }
            }
        }

        // ding.set(true);
    }) as Box<dyn Fn(_)>);
    window
        .add_event_listener_with_callback("message", a.as_ref().unchecked_ref())
        .unwrap();
    a.forget();

    let wallet_sig = ctx.use_context::<Signal<PhantomWallet>>();

    view! {ctx, div(id="message-target",
        on:connect={|event: web_sys::Event| {
            log::debug!("message-target: {:?}", event.type_());
            let wallet = wallet_sig.get().pubkey().unwrap();
            wallet_sig.set(wallet);
         }}) {
            button(class="px-5 py-3 rounded-lg shadow-lg bg-indigo-700 hover:bg-indigo-600 active:bg-indigo-800
                        focus:outline-none text-sm text-slate-200 uppercase tracking-wider
                        font-semibold sm:text-base",
                    on:click={|_| {
                        // HACK: not sure why we have to call connect twice to get wallet to show up as connected
                        #[allow(unused_assignments)]
                        let mut wallet = PhantomWallet::connect().unwrap();
                        wallet = PhantomWallet::connect().unwrap();
                        reducer(ctx, Action::WalletSet(wallet));
                     }
            }) {
                "Connect"
            }

            button(class="px-5 py-3 rounded-lg shadow-lg bg-indigo-700 hover:bg-indigo-600 active:bg-indigo-800
                        focus:outline-none text-sm text-slate-200 uppercase tracking-wider
                        font-semibold sm:text-base",
                    on:click=|_| {
                        let wallet = wallet_sig.get().disconnect().unwrap();
                        wallet_sig.set(wallet);
                     }
            ) {
                "Disconnect"
            }
        }
    }
}
