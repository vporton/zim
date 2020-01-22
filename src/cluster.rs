use std::fmt;
use std::io::Cursor;
use std::io::Read;

use bitreader::BitReader;
use byteorder::{LittleEndian, ReadBytesExt};
use memmap::Mmap;
use xz2::read::XzDecoder;

use crate::errors::{Error, Result};

#[repr(u8)]
#[derive(Debug)]
pub enum Compression {
    None = 0,
    LZMA2 = 4,
}

impl From<Compression> for u8 {
    fn from(mode: Compression) -> u8 {
        mode as u8
    }
}

impl Compression {
    pub fn from(raw: u8) -> Result<Compression> {
        match raw {
            0 => Ok(Compression::None),
            1 => Ok(Compression::None),
            4 => Ok(Compression::LZMA2),
            _ => Err(Error::UnknownCompression),
        }
    }
}

/// A cluster of blobs
///
/// Within an ZIM archive, clusters contain several blobs of data that are all compressed together.
/// Each blob is the data for an article.
#[allow(dead_code)]
pub struct Cluster<'a> {
    extended: bool,
    compression: Compression,
    start: u64,
    end: u64,
    size: u64,
    view: &'a [u8],
    blob_list: Option<Vec<u64>>, // offsets into data
    decompressed: Option<Vec<u8>>,
}

impl<'a> fmt::Debug for Cluster<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cluster")
            .field("extended", &self.extended)
            .field("compression", &self.compression)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("size", &self.size)
            .field("view len", &self.view.len())
            .field("blob_list", &self.blob_list)
            .field(
                "decompressed len",
                &self.decompressed.as_ref().map(|s| s.len()),
            )
            .finish()
    }
}

impl<'a> Cluster<'a> {
    pub fn new(
        master_view: &'a Mmap,
        cluster_list: &'a Vec<u64>,
        idx: u32,
        checksum_pos: u64,
        version: u16,
    ) -> Result<Cluster<'a>> {
        let idx = idx as usize;
        let start = cluster_list[idx];
        let end = if idx < cluster_list.len() - 1 {
            cluster_list[idx + 1]
        } else {
            checksum_pos
        };

        assert!(end > start);
        let cluster_size = end - start;
        let cluster_view = master_view
            .get(start as usize..end as usize)
            .ok_or(Error::OutOfBounds)?;

        let (extended, compression) =
            parse_details(cluster_view.get(0).ok_or(Error::OutOfBounds)?)?;

        // extended clusters are only allowed in version 6
        if extended && version != 6 {
            return Err(Error::InvalidClusterExtension);
        }

        Ok(Cluster {
            extended: extended,
            compression: compression,
            start: start,
            end: end,
            size: cluster_size,
            view: cluster_view,
            decompressed: None,
            blob_list: None,
        })
    }

    pub fn decompress(&mut self) -> Result<()> {
        match self.compression {
            Compression::LZMA2 => {
                if self.decompressed.is_none() {
                    let mut decoder = XzDecoder::new(&self.view[1..]);
                    let mut d = Vec::with_capacity(self.view.len());
                    decoder.read_to_end(&mut d)?;
                    self.decompressed = Some(d);
                }
            }
            Compression::None => {}
        }

        match self.blob_list {
            Some(_) => {}
            None => {
                let blob_list = match self.compression {
                    Compression::LZMA2 => {
                        let cur = Cursor::new(self.decompressed.as_ref().unwrap());
                        parse_blob_list(cur, self.extended)?
                    }
                    Compression::None => {
                        let cur = Cursor::new(&self.view[1..]);
                        parse_blob_list(cur, self.extended)?
                    }
                };
                self.blob_list = Some(blob_list);
            }
        }

        Ok(())
    }

    pub fn get_blob(&mut self, idx: u32) -> Result<&[u8]> {
        self.decompress()?;

        match self.blob_list {
            Some(ref list) => {
                let start = list[idx as usize] as usize;
                let n = idx as usize + 1;
                let end = if list.len() > n {
                    list[n] as usize
                } else {
                    self.size as usize
                };

                Ok(match self.compression {
                    Compression::LZMA2 => {
                        // decompressed, so we know this exists
                        &self.decompressed.as_ref().unwrap().as_slice()[start..end]
                    }
                    Compression::None => &self.view[1 + start..1 + end],
                })
            }
            None => Err(Error::MissingBlobList),
        }
    }
}

/// Parses the cluster information.
///
/// Fourth low bits:
///   - 0: default (no compression),
///   - 1: none (inherited from Zeno),
///   - 4: LZMA2 compressed
/// Firth bits :
///   - 0: normal (OFFSET_SIZE=4)
///   - 1: extended (OFFSET_SIZE=8)
fn parse_details(details: &u8) -> Result<(bool, Compression)> {
    let slice = &[*details];
    let mut reader = BitReader::new(slice);
    // skip first three bits
    reader.skip(3)?;

    // extended mode is the 4th bits from the left
    // compression are the last four bits

    Ok((reader.read_bool()?, Compression::from(reader.read_u8(4)?)?))
}

fn parse_blob_list<T: ReadBytesExt>(mut cur: T, extended: bool) -> Result<Vec<u64>> {
    let mut blob_list = Vec::new();

    // determine the count of blobs, by reading the first offset
    let first = if extended {
        cur.read_u64::<LittleEndian>()?
    } else {
        cur.read_u32::<LittleEndian>()? as u64
    };

    let count = if extended { first / 8 } else { first / 4 };

    blob_list.push(first);

    for _ in 0..(count as usize - 1) {
        if extended {
            blob_list.push(cur.read_u64::<LittleEndian>()?);
        } else {
            blob_list.push(cur.read_u32::<LittleEndian>()? as u64);
        }
    }

    Ok(blob_list)
}
