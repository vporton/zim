extern crate memmap;

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
use memmap::{Mmap, MmapViewSync};

use std::fs::File;
use std::io::BufRead;
use std::path::Path;

use cluster::Cluster;
use directory_entry::DirectoryEntry;
use directory_iterator::DirectoryIterator;
use mime_type::MimeType;
use errors::ParsingError;

/// Magic number to recognise the file format, must be 72173914
pub const ZIM_MAGIC_NUMBER: u32 = 72173914;

/// Represents a ZIM file
#[allow(dead_code)]
pub struct Zim {
    // Zim structure data:
    pub header: ZimHeader,

    pub master_view: MmapViewSync,

    /// List of mimetypes used in this ZIM archive
    pub mime_table: Vec<String>, // a list of mimetypes
    pub url_list: Vec<u64>, // a list of offsets
    pub article_list: Vec<u32>, // a list of indicies into url_list
    pub cluster_list: Vec<u64>, // a list of offsets
}

/// A ZIM file starts with a header.
pub struct ZimHeader {
    /// ZIM=5, bytes 1-2: major, bytes 3-4: minor version of the ZIM file format
    pub version: u32,
    /// unique id of this zim file
    pub uuid: [u64; 2],
    /// total number of articles
    pub article_count: u32,
    /// total number of clusters
    pub cluster_count: u32,
    /// position of the directory pointerlist ordered by URL
    pub url_ptr_pos: u64,
    /// position of the directory pointerlist ordered by Title
    pub title_ptr_pos: u64,
    /// position of the cluster pointer list
    pub cluster_ptr_pos: u64,
    /// position of the MIME type list (also header size)
    pub mime_list_pos: u64,
    /// main page or 0xffffffff if no main page
    pub main_page: Option<u32>,
    /// ayout page or 0xffffffffff if no layout page
    pub layout_page: Option<u32>,
    /// pointer to the md5checksum of this file without the checksum itself.
    /// This points always 16 bytes before the end of the file.
    pub checksum_pos: u64,
    /// pointer to the geo index (optional). Present if mimeListPos is at least 80.
    pub geo_index_pos: Option<u64>,
}


