mod components;
mod crypto;
mod error;
mod merkle;
mod transaction;
use components::files::FilesUploader;
use error::Error;
use sycamore::prelude::*;
use transaction::Transaction;

pub async fn create_transaction(file: gloo_file::File) -> Result<Transaction, Error> {
    let bytes = gloo_file::futures::read_as_bytes(&file).await.unwrap();
    transaction::merklize(bytes)
}

#[derive(Prop)]
struct CounterProps<'a> {
    label: &'a ReadSignal<String>,
}

#[component]
fn Counter<'a, G: Html>(ctx: ScopeRef<'a>, props: CounterProps<'a>) -> View<G> {
    let count = ctx.use_context::<Signal<i32>>();
    view! {ctx, div() {
            button(class="px-5 py-3 rounded-lg shadow-lg bg-indigo-700 hover:bg-indigo-600 active:bg-indigo-800
                        focus:outline-none text-sm text-slate-200 uppercase tracking-wider
                        font-semibold sm:text-base",
                    on:click=|_| { count.set(*count.get() + 1) }
            ) {
                (format!("{}: {}", props.label.get(), count.get()))
            }
        }
    }
}

#[component]
fn Wallet<'a, G: Html>(ctx: ScopeRef<'a>) -> View<G> {
    view! {ctx, div() {
            button(class="px-5 py-3 rounded-lg shadow-lg bg-indigo-700 hover:bg-indigo-600 active:bg-indigo-800
                        focus:outline-none text-sm text-slate-200 uppercase tracking-wider
                        font-semibold sm:text-base",
                    on:click=|_| {
                        let window = web_sys::window().unwrap();
                        let solana: js_sys::Object = window.get("solana").unwrap();
                        let is_phantom = js_sys::Reflect::get(&*solana, &wasm_bindgen::JsValue::from_str("isPhantom")).unwrap().as_bool().unwrap();
                        let connect: js_sys::Function = js_sys::Reflect::get(&*solana, &wasm_bindgen::JsValue::from_str("connect")).unwrap().into();
                        let res = js_sys::Reflect::apply(&connect, &*solana, &js_sys::Array::new()).unwrap();
                        log::debug!("{:?}", res);
                     }
            ) {
                "CONNECT"
            }
        }
    }
}

#[component]
fn App<G: Html>(ctx: ScopeRef) -> View<G> {
    let label = ctx.create_signal("count".to_string());
    let count = ctx.create_signal(0);
    ctx.provide_context_ref(count);
    view! { ctx,
        h1(class="text-xl text-slate-200 font-semibold  p-4") {
            "WASM Token App"
        }
        div(class="mx-auto max-w-2xl space-y-4") {
            Counter {
                label: label
            }
            FilesUploader {}
            Wallet {}
        }
    }
}

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).unwrap();

    sycamore::render(|ctx| view! {ctx, App() });
}
