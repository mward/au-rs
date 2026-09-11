//! Criterion benchmarks for the au encoder and decoder.
//!
//! Run with `cargo bench`. `encode/record` measures streaming, amortized
//! per-record encoding (the dictionary warms up as in a real stream);
//! `encode/1000_records` measures a full cold pipeline; `decode/1000_records`
//! measures parsing a pre-encoded stream back into values.

use au::byte_source::BufferByteSource;
use au::decoder::parse_stream;
use au::dictionary::Dictionary;
use au::encoder::AuEncoder;
use au::handler::ValueHandler;
use au::record_handler::AuRecordHandler;
use au::writer::AuWriter;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

/// A representative record: a mixed object with repeated keys (so interning and
/// dictionary backrefs come into play) plus a small nested array.
fn write_sample_record(w: &mut AuWriter, i: u64) {
    w.map(|w| {
        w.kv_str("event", "order_update");
        w.kv_u64("seq", i);
        w.kv_str("symbol", "AAPL");
        w.kv_f64("price", 123.45 + (i % 100) as f64);
        w.kv_i64("qty", (i % 1000) as i64);
        w.kv_bool("active", i.is_multiple_of(2));
        w.key("tags");
        w.array(|w| {
            w.value_str("fix");
            w.value_str("equity");
            w.value_str("us");
        });
    });
}

fn encode_to_vec(records: usize) -> Vec<u8> {
    let mut enc = AuEncoder::new();
    let mut out = Vec::with_capacity(records * 64);
    for i in 0..records as u64 {
        let sink = &mut out;
        enc.encode(
            |w| write_sample_record(w, i),
            |a, b| {
                sink.extend_from_slice(a);
                sink.extend_from_slice(b);
                a.len() + b.len()
            },
        );
    }
    out
}

/// Counts decoded scalars/strings so the decoder's work can't be optimized away.
#[derive(Default)]
struct CountingValueHandler {
    count: u64,
}

impl ValueHandler for CountingValueHandler {
    fn on_bool(&mut self, _pos: usize, _val: bool) {
        self.count += 1;
    }
    fn on_int(&mut self, _pos: usize, _val: i64) {
        self.count += 1;
    }
    fn on_uint(&mut self, _pos: usize, _val: u64) {
        self.count += 1;
    }
    fn on_double(&mut self, _pos: usize, _val: f64) {
        self.count += 1;
    }
    fn on_string_end(&mut self) {
        self.count += 1;
    }
}

fn bench_encode(c: &mut Criterion) {
    // Full cold pipeline: fresh encoder + output buffer for 1000 records.
    const N: usize = 1000;
    let mut group = c.benchmark_group("encode");

    // Streaming, amortized per-record encode: one long-lived encoder whose
    // dictionary warms up, matching real usage.
    group.throughput(Throughput::Elements(1));
    group.bench_function("record", |b| {
        let mut enc = AuEncoder::new();
        let mut sink = Vec::with_capacity(4096);
        let mut i = 0u64;
        b.iter(|| {
            sink.clear();
            let out = &mut sink;
            enc.encode(
                |w| write_sample_record(w, black_box(i)),
                |a, bb| {
                    out.extend_from_slice(a);
                    out.extend_from_slice(bb);
                    a.len() + bb.len()
                },
            );
            i += 1;
            black_box(sink.len())
        });
    });

    group.throughput(Throughput::Elements(N as u64));
    group.bench_function("1000_records", |b| {
        b.iter(|| black_box(encode_to_vec(black_box(N)).len()));
    });

    group.finish();
}

/// Encode `n` records whose string *values* are all distinct, so the frequency
/// intern cache never promotes them and churns through evictions — the workload
/// that stresses `UsageTracker`'s eviction path.
fn encode_unique_values(n: usize) -> usize {
    let mut enc = AuEncoder::new();
    let mut sink = Vec::with_capacity(4096);
    let mut total = 0usize;
    for i in 0..n as u64 {
        sink.clear();
        let out = &mut sink;
        enc.encode(
            |w| {
                w.map(|w| {
                    w.kv_str("event", "order_update"); // stable key+value (interned)
                    w.kv_str("id", &format!("uid-{i:012x}")); // unique every record
                });
            },
            |a, b| {
                out.extend_from_slice(a);
                out.extend_from_slice(b);
                a.len() + b.len()
            },
        );
        total += sink.len();
    }
    total
}

fn bench_encode_high_cardinality(c: &mut Criterion) {
    const N: usize = 20_000;
    let mut group = c.benchmark_group("encode_high_cardinality");
    group.throughput(Throughput::Elements(N as u64));
    group.bench_function("unique_values_20000", |b| {
        b.iter(|| black_box(encode_unique_values(black_box(N))));
    });
    group.finish();
}

/// Encode `n` records each with a distinct *long* key. Keys are force-interned,
/// so each is created and stored in the dictionary — a high-churn workload of
/// many long interned strings, unlike the few short reused ones elsewhere.
fn encode_unique_long_keys(n: usize) -> usize {
    let mut enc = AuEncoder::new();
    let mut sink = Vec::with_capacity(4096);
    let mut total = 0usize;
    for i in 0..n as u64 {
        sink.clear();
        let out = &mut sink;
        enc.encode(
            |w| {
                w.map(|w| {
                    // ~48-byte unique key each record -> interned.
                    w.kv_u64(&format!("long_descriptive_metric_field_name_number_{i:08}"), i);
                });
            },
            |a, b| {
                out.extend_from_slice(a);
                out.extend_from_slice(b);
                a.len() + b.len()
            },
        );
        total += sink.len();
    }
    total
}

fn bench_encode_long_keys(c: &mut Criterion) {
    const N: usize = 20_000;
    let mut group = c.benchmark_group("encode_long_keys");
    group.throughput(Throughput::Elements(N as u64));
    group.bench_function("unique_long_keys_20000", |b| {
        b.iter(|| black_box(encode_unique_long_keys(black_box(N))));
    });
    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    const N: usize = 1000;
    let data = encode_to_vec(N);

    let mut group = c.benchmark_group("decode");
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.bench_function("1000_records", |b| {
        b.iter(|| {
            let mut source = BufferByteSource::new(&data);
            let mut dictionary = Dictionary::new();
            let mut handler = CountingValueHandler::default();
            let mut record_handler = AuRecordHandler::new(&mut dictionary, &mut handler);
            parse_stream(&mut source, &mut record_handler, true).unwrap();
            drop(record_handler);
            black_box(handler.count)
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_encode,
    bench_encode_high_cardinality,
    bench_encode_long_keys,
    bench_decode
);
criterion_main!(benches);
