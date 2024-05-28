use clap::Parser;
use rmp;
use rmp_serde;
use rmpv;
use serde::{Deserialize, Serialize};
use serde_bytes;
use serde_json;
use std::collections::BTreeMap;
use std::fs::{create_dir, read_dir, File};
use std::io::{prelude::*, BufReader, BufWriter, SeekFrom};
use std::path::PathBuf;

use lz4::block::{compress, decompress};

#[derive(Serialize, Deserialize)]
struct HeaderEntry {
    offset: u32,
    length: u32,
}

type InfoHeader = BTreeMap<String, HeaderEntry>;

#[derive(Serialize)]
#[serde(rename = "_ExtStruct")]
struct ExtStruct((i8, serde_bytes::ByteBuf));

#[derive(Parser)]
#[command(version, about, long_about = None, after_help = "`¶cﾘ˘ヮ˚)|")]
struct Cli {
    #[arg(short, long)]
    quiet: bool,
    #[arg(short, long)]
    no_compress: bool,
    path: PathBuf,
}

fn main() {
    let args = Cli::parse();
    if args.path.is_file() {
        unpack(&args.path, args.quiet);
    } else {
        repack(&args.path, args.quiet, !args.no_compress);
    }
}

fn unpack(path: &PathBuf, quiet: bool) {
    let mut db_file = File::open(path).unwrap();
    let header: InfoHeader = rmp_serde::from_read(&db_file).unwrap();
    let offset = db_file.seek(SeekFrom::Current(0)).unwrap();
    let dirpath = format!("{}_unpacked", path.file_stem().unwrap().to_str().unwrap());
    create_dir(&dirpath).expect("Cannot create directory!");
    for (name, entry) in &header {
        if !quiet {
            println!("unpacking: {}.json", name);
        }
        db_file
            .seek(SeekFrom::Start(offset + entry.offset as u64))
            .unwrap();
        let data: rmpv::Value = rmp_serde::from_read(&db_file).unwrap();
        let outfile = File::create(format!("{}/{}.json", &dirpath, name)).unwrap();
        let bw = BufWriter::new(outfile);
        if data.is_ext() {
            let data = data.as_ext().unwrap();
            let size: i32 = rmp_serde::from_slice(data.1).unwrap();
            let uncompressed = decompress(&data.1[5..], Some(size)).unwrap();
            let msgpack = rmp_serde::from_slice::<rmpv::Value>(&uncompressed).unwrap();
            serde_json::to_writer(bw, &msgpack).unwrap();
        } else {
            serde_json::to_writer(bw, &data).unwrap();
        }
    }
}

fn repack(path: &PathBuf, quiet: bool, do_compress: bool) {
    let mut header = InfoHeader::new();
    let mut body: Vec<u8> = vec![];
    let mut offset = 0;

    for entry in read_dir(path).unwrap() {
        if let Ok(entry) = entry {
            let jsonfile = File::open(entry.path()).unwrap();
            let reader = BufReader::new(jsonfile);
            let data: rmpv::Value = serde_json::from_reader(reader).unwrap();
            let data_vec = rmp_serde::to_vec(&data).unwrap();
            let mut packed = if do_compress && data_vec.len() > 64 {
                let mut compressed = vec![];
                rmp::encode::write_i32(&mut compressed, data_vec.len() as i32).unwrap();
                let mut data_compressed = compress(&data_vec, None, false).unwrap();
                compressed.append(&mut data_compressed);
                rmp_serde::to_vec(&ExtStruct((99, serde_bytes::ByteBuf::from(compressed)))).unwrap()
            } else {
                data_vec
            };
            if !quiet {
                println!(
                    "repacking: {}",
                    entry.path().file_stem().unwrap().to_str().unwrap()
                );
            }
            header.insert(
                entry
                    .path()
                    .file_stem()
                    .unwrap()
                    .to_os_string()
                    .into_string()
                    .unwrap(),
                HeaderEntry {
                    offset,
                    length: packed.len() as u32,
                },
            );
            offset += packed.len() as u32;
            body.append(&mut packed);
        }
    }
    let binaryfile = File::create(format!(
        "{}.bytes",
        path.file_name().unwrap().to_str().unwrap()
    ))
    .unwrap();
    let mut writer = BufWriter::new(binaryfile);
    let header_vec = rmp_serde::to_vec(&header).unwrap();
    writer.write_all(&header_vec).unwrap();
    writer.write_all(&body).unwrap();
}
