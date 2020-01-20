extern crate clap;
extern crate num_cpus;
extern crate scoped_threadpool;
extern crate stopwatch;
extern crate zim;

use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;

use clap::{App, Arg};
use scoped_threadpool::Pool;
use stopwatch::Stopwatch;

use zim::{Cluster, DirectoryEntry, MimeType, Namespace, Target, Zim};

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

    ensure_dir(root_output);

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

    let threads = num_cpus::get();
    println!(
        "  Spawning {} tasks across {} threads",
        cluster_map.len(),
        threads
    );

    let (tx, rx) = mpsc::channel();

    let mut pool = Pool::new(threads as u32);
    let zim_file_arc = Arc::new(&zim_file);

    pool.scoped(|scope| {
        for (cid, entries) in cluster_map {
            let zim_file_arc = zim_file_arc.clone();
            let tx = tx.clone();
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
                        skip_link,
                        &tx,
                    );
                }
            });
        }
    });

    for (src, dst) in rx.try_iter() {
        make_link(src, dst, flatten_link)
    }

    println!("  Extraction done in {}ms", sw.elapsed_ms());

    if let Some(main_page_idx) = zim_file.header.main_page {
        let page = zim_file
            .get_by_url_index(main_page_idx)
            .expect("failed to get main page");
        println!("  Main page is {}", page.url);
    }
}

fn safe_write(path: &Path, data: &[u8], count: usize) {
    let display = path.display();
    let contain_path = path.parent().unwrap();

    ensure_dir(contain_path);

    match File::create(&path) {
        Err(why) => {
            eprintln!("couldn't create {}: {}", display, why.description());

            if count < 3 {
                safe_write(path, data, count + 1);
            } else {
                panic!("failed retry: couldn't create {}: {:?}", display, why,);
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
    if path.exists() {
        // already done
        return;
    }

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
    skip_link: bool,
    tx: &mpsc::Sender<(PathBuf, PathBuf)>,
) {
    let dst = make_path(root_output, entry.namespace, &entry.url, &entry.mime_type);

    if entry.target.is_none() {
        println!("skipping missing target {:?} {:?}", dst, entry);
        return;
    }

    match entry.target.as_ref().unwrap() {
        &Target::Cluster(_, bid) => {
            let cluster = cluster.as_mut().unwrap();
            let blob = cluster.get_blob(bid).expect("failed to get blob");

            safe_write(&dst, blob, 1);
        }
        &Target::Redirect(redir) => {
            if !skip_link && !dst.exists() {
                let entry = {
                    zim_file
                        .get_by_url_index(redir)
                        .expect("failed to get_by_url_index")
                };
                let src = make_path(root_output, entry.namespace, &entry.url, &entry.mime_type);
                tx.send((src, dst)).expect("couldn't send");
            }
        }
    }
}

fn make_link(src: PathBuf, dst: PathBuf, flatten_link: bool) {
    if !src.exists() {
        println!("Link source doesn't exist: {}", src.display());
    } else if !dst.exists() {
        if flatten_link {
            std::fs::copy(src, dst).expect("failed to copy");
        } else {
            std::fs::hard_link(src, dst).expect("failed to create link");
        }
    }
}

fn make_path(root: &Path, namespace: Namespace, url: &str, mime_type: &MimeType) -> PathBuf {
    let mut s = String::new();
    s.push(namespace as u8 as char);
    let mut path = if url.starts_with("/") {
        // make absolute urls relative to the output folder
        let url = url.replacen("/", "", 1);
        root.join(&s).join(url)
    } else {
        root.join(&s).join(url)
    };

    if let MimeType::Type(ref typ) = mime_type {
        if typ == "text/html"
            && (path.extension().is_none() || path.extension().unwrap().is_empty())
        {
            path.set_extension("html");
        }
    }

    path
}
