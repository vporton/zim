extern crate zim;
extern crate clap;
extern crate stopwatch;
extern crate pbr;

use clap::{Arg, App};
use zim::{Zim, Target};
use std::fs::File;
use std::io::Write;
use std::collections::HashMap;
use std::path::Path;
use std::thread;
use stopwatch::Stopwatch;
use pbr::MultiBar;

fn main() {
    let matches = App::new("zimextractor")
        .version("0.1")
        .about("Extract zim files")
        .arg(Arg::with_name("out")
                 .short("o")
                 .long("out")
                 .help("Output directory")
                 .takes_value(true))
        .arg(Arg::with_name("INPUT")
                 .help("Set the zim file to extract")
                 .required(true)
                 .index(1))
        .get_matches();

    let out = matches.value_of("out").unwrap_or("out");
    let root_output = Path::new(out);

    let input = matches.value_of("INPUT").unwrap();
    let mut mb = MultiBar::new();

    mb.println(&format!("Extracting file: {} to {}:", input, out));
    mb.println("");

    let sw = Stopwatch::start_new();

    let zim = Zim::new(input).ok().unwrap();

    // map between cluster and directory entry
    let mut cluster_map = HashMap::new();

    let mut p1 = mb.create_bar(zim.header.cluster_count as u64);
    let mut p2 = mb.create_bar(zim.header.cluster_count as u64);
    let mut p3 = mb.create_bar(zim.header.cluster_count as u64);

    thread::spawn(move || { mb.listen(); });

    p1.show_message = true;
    p1.message("Building cluster map :");

    for i in zim.iterate_by_urls() {
        if let Some(Target::Cluster(cid, _)) = i.target {
            cluster_map.entry(cid).or_insert(Vec::new()).push(i);
        }
        p1.inc();
    }

    p1.finish_print("Created cluster map");

    p2.show_message = true;
    p2.message("Extracting entries :");

    // extract all non redirect entries
    for (cid, entries) in cluster_map {
        //println!("{}", cid);
        //println!("{:?}", entries);
        let cluster = zim.get_cluster(cid).unwrap();

        for entry in entries {
            if let Some(Target::Cluster(_cid, bid)) = entry.target {
                assert_eq!(cid, _cid);
                let mut s = String::new();
                s.push(entry.namespace);
                let out_path = root_output.join(&s).join(&entry.url);
                std::fs::create_dir_all(out_path.parent().unwrap());
                let data = cluster.get_blob(bid);
                let mut f = File::create(&out_path).unwrap();
                f.write_all(data);
                //println!("{} written to {}", entry.url, out_path.display());
            }
        }
        p2.inc();
        // println!("Finished processing cluster {} of {} ({}%)",
        //          c,
        //          zim.header.cluster_count,
        //          c * 100 / zim.header.cluster_count);
    }

    p2.finish_print(&format!("Extraction done in {}ms", sw.elapsed_ms()));

    p3.show_message = true;
    p3.message("Linking redirects :");

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
        p3.inc();
    }

    p3.finish_print(&format!("Linking done in {}ms", sw.elapsed_ms()));

    if let Some(main_page_idx) = zim.header.main_page {
        let page = zim.get_by_url_index(main_page_idx).unwrap();
        println!("Main page is {}", page.url);
    }
}
