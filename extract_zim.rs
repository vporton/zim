extern crate zim;
extern crate clap;
extern crate stopwatch;
extern crate scoped_threadpool;

use std::fs::File;
use std::io::{Write, BufWriter};
use std::error::Error;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Arg, App};
use stopwatch::Stopwatch;
use scoped_threadpool::Pool;

use zim::{Zim, Target, DirectoryEntry, Cluster};

fn main() {
    let matches = App::new("zimextractor")
        .version("0.1")
        .about("Extract zim files")
        .arg(
            Arg::with_name("out")
                .short("o")
                .long("out")
                .help("Output directory")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("skip-link")
                .long("skip-link")
                .help("Skip genrating hard links")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("flatten-link")
                .long("flatten-link")
                .help("Write files to disk, instead of using hard links")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("INPUT")
                .help("Set the zim file to extract")
                .required(true)
                .index(1),
        )
        .get_matches();

    let skip_link = matches.is_present("skip-link");
    let flatten_link = matches.is_present("flatten-link");
    let out = matches.value_of("out").unwrap_or("out");
    let root_output = Path::new(out);

    let input = matches.value_of("INPUT").unwrap();

    println!("Extracting file: {} to {}\n", input, out);

    let sw = Stopwatch::start_new();

    let zim_file = Zim::new(input).expect("failed to parse input");

    // map between cluster and directory entry
    let mut cluster_map = HashMap::new();

    println!("  Creating map");
    let link_entry = zim_file.header.cluster_count + 1;
    for i in zim_file.iterate_by_urls() {
        if let Some(Target::Cluster(cid, _)) = i.target {
            cluster_map.entry(cid).or_insert(Vec::new()).push(i);
        } else if !skip_link {
            cluster_map.entry(link_entry).or_insert(Vec::new()).push(i);
        }
    }

    println!("  Extracting entries: {}", zim_file.header.cluster_count);
    if !skip_link {
        println!("  Extracting links: {}", cluster_map.get(&0).unwrap().len());
    }

    println!("  Spawning {} threads", cluster_map.len());


    let mut pool = Pool::new(16);
    let zim_file_arc = Arc::new(&zim_file);

    pool.scoped(|scope| for (cid, entries) in cluster_map {
        let zim_file_arc = zim_file_arc.clone();

        scope.execute(move || {
            let mut cluster = if cid == link_entry {
                None
            } else {
                Some(zim_file_arc.get_cluster(cid).unwrap())
            };

            for entry in entries {
                process_target(
                    zim_file_arc.clone(),
                    &mut cluster,
                    root_output,
                    &entry,
                    flatten_link,
                    skip_link,
                );
            }
        });
    });

    println!("  Extraction done in {}ms", sw.elapsed_ms());

    if let Some(main_page_idx) = zim_file.header.main_page {
        let page = zim_file.get_by_url_index(main_page_idx).expect(
            "failed to get main page",
        );
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

            if count < 3 {
                safe_write(path, data, count + 1);
            } else {
                panic!(
                    "failed retry: couldn't create {}: {}",
                    display,
                    why.description()
                );
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

fn process_target(
    zim_file: Arc<&Zim>,
    cluster: &mut Option<Cluster>,
    root_output: &Path,
    entry: &DirectoryEntry,
    flatten_link: bool,
    skip_link: bool,
) {
    let dst = make_path(root_output, entry.namespace, &entry.url);

    if entry.target.is_none() {
        println!("skipping missing target {:?} {:?}", dst, entry);
        return ();
    }

    match entry.target.as_ref().unwrap() {
        Target::Cluster(_, bid) => {
            let mut cluster = cluster.as_mut().unwrap();
            let blob = cluster.get_blob(*bid).expect("failed to get blob");

            safe_write(&dst, blob, 1);
        }
        Target::Redirect(redir) => {
            if !skip_link && !dst.exists() {
                let entry = {
                    // let zim_file = zim_file.lock().expect("failed to get zim_file lock");
                    zim_file.get_by_url_index(*redir).expect(
                        "failed to get_by_url_index",
                    )
                };
                // let redir = Arc::new(&entry);

                let src = make_path(root_output, entry.namespace, &entry.url);

                if flatten_link {
                    if src.exists() {
                        std::fs::copy(src, dst).expect("failed to copy");
                    } else {
                        process_target(
                            zim_file,
                            cluster,
                            root_output,
                            &entry, //redir.clone(),
                            flatten_link,
                            skip_link,
                        );
                    }
                } else {
                    std::fs::hard_link(src, dst).expect("failed to create link");
                }
            }
        }
    }
}

fn make_path(root: &Path, namespace: char, url: &str) -> PathBuf {
    let mut s = String::new();
    s.push(namespace);
    if url.starts_with("/") {
        // make absolute urls relative to the output folder
        let url = url.replacen("/", "", 1);
        root.join(&s).join(url)
    } else {
        root.join(&s).join(url)
    }
}