impl Zim {
    /// Loads a Zim file
    ///
    /// Loads a Zim file and parses the header, and the url, title, and cluster offset tables.  The
    /// rest of the data isn't parsed until it's needed, so this should be fairly quick.
    pub fn new<P: AsRef<Path>>(p: P) -> Result<Zim, ParsingError> {
        let f = try!(File::open(p));
        let mmap = try!(Mmap::open(&f, memmap::Protection::Read));
        let master_view = mmap.into_view_sync();

        let header_view = {
            let view = unsafe { master_view.clone() };
            view
        };

        let mut header_cur = Cursor::new(unsafe { header_view.as_slice() });
        let magic = try!(header_cur.read_u32::<LittleEndian>());
        assert_eq!(magic, ZIM_MAGIC_NUMBER);
        let version = try!(header_cur.read_u32::<LittleEndian>());
        let uuid = [try!(header_cur.read_u64::<LittleEndian>()),
                    try!(header_cur.read_u64::<LittleEndian>())];
        let article_count = try!(header_cur.read_u32::<LittleEndian>());
        let cluster_count = try!(header_cur.read_u32::<LittleEndian>());
        let url_ptr_pos = try!(header_cur.read_u64::<LittleEndian>());
        let title_ptr_pos = try!(header_cur.read_u64::<LittleEndian>());
        let cluster_ptr_pos = try!(header_cur.read_u64::<LittleEndian>());
        let mime_list_pos = try!(header_cur.read_u64::<LittleEndian>());

        let main_page = try!(header_cur.read_u32::<LittleEndian>());
        let layout_page = try!(header_cur.read_u32::<LittleEndian>());
        let checksum_pos = try!(header_cur.read_u64::<LittleEndian>());

        assert_eq!(header_cur.position(), 80);

        let geo_index_pos = if mime_list_pos > 80 {
            Some(try!(header_cur.read_u64::<LittleEndian>()))
        } else {
            None
        };

        // the mime table is always directly after the 80-byte header, so we'll keep
        // using our header cursor
        let mime_table = {
            let mut mime_table = Vec::new();
            loop {
                let mut mime_buf = Vec::new();
                if let Ok(size) = header_cur.read_until(0, &mut mime_buf) {
                    if size <= 1 {
                        break;
                    }
                    mime_buf.truncate(size - 1);
                    mime_table.push(try!(String::from_utf8(mime_buf)));
                }
            }
            mime_table
        };

        let url_list = {
            let url_list_view = {
                let mut v = unsafe { master_view.clone() };
                v.restrict(url_ptr_pos as usize, article_count as usize * 8)
                    .ok();
                v
            };
            let mut url_cur = Cursor::new(unsafe { url_list_view.as_slice() });

            (0..article_count)
                .map(|_| {
                         url_cur
                             .read_u64::<LittleEndian>()
                             .ok()
                             .expect("unable to read url_list")
                     })
                .collect()
        };

        let article_list = {
            let art_list_view = {
                let mut v = unsafe { master_view.clone() };
                v.restrict(title_ptr_pos as usize, article_count as usize * 8)
                    .ok();
                v
            };
            let mut art_cur = Cursor::new(unsafe { art_list_view.as_slice() });

            (0..article_count)
                .map(|_| {
                         art_cur
                             .read_u32::<LittleEndian>()
                             .ok()
                             .expect("unable to read url_list")
                     })
                .collect()
        };


        let cluster_list = {
            let cluster_list_view = {
                let mut v = unsafe { master_view.clone() };
                try!(v.restrict(cluster_ptr_pos as usize, cluster_count as usize * 8));
                v
            };
            let mut cluster_cur = Cursor::new(unsafe { cluster_list_view.as_slice() });
            (0..cluster_count)
                .map(|_| {
                         cluster_cur
                             .read_u64::<LittleEndian>()
                             .ok()
                             .expect("unable to read url_list")
                     })
                .collect()
        };

        Ok(Zim {
               header: ZimHeader {
                   version: version,
                   uuid: uuid,
                   article_count: article_count,
                   cluster_count: cluster_count,
                   url_ptr_pos: url_ptr_pos,
                   title_ptr_pos: title_ptr_pos,
                   cluster_ptr_pos: cluster_ptr_pos,
                   mime_list_pos: mime_list_pos,
                   main_page: is_defined(main_page),
                   layout_page: is_defined(layout_page),
                   checksum_pos: checksum_pos,
                   geo_index_pos: geo_index_pos,
               },

               master_view: master_view,
               mime_table: mime_table,
               url_list: url_list,
               article_list: article_list,
               cluster_list: cluster_list,
           })

    }

    /// Indexes into the ZIM mime_table.
    pub fn get_mimetype(&self, id: u16) -> Option<MimeType> {
        match id {
            0xffff => Some(MimeType::Redirect),
            0xfffe => Some(MimeType::LinkTarget),
            0xfffd => Some(MimeType::DeletedEntry),
            id => {
                if (id as usize) < self.mime_table.len() {
                    Some(MimeType::Type(self.mime_table[id as usize].clone()))
                } else {
                    println!("WARNINING unknown mimetype idx {}", id);
                    None
                }
            }
        }
    }

    /// Iterates over articles, sorted by URL.
    ///
    /// For performance reasons, you might want to extract by cluster instead.
    pub fn iterate_by_urls(&self) -> DirectoryIterator {
        DirectoryIterator::new(self)
    }

    /// Returns the `DirectoryEntry` for the article found at the given URL index.
    ///
    /// idx must be between 0 and `article_count`
    pub fn get_by_url_index(&self, idx: u32) -> Option<DirectoryEntry> {
        let entry_offset = self.url_list[idx as usize] as usize;
        let dir_view = {
            let mut view = unsafe { self.master_view.clone() };
            let len = view.len();
            view.restrict(entry_offset, len - entry_offset).ok();
            view
        };
        let slice = unsafe { dir_view.as_slice() };
        DirectoryEntry::new(self, slice).ok()
    }

    /// Returns the given `Cluster`
    ///
    /// idx must be between 0 and `cluster_count`
    pub fn get_cluster(&self, idx: u32) -> Option<Cluster> {
        Cluster::new(self, idx).ok()
    }
}

fn is_defined(val: u32) -> Option<u32> {
    if val == 0xffffffff { None } else { Some(val) }
}
