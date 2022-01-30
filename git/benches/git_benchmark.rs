use std::fs;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use git::internals::{self, get_log_data};

#[allow(dead_code)]
fn rewrite_input_file(file: &str) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let repository = std::path::Path::new("/home/p4c/apps/git");
            let revision_range = &["--all".to_string(), "--until=01.01.2021".to_string()];
            let log = internals::get_log(repository, revision_range)
                .await
                .expect("get_log failed");
            fs::write(file, log).expect("Couldn't write a file");
        })
}

fn get_log_data_benchmark(c: &mut Criterion) {
    let benchmark_input = "benches/git_benchmark_input.txt";
    // to update the git_benchmark_input.txt use the call below
    // rewrite_input_file(benchmark_input);

    let text = fs::read(benchmark_input).expect("Couldn't read benchmark input");
    let text = String::from_utf8(text).expect("failed on decoding input");

    c.bench_function("get_log_data", |b| {
        b.iter(|| get_log_data(black_box(&text)))
    });
}

criterion_group!(benches, get_log_data_benchmark);
criterion_main!(benches);
