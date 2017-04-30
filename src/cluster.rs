use std::io::Cursor;

use byteorder::{LittleEndian, ReadBytesExt};
use xz2::read::XzDecoder;
use std::io::Read;

use errors::ParsingError;
use zim::Zim;

/// A cluster of blobs
///
/// Within an ZIM archive, clusters contain several blobs of data that are all compressed together.
/// Each blob is the data for an article.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Cluster {
    start_off: u64,
    end_off: u64,
    comp_type: u8,
    blob_list: Vec<u32>, // offsets into data
    data: Vec<u8>,
}

impl Cluster {
    pub fn new(zim: &Zim, idx: u32) -> Result<Cluster, ParsingError> {
        let idx = idx as usize;
        let this_cluster_off = zim.cluster_list[idx];
        let next_cluster_off = if idx < zim.cluster_list.len() - 1 {
            zim.cluster_list[idx + 1]
        } else {
            zim.header.checksum_pos
        };

        assert!(next_cluster_off > this_cluster_off);
        let total_cluster_size: usize = (next_cluster_off - this_cluster_off) as usize;

        let cluster_view = {
            let mut view = unsafe { zim.master_view.clone() };
            view.restrict(this_cluster_off as usize, total_cluster_size).ok();
            view
        };
        let slice = unsafe { cluster_view.as_slice() };
        let comp_type = slice[0];
        let mut blob_list = Vec::new();
        let data: Vec<u8> = if comp_type == 4 {
            let mut decoder = XzDecoder::new(&slice[1..total_cluster_size]);
            let mut data = Vec::new();
            try!(decoder.read_to_end(&mut data));
            
            // println!("Decompressed {} bytes of data", data.len());
            data
        } else {
            Vec::from(&slice[1..total_cluster_size])
        };
        let datalen = data.len();
        {
            let mut cur = Cursor::new(&data);
            loop {
                let offset = try!(cur.read_u32::<LittleEndian>());
                blob_list.push(offset);
                if offset as usize >= datalen {
                    //println!("at end");
                    break;
                }
            }
        }

        Ok(Cluster {
               comp_type: comp_type,
               start_off: this_cluster_off,
               end_off: next_cluster_off,
               data: data,
               blob_list: blob_list,
           })

    }

    pub fn get_blob(&self, idx: u32) -> &[u8] {
        let this_blob_off = self.blob_list[idx as usize] as usize;
        let n = idx as usize + 1;
        if self.blob_list.len() > n {
            let next_blob_off = self.blob_list[n] as usize;
            &self.data[this_blob_off..next_blob_off]
        } else {
            &self.data[this_blob_off..]
        }
    }
}
