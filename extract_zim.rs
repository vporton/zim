extern crate zim;
extern crate clap;
extern crate stopwatch;
extern crate crossbeam;

use std::fs::File;
use std::io::{Write, BufWriter};
use std::error::Error;
use std::collections::HashMap;
use std::path::Path;

use clap::{Arg, App};
use stopwatch::Stopwatch;

use zim::{Zim, Target};

fn main() {
    let matches = App::new("zimextractor")
        .version("0.1")
        .about("Extract zim files")
        .arg(Arg::with_name("out")
                 .short("o")
                 .long("out")
                 .help("Output directory")
                 .takes_value(true))
        .arg(Arg::with_name("skip-link")
                 .long("skip-link")
                 .help("Skip genrating hard links")
                 .takes_value(false))
        .arg(Arg::with_name("INPUT")
                 .help("Set the zim file to extract")
                 .required(true)
                 .index(1))
        .get_matches();

    let skip_link = matches.is_present("skip-link");
    let out = matches.value_of("out").unwrap_or("out");
    let root_output = Path::new(out);

    let input = matches.value_of("INPUT").unwrap();

    println!("Extracting file: {} to {}\n", input, out);

    let sw = Stopwatch::start_new();

    let zim = Zim::new(input).ok().unwrap();

    // map between cluster and directory entry
    let mut cluster_map = HashMap::new();

    println!("  Creating cluster map");
    for i in zim.iterate_by_urls() {
        if let Some(Target::Cluster(cid, _)) = i.target {
            cluster_map.entry(cid).or_insert(Vec::new()).push(i);
        }
    }

    println!("  Extracting entries: {}", zim.header.cluster_count);

    crossbeam::scope(|scope| for (cid, entries) in cluster_map {
                         let mut cluster = zim.get_cluster(cid).unwrap();

                         scope.spawn(move || {
            cluster.decompress();

            for entry in entries {
                if let Some(Target::Cluster(_cid, bid)) = entry.target {
                    assert_eq!(cid, _cid);
                    let mut s = String::new();
                    s.push(entry.namespace);
                    let out_path = root_output.join(&s).join(&entry.url);
                    safe_write(&out_path, cluster.get_blob(bid), 0);
                }
            }
        });
                     });

    println!("  Extraction done in {}ms", sw.elapsed_ms());

    if !skip_link {
        println!("  Linking redirects");

        // link all redirects
        for entry in zim.iterate_by_urls() {
            // get redirect entry
            if let Some(Target::Redirect(redir)) = entry.target {
                let redir = zim.get_by_url_index(redir).unwrap();

                let mut s = String::new();
                s.push(redir.namespace);
                let src = root_output.join(&s).join(&redir.url);

                let mut d = String::new();
                d.push(entry.namespace);
                let dst = root_output.join(&s).join(&entry.url);

                if !dst.exists() {
                    // println!("{:?} -> {:?}", src, dst);
                    std::fs::hard_link(src, dst).unwrap();
                }
            }
        }

        println!("  Linking done in {}ms", sw.elapsed_ms());
    } else {
        println!("  Skipping linking...");
    }

    if let Some(main_page_idx) = zim.header.main_page {
        let page = zim.get_by_url_index(main_page_idx).unwrap();
        println!("  Main page is {}", page.url);
    }
}

fn safe_write(path: &Path, data: &[u8], count: usize) {
    let display = path.display();
    let contain_path = path.parent().unwrap();

    ensure_dir(contain_path);

    match File::create(&path) {
        Err(why) => {
            println!("couldn't create {}: {}", display, why.description());

            if count == 1 {
                safe_write(path, data, 2);
            } else {
                panic!("failed retry: couldn't create {}: {}",
                       display,
                       why.description());
            }
        }
        Ok(file) => {
            let mut writer = BufWriter::new(&file);

            if let Err(why) = writer.write_all(data) {
                println!("couldn't write to {}: {}", display, why.description());
            }
        }
    }
}

fn ensure_dir(path: &Path) {
    match std::fs::create_dir_all(path) {
        Err(e) => {
            use std::io::ErrorKind::*;

            match e.kind() {
                // do not panic if it already exists, that's fine, we just want to make
                // sure we have it before moving on
                AlreadyExists => {}
                _ => {
                    panic!("failed to create {}: {}", path.display(), e.description());
                }
            }
        }
        _ => {}
    }
}
