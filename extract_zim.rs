extern crate zim;
extern crate clap;

use clap::{Arg, App};
use zim::{Zim, Target};
use std::fs::File;
use std::io::Write;
use std::collections::HashMap;
use std::path::Path;

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
    
    println!("Extracting file: {} to {}", input, out);
    
    let zim = Zim::new(input).ok().unwrap();

    // map between cluster and directory entry
    let mut cluster_map = HashMap::new();

    println!("Building cluster map...");

    for i in zim.iterate_by_urls() {
        if let Some(Target::Cluster(cid, _)) = i.target {

            cluster_map.entry(cid).or_insert(Vec::new()).push(i);
        }
        //println!("{:?}", i);
        //if c > 10 { break; }
        //c += 1;
    }
    println!("Done!");

    // extract all non redirect entries
    let mut c = 0;
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
        c += 1;
        println!("Finished processing cluster {} of {} ({}%)", c, zim.cluster_count, c * 100 / zim.cluster_count);
    }

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
                println!("{:?} -> {:?}", src, dst);
                std::fs::hard_link(src, dst).unwrap();
            }
        }
    }

    if let Some(main_page_idx) = zim.main_page_idx {
        let page = zim.get_by_url_index(main_page_idx).unwrap();
        println!("Main page is {}", page.url);
    }
}
