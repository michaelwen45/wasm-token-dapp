mod crypto;
mod error;
mod merkle;
mod transaction;
use sycamore::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Event, HtmlInputElement};

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).unwrap();

    sycamore::render(|ctx| {
        let state = ctx.create_signal(0);
        let label = ctx.create_signal("nada");
        view! { ctx,
            div(class="container mx-auto h-screen") {
                button(class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded", on:click=|_| { state.set(*state.get() + 1) } ) {
                    (label.get())
                }
                input(type="file", multiple=true, on:change={
                    |event: Event| {
                        let target: HtmlInputElement = event.target().unwrap().unchecked_into();
                        if let Some(files) = target.files() {
                            wasm_bindgen_futures::spawn_local(async move {
                                let file = gloo_file::File::from(files.get(0).unwrap());
                                let bytes = gloo_file::futures::read_as_bytes(&file).await.unwrap();
                                log::debug!("length: {}, bytes: {:?}", bytes.len(), bytes)});
                        }
                    }
                }) {

                }
            }
        }
    });
}
