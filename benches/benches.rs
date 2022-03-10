use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput, BenchmarkId};

// use std::time::SystemTime;
use std::fs::File;
use std::io::{BufReader, Read};
use flate2::bufread::GzDecoder;
use serde::Deserialize;
use jumprope::JumpRope;

/// This file contains some simple helpers for loading test data. Its used by benchmarking and
/// testing code.

/// (position, delete length, insert content).
#[derive(Debug, Clone, Deserialize)]
pub struct TestPatch(pub usize, pub usize, pub String);

#[derive(Debug, Clone, Deserialize)]
pub struct TestTxn {
    // time: String, // ISO String. Unused.
    pub patches: Vec<TestPatch>
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestData {
    #[serde(rename = "startContent")]
    pub start_content: String,
    #[serde(rename = "endContent")]
    pub end_content: String,

    pub txns: Vec<TestTxn>,
}

impl TestData {
    pub fn len(&self) -> usize {
        self.txns.iter()
            .map(|txn| { txn.patches.len() })
            .sum::<usize>()
    }

    pub fn is_empty(&self) -> bool {
        !self.txns.iter().any(|txn| !txn.patches.is_empty())
    }
}

// TODO: Make a try_ version of this method, which returns an appropriate Error object.
pub fn load_testing_data(filename: &str) -> TestData {
    // let start = SystemTime::now();
    // let mut file = File::open("benchmark_data/automerge-paper.json.gz").unwrap();
    let file = File::open(filename).unwrap();

    let reader = BufReader::new(file);
    // We could pass the GzDecoder straight to serde, but it makes it way slower to parse for
    // some reason.
    let mut reader = GzDecoder::new(reader);
    let mut raw_json = vec!();
    reader.read_to_end(&mut raw_json).unwrap();

    // println!("uncompress time {}", start.elapsed().unwrap().as_millis());

    // let start = SystemTime::now();
    let data: TestData = serde_json::from_reader(raw_json.as_slice()).unwrap();
    // println!("JSON parse time {}", start.elapsed().unwrap().as_millis());

    data
}

fn testing_data(name: &str) -> TestData {
    let filename = format!("benchmark_data/{}.json.gz", name);
    load_testing_data(&filename)
}

const DATASETS: &[&str] = &["automerge-paper", "rustcode", "sveltecomponent", "seph-blog1"];

fn realworld_benchmarks(c: &mut Criterion) {
    for name in DATASETS {
        let mut group = c.benchmark_group("realworld");
        // let mut group = c.benchmark_group("local");
        let test_data = testing_data(name);
        assert_eq!(test_data.start_content.len(), 0);

        group.throughput(Throughput::Elements(test_data.len() as u64));

        group.bench_function(BenchmarkId::new("apply", name), |b| {
            b.iter(|| {
                let mut rope = JumpRope::new();
                for txn in test_data.txns.iter() {
                    for TestPatch(pos, del_span, ins_content) in &txn.patches {
                        if *del_span > 0 {
                            rope.remove(*pos .. *pos + *del_span);
                        }
                        if !ins_content.is_empty() {
                            rope.insert(*pos, ins_content);
                        }
                    }
                }

                assert_eq!(rope.len_bytes(), test_data.end_content.len());
                black_box(rope.len_chars());
            })
        });

        group.finish();
    }
}

criterion_group!(benches, realworld_benchmarks);
criterion_main!(benches);
