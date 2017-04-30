extern crate zim;
extern crate clap;
extern crate stopwatch;
extern crate crossbeam;

use clap::{Arg, App};
use zim::{Zim, Target};
use std::fs::File;
use std::io::{Write, BufWriter};
use std::collections::HashMap;
use std::path::Path;
use stopwatch::Stopwatch;

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
                    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
                    let data = cluster.get_blob(bid);
                    let f = File::create(&out_path)
                        .ok()
                        .expect(&format!("failed to write file {:?}", out_path));
                    let mut writer = BufWriter::new(&f);
                    writer.write_all(data).unwrap();
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
