use criterion::*;
use crdt_testdata::*;

use jumprope::{JumpRope, JumpRopeBuf};

fn count_chars(s: &String) -> usize {
    s.chars().count()
}

#[derive(Debug, Clone)]
enum Op {
    Ins(usize, String),
    Del(usize, usize),
}
use Op::*;

fn collapse(test_data: &TestData) -> Vec<Op> {
    let mut result = Vec::new();

    let mut merge = |op: Op| {
        let append = match (&op, result.last_mut()) {
            (Ins(pos, new_content), Some(Ins(cur_pos, cur_content))) => {
                if *pos == *cur_pos + count_chars(&cur_content) {
                    cur_content.push_str(new_content.as_str());
                    false
                } else { true }
            }
            (Del(pos, new_del), Some(Del(cur_pos, cur_del))) => {
                if *pos == *cur_pos {
                    // The new delete follows the old.
                    *cur_del += *new_del;
                    false
                } else if *pos + *new_del == *cur_pos {
                    // The new delete is a backspace (before the old)
                    *cur_pos = *pos;
                    *cur_del += *new_del;
                    false
                } else {
                    true
                }
            }
            _ => true,
        };

        if append { result.push(op); }
    };

    for txn in test_data.txns.iter() {
        for TestPatch(pos, del_span, ins_content) in &txn.patches {
            if *del_span > 0 {
                merge(Op::Del(*pos, *del_span));
            }
            if !ins_content.is_empty() {
                merge(Op::Ins(*pos, ins_content.clone()));
            }
        }
    }
    result
}

fn testing_data(name: &str) -> TestData {
    let filename = format!("benchmark_data/{}.json.gz", name);
    load_testing_data(&filename)
}

const DATASETS: &[&str] = &["automerge-paper", "rustcode", "sveltecomponent", "seph-blog1"];

fn realworld_benchmarks(c: &mut Criterion) {
    for name in DATASETS {
        let mut group = c.benchmark_group("testdata");
        // let mut group = c.benchmark_group("local");
        let test_data = testing_data(name);
        let merged = collapse(&test_data);
        assert_eq!(test_data.start_content.len(), 0);

        let len = test_data.txns.iter()
            .flat_map(|txn| txn.patches.iter() )
            .map(|patch| patch.1 + patch.2.len())
            .sum::<usize>();
        group.throughput(Throughput::Elements(len as u64));

        group.bench_function(BenchmarkId::new("direct", name), |b| {
            b.iter(|| {
                let mut rope = JumpRope::new();
                for txn in test_data.txns.iter() {
                    for TestPatch(pos, del_span, ins_content) in &txn.patches {
                        rope.replace(*pos .. *pos + *del_span, ins_content);
                        // if *del_span > 0 {
                        //     rope.remove(*pos .. *pos + *del_span);
                        // }
                        // if !ins_content.is_empty() {
                        //     rope.insert(*pos, ins_content);
                        // }
                    }
                }

                assert_eq!(rope.len_bytes(), test_data.end_content.len());
                black_box(rope.len_chars());
            })
        });

        // group.bench_function(BenchmarkId::new("merged", name), |b| {
        //     b.iter(|| {
        //         let mut rope = JumpRope::new();
        //         for op in merged.iter() {
        //             match op {
        //                 Ins(pos, content) => {
        //                     rope.insert(*pos, content);
        //                 }
        //                 Del(pos, del_span) => {
        //                     rope.remove(*pos..*pos + *del_span);
        //                 }
        //             }
        //         }
        //
        //         // assert_eq!(test_data.end_content, rope.to_string());
        //
        //         assert_eq!(rope.len_bytes(), test_data.end_content.len());
        //         black_box(rope.len_chars());
        //     })
        // });

        group.bench_function(BenchmarkId::new("buffered", name), |b| {
            b.iter(|| {
                let mut rope = JumpRopeBuf::new();
                for op in merged.iter() {
                    match op {
                        Ins(pos, content) => {
                            rope.insert(*pos, content);
                        }
                        Del(pos, del_span) => {
                            rope.remove(*pos..*pos + *del_span);
                        }
                    }
                }

                // assert_eq!(test_data.end_content, rope.to_string());

                let rope = rope.into_inner();
                assert_eq!(rope.len_bytes(), test_data.end_content.len());
                black_box(rope.len_chars());
            })
        });

        group.finish();
    }
}

criterion_group!(benches, realworld_benchmarks);
criterion_main!(benches);
