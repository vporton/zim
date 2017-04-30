use std::io::Cursor;
use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use xz2::read::XzDecoder;
use memmap::MmapViewSync;

use errors::ParsingError;
use zim::Zim;

/// A cluster of blobs
///
/// Within an ZIM archive, clusters contain several blobs of data that are all compressed together.
/// Each blob is the data for an article.
#[allow(dead_code)]
pub struct Cluster {
    start_off: u64,
    end_off: u64,
    comp_type: u8,
    size: usize,
    view: MmapViewSync,
    blob_list: Option<Vec<u32>>, // offsets into data
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
            view.restrict(this_cluster_off as usize, total_cluster_size)
                .ok();
            view
        };

        Ok(Cluster {
               comp_type: unsafe { cluster_view.as_slice()[0] },
               start_off: this_cluster_off,
               end_off: next_cluster_off,
               data: Vec::new(),
               size: total_cluster_size,
               view: cluster_view,
               blob_list: None,
           })
    }

    pub fn decompress(&mut self) {
        let slice = unsafe { self.view.as_slice() };

        self.data = if self.comp_type == 4 {
            let mut decoder = XzDecoder::new(&slice[1..self.size]);
            let mut d = Vec::new();
            decoder
                .read_to_end(&mut d)
                .ok()
                .expect("failed to decompress");
            d
        } else {
            Vec::from(&slice[1..self.size])
        };

        let mut blob_list = Vec::new();
        let datalen = self.data.len();
        {
            let mut cur = Cursor::new(&self.data);
            loop {
                let offset = cur.read_u32::<LittleEndian>()
                    .ok()
                    .expect("failed to read blob-list");
                blob_list.push(offset);
                if offset as usize >= datalen {
                    //println!("at end");
                    break;
                }
            }
        }
        self.blob_list = Some(blob_list);
    }

    pub fn get_blob(&mut self, idx: u32) -> &[u8] {
        // delay decompression until needed
        match self.blob_list {
            None => self.decompress(),
            _ => {}
        }

        match self.blob_list {
            Some(ref list) => {

                let this_blob_off = list[idx as usize] as usize;
                let n = idx as usize + 1;
                if list.len() > n {
                    let next_blob_off = list[n] as usize;
                    &self.data[this_blob_off..next_blob_off]
                } else {
                    &self.data[this_blob_off..]
                }
            }
            _ => panic!("no blob_list found"),
        }
    }
}
