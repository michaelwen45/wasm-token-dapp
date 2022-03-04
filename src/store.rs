use std::collections::HashMap;
use sycamore::prelude::*;

pub struct Count(pub i32);

impl Default for Count {
    fn default() -> Count {
        Count(0)
    }
}

pub struct Files(pub HashMap<String, gloo_file::File>);

impl Default for Files {
    fn default() -> Files {
        let files: HashMap<String, gloo_file::File> = HashMap::new();
        Files(files)
    }
}

pub type FilesVec = Vec<gloo_file::File>;

pub fn initialize_store(ctx: ScopeRef) {
    ctx.provide_context_ref(ctx.create_signal(Count::default()));
    ctx.provide_context_ref(ctx.create_signal(Files::default()));
    ctx.provide_context_ref(ctx.create_signal(FilesVec::new()));
}
pub enum Action {
    CountIncrement(i32),
    FilesSet(web_sys::FileList),
}

pub fn reducer(ctx: ScopeRef, action: Action) {
    match action {
        Action::CountIncrement(increment) => {
            let count = ctx.use_context::<Signal<Count>>();
            count.set(Count(count.get().0 + increment));
        }
        Action::FilesSet(file_list) => {
            let files = ctx.use_context::<Signal<Files>>();
            let files_vec = ctx.use_context::<Signal<FilesVec>>();

            let new_files_vec = gloo_file::FileList::from(file_list).to_vec();
            log::debug!("{:?}", new_files_vec);
            files_vec.set(new_files_vec.clone());

            let mut new_files = Files::default();
            new_files_vec.into_iter().for_each(|f| {
                new_files.0.insert(f.name(), f);
            });
            files.set(new_files);
        }
    }
}
