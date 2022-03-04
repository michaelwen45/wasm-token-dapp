use sycamore::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Event, HtmlInputElement};

#[component]
pub fn FilesUploader<G: Html>(ctx: ScopeRef) -> View<G> {
    let file_list = ctx.create_signal(Vec::<gloo_file::File>::new());
    view! {ctx,
        div(class="space-y-4") {
            label(for="file-upload", class="px-5 py-3 rounded-lg shadow-lg bg-indigo-700 hover:bg-indigo-600 active:bg-indigo-800
            focus:outline-none text-sm text-slate-200 uppercase tracking-wider
            font-semibold sm:text-base"){
                    "Select Files..."
                    input(id="file-upload", class="hidden", type="file", multiple=true, on:change={
                        |event: Event| {
                            let target: HtmlInputElement = event.target().unwrap().unchecked_into();
                            if let Some(files) = target.files() {
                                file_list.set((0..files.length()).map(|i|gloo_file::File::from(files.get(i).unwrap())).collect());
                                log::debug!("{:?}", file_list.get());

                                // let files = js_sys::try_iter(files).map(|file| ;
                                // wasm_bindgen_futures::spawn_local(async move {
                                //     let tx = create_transaction(files.get(0).unwrap()).await.unwrap();
                                //     ;
                            }
                        }
                    }) {
                    }

                }
            }
            div(class="overflow-hidden rounded-lg") {
                table(class="min-w-full") {
                    thead(class="bg-slate-700") {
                        tr {
                            th(scope="col", class="py-3 px-6 font-semibold tracking-wider text-left text-slate-100 uppercase") {"Name"}
                            th(scope="col", class="py-3 px-6 font-semibold tracking-wider text-left text-slate-100 uppercase") {"Size"}
                            th(scope="col", class="py-3 px-6 font-semibold tracking-wider text-left text-slate-100 uppercase") {""}
                        }
                    }
                    tbody {
                        Indexed {
                            iterable: file_list,
                            view: |ctx, f| {
                                let name = f.name();
                                let size = f.size();
                                view! {ctx,
                                    tr(class="bg-slate-600 border-slate-700") {
                                        td(class="py-4 px-6 font-medium whitespace-nowrap text-white") {(name)}
                                        td(class="py-4 px-6 text-slate-200") {(size)}
                                        td(class="py-4 px-6 text-slate-200") {button(class="px-5 py-3 rounded-lg shadow-lg bg-indigo-700 hover:bg-indigo-600 active:bg-indigo-800
                                        focus:outline-none text-sm text-slate-200 uppercase tracking-wider
                                        font-semibold sm:text-base"){"Upload"}}
                                    }
                                }
                            }
                        }
                    }
                }
            }
    }
}
