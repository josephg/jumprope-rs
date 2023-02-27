// use std::time::SystemTime;
use std::fs::File;
use std::io::{BufReader, Read};
use flate2::bufread::GzDecoder;
use serde::Deserialize;

/// This file contains some simple helpers for loading test data. Its used by benchmarking and
/// testing code.

/// (position, delete length, insert content).
#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
pub struct TestPatch(pub usize, pub usize, pub String);

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
pub struct TestTxn {
    // time: String, // ISO String. Unused.
    pub patches: Vec<TestPatch>
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
pub struct TestData {
    #[serde(default)]
    pub using_byte_positions: bool,

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

    /// This method returns a clone of the testing data using byte offsets instead of codepoint
    /// indexes.
    pub fn chars_to_bytes(&self) -> Self {
        assert_eq!(false, self.using_byte_positions);

        let mut r = ropey::Rope::new();

        Self {
            using_byte_positions: true,
            start_content: self.start_content.clone(),
            end_content: self.end_content.clone(),
            txns: self.txns.iter().map(|txn| {
                TestTxn {
                    patches: txn.patches.iter().map(|TestPatch(pos_chars, del_chars, ins)| {
                        let pos_bytes = r.char_to_byte(*pos_chars);
                        // if *pos_chars != pos_bytes {
                        //     println!("Converted position {} to {}", *pos_chars, pos_bytes);
                        // }
                        let del_bytes = if *del_chars > 0 {
                            let del_end_bytes = r.char_to_byte(pos_chars + *del_chars);
                            r.remove(*pos_chars..*pos_chars + *del_chars);
                            del_end_bytes - pos_bytes
                        } else { 0 };
                        if !ins.is_empty() { r.insert(*pos_chars, ins); }

                        TestPatch(pos_bytes, del_bytes, ins.clone())
                    }).collect(),
                }
            }).collect()
        }
    }

    pub fn patches(&self) -> impl Iterator<Item=&TestPatch> {
        self.txns.iter().flat_map(|txn| txn.patches.iter())
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

#[cfg(test)]
mod tests {
    use crate::{load_testing_data, TestData, TestPatch, TestTxn};

    #[test]
    fn it_works() {
        let data = load_testing_data("../benchmark_data/sveltecomponent.json.gz");
        assert!(data.txns.len() > 0);
    }

    #[test]
    fn convert_chars_to_bytes() {
        let data = TestData {
            using_byte_positions: false,
            start_content: "".to_string(),
            end_content: "".to_string(),
            txns: vec![
                TestTxn {
                    patches: vec![
                        TestPatch(0, 0, "ツ".into()),
                        TestPatch(1, 0, "x".into()),
                        TestPatch(1, 1, "".into()),
                        TestPatch(0, 1, "".into()),
                    ],
                }
            ],
        };

        // let data = load_testing_data("../benchmark_data/seph-blog1.json.gz");
        // let data = load_testing_data("../benchmark_data/sveltecomponent.json.gz");
        let data2 = data.chars_to_bytes();
        dbg!(&data2);

        assert_eq!(data2, TestData {
            using_byte_positions: true,
            start_content: "".to_string(),
            end_content: "".to_string(),
            txns: vec![
                TestTxn {
                    patches: vec![
                        // Positions have changed!
                        TestPatch(0, 0, "ツ".into()),
                        TestPatch(3, 0, "x".into()),
                        TestPatch(3, 1, "".into()),
                        TestPatch(0, 3, "".into()),
                    ],
                }
            ],
        });

        // dbg!(&data2);

        for (p1, p2) in data.patches().zip(data2.patches()) {
            // assert_eq!(p1.1, p2.1);
            assert_eq!(p1.2, p2.2);
            if p1.1 != p2.1 {
                println!("{} / {} ({} {})", p1.0, p2.0, p1.1, p1.2);
            }

            // if p1.2.chars().count() != p1.2.len() {
            //     println!("unicode! {}", p1.2);
            // }
        }
    }
}
