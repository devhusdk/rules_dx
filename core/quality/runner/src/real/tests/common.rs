use super::*;

pub(super) fn oversized_selection(count: usize) -> BTreeMap<String, String> {
    padded_selection(count, 96)
}

pub(super) fn padded_selection(count: usize, pad: usize) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for index in 0..count {
        let name = format!("notes-{index:04}-{}.txt", "n".repeat(pad));
        let body = if index % 2 == 0 {
            KEEP_SORTED_DIRTY
        } else {
            KEEP_SORTED_CLEAN
        };
        files.insert(name, body.to_owned());
    }
    files
}

pub(super) struct SpawnCount(std::sync::Mutex<Vec<(std::thread::ThreadId, usize)>>);

impl SpawnCount {
    pub(super) const fn new() -> Self {
        Self(std::sync::Mutex::new(Vec::new()))
    }

    fn with<T>(&self, act: impl FnOnce(&mut usize) -> T) -> T {
        let me = std::thread::current().id();
        let mut all = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let at = if let Some(at) = all.iter().position(|(id, _)| *id == me) {
            at
        } else {
            all.push((me, 0));
            all.len() - 1
        };
        act(&mut all[at].1)
    }

    pub(super) fn bump(&self) -> usize {
        self.with(|count| {
            *count += 1;
            *count - 1
        })
    }

    pub(super) fn reset(&self) {
        self.with(|count| *count = 0);
    }

    pub(super) fn get(&self) -> usize {
        self.with(|count| *count)
    }
}

pub(super) fn assert_within_budget(argv: &[OsString]) {
    let budget = commands::argv_budget(&argv[0]);
    let (bytes, units) = commands::argv_cost(argv);
    assert!(bytes <= budget, "spawn carries {bytes} argv bytes");
    assert!(units <= budget, "spawn carries {units} argv units");
}

pub(super) const NOTEBOOK_DIRTY: &str = "{\n  \"cells\": [\n    {\n      \"cell_type\": \"markdown\",\n      \"metadata\": {},\n      \"source\": [\"# Title\"]\n    },\n    {\n      \"cell_type\": \"code\",\n      \"execution_count\": 7,\n      \"metadata\": {\"tags\": [\"keep\"]},\n      \"outputs\": [{\"name\": \"stdout\", \"output_type\": \"stream\", \"text\": [\"hi\\n\"]}],\n      \"source\": [\n        \"import os\\n\",\n        \"print('hi')\"\n      ]\n    },\n    {\n      \"cell_type\": \"code\",\n      \"execution_count\": null,\n      \"metadata\": {},\n      \"outputs\": [],\n      \"source\": [\"x   =   1\"]\n    }\n  ],\n  \"metadata\": {\"language_info\": {\"name\": \"python\"}},\n  \"nbformat\": 4,\n  \"nbformat_minor\": 5\n}\n";

pub(super) fn notebook_backend() -> RealBackend {
    backend_for("ruff", plain_tool(), ruff_notebook_behavior)
}
