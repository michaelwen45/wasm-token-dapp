mod components;
mod crypto;
mod error;
mod merkle;
mod store;
mod transaction;
use components::{
    files::FilesSelector,
    phantom_wallet::{PhantomWallet, Wallet},
};
use store::*;
use sycamore::prelude::*;
use wasm_bindgen::{prelude::*, JsCast};

#[derive(Prop)]
struct CounterProps<'a> {
    label: &'a ReadSignal<String>,
}

#[component]
fn Counter<'a, G: Html>(ctx: ScopeRef<'a>, props: CounterProps<'a>) -> View<G> {
    let count = ctx.use_context::<Signal<Count>>();
    view! {ctx, div() {
            button(class="px-5 py-3 rounded-lg shadow-lg bg-indigo-700 hover:bg-indigo-600 active:bg-indigo-800
                        focus:outline-none text-sm text-slate-200 uppercase tracking-wider
                        font-semibold sm:text-base",
                    on:click=|_| { reducer(ctx, Action::CountIncrement(2)) }
            ) {
                (format!("{}: {}", props.label.get(), count.get().0))
            }
        }
    }
}

#[component]
fn App<G: Html>(ctx: ScopeRef) -> View<G> {
    initialize_store(ctx);
    let label = ctx.create_signal("count".to_string());
    let wallet = ctx.use_context::<Signal<PhantomWallet>>();
    ctx.create_effect(|| log::debug!("Connection status changed to {:?}", wallet.get()));

    view! { ctx,
        div(class="container mx-auto space-y-4") {
            h1(class="text-2xl text-slate-200 font-semibold pt-8") {
                "WASM Token App"
            }
            Counter {
                label: label
            }
            FilesSelector {}
            Wallet {}
        }
    }
}

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).unwrap();

    sycamore::render(|ctx| view! {ctx, App() });
}
