use byteorder::{LittleEndian, ReadBytesExt};
use memmap::Mmap;
use std::io::Cursor;

use std::fs::File;
use std::io::BufRead;
use std::path::Path;

use crate::cluster::Cluster;
use crate::directory_entry::DirectoryEntry;
use crate::directory_iterator::DirectoryIterator;
use crate::errors::{Error, Result};
use crate::mime_type::MimeType;

/// Magic number to recognise the file format, must be 72173914
pub const ZIM_MAGIC_NUMBER: u32 = 72173914;

/// Represents a ZIM file
#[allow(dead_code)]
pub struct Zim {
    // Zim structure data:
    pub header: ZimHeader,

    pub master_view: Mmap,

    /// List of mimetypes used in this ZIM archive
    pub mime_table: Vec<String>, // a list of mimetypes
    pub url_list: Vec<u64>,     // a list of offsets
    pub article_list: Vec<u32>, // a list of indicies into url_list
    pub cluster_list: Vec<u64>, // a list of offsets
}

/// A ZIM file starts with a header.
pub struct ZimHeader {
    /// Major version, either 5 or 6
    pub version_major: u16,
    /// Minor version
    pub version_minor: u16,
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
    pub fn new<P: AsRef<Path>>(p: P) -> Result<Zim> {
        let f = File::open(p)?;
        let master_view = unsafe { Mmap::map(&f)? };

        let (header, mime_table) = parse_header(&master_view)?;

        let url_list = parse_url_list(&master_view, header.url_ptr_pos, header.article_count)?;
        let article_list =
            parse_article_list(&master_view, header.title_ptr_pos, header.article_count)?;

        let cluster_list =
            parse_cluster_list(&master_view, header.cluster_ptr_pos, header.cluster_count)?;

        Ok(Zim {
            header: header,
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
                    println!("WARNING unknown mimetype idx {}", id);
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
    pub fn get_by_url_index(&self, idx: u32) -> Result<DirectoryEntry> {
        let entry_offset = self.url_list[idx as usize] as usize;
        let (_, dir_view) = self.master_view.split_at(entry_offset);

        DirectoryEntry::new(self, dir_view)
    }

    /// Returns the given `Cluster`
    ///
    /// idx must be between 0 and `cluster_count`
    pub fn get_cluster(&self, idx: u32) -> Result<Cluster> {
        Cluster::new(
            &self.master_view,
            &self.cluster_list,
            idx,
            self.header.checksum_pos,
            self.header.version_major,
        )
    }
}

fn is_defined(val: u32) -> Option<u32> {
    if val == 0xffffffff {
        None
    } else {
        Some(val)
    }
}

fn parse_header(master_view: &Mmap) -> Result<(ZimHeader, Vec<String>)> {
    let mut header_cur = Cursor::new(master_view);

    let magic = header_cur.read_u32::<LittleEndian>()?;

    if magic != ZIM_MAGIC_NUMBER {
        return Err(Error::InvalidMagicNumber);
    }

    let version_major = header_cur.read_u16::<LittleEndian>()?;
    if version_major != 5 && version_major != 6 {
        return Err(Error::InvalidVersion);
    }

    let version_minor = header_cur.read_u16::<LittleEndian>()?;

    let uuid = [
        header_cur.read_u64::<LittleEndian>()?,
        header_cur.read_u64::<LittleEndian>()?,
    ];
    let article_count = header_cur.read_u32::<LittleEndian>()?;
    let cluster_count = header_cur.read_u32::<LittleEndian>()?;
    let url_ptr_pos = header_cur.read_u64::<LittleEndian>()?;
    let title_ptr_pos = header_cur.read_u64::<LittleEndian>()?;
    let cluster_ptr_pos = header_cur.read_u64::<LittleEndian>()?;
    let mime_list_pos = header_cur.read_u64::<LittleEndian>()?;

    let main_page = header_cur.read_u32::<LittleEndian>()?;
    let layout_page = header_cur.read_u32::<LittleEndian>()?;
    let checksum_pos = header_cur.read_u64::<LittleEndian>()?;

    if header_cur.position() != 80 {
        return Err(Error::InvalidHeader);
    }

    let geo_index_pos = if mime_list_pos > 80 {
        Some(header_cur.read_u64::<LittleEndian>()?)
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
                mime_table.push(String::from_utf8(mime_buf)?);
            }
        }
        mime_table
    };

    Ok((
        ZimHeader {
            version_major: version_major,
            version_minor: version_minor,
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
        mime_table,
    ))
}

/// Parses the URL Pointer List.
/// See https://wiki.openzim.org/wiki/ZIM_file_format#URL_Pointer_List_.28urlPtrPos.29
fn parse_url_list(master_view: &Mmap, ptr_pos: u64, count: u32) -> Result<Vec<u64>> {
    let start = ptr_pos as usize;
    let end = (ptr_pos + count as u64 * 8) as usize;
    let list_view = master_view.get(start..end).ok_or(Error::OutOfBounds)?;
    let mut cur = Cursor::new(list_view);

    let mut out: Vec<u64> = Vec::new();
    for _ in 0..count {
        out.push(cur.read_u64::<LittleEndian>()?);
    }

    Ok(out)
}

fn parse_article_list(master_view: &Mmap, ptr_pos: u64, count: u32) -> Result<Vec<u32>> {
    let start = ptr_pos as usize;
    let end = (ptr_pos as u32 + count * 4) as usize;
    let list_view = master_view.get(start..end).ok_or(Error::OutOfBounds)?;

    let mut cur = Cursor::new(list_view);
    let mut out: Vec<u32> = Vec::new();

    for _ in 0..count {
        out.push(cur.read_u32::<LittleEndian>()?);
    }

    Ok(out)
}

fn parse_cluster_list(master_view: &Mmap, ptr_pos: u64, count: u32) -> Result<Vec<u64>> {
    let start = ptr_pos as usize;
    let end = (ptr_pos as u32 + count * 8) as usize;
    let cluster_list_view = master_view.get(start..end).ok_or(Error::OutOfBounds)?;

    let mut cluster_cur = Cursor::new(cluster_list_view);
    let mut out: Vec<u64> = Vec::new();
    for _ in 0..count {
        out.push(cluster_cur.read_u64::<LittleEndian>()?);
    }
    Ok(out)
}

#[test]
fn test_zim() {
    let zim = Zim::new("fixtures/wikipedia_ab_all_2017-03.zim").expect("failed to parse fixture");

    assert_eq!(zim.header.version_major, 5);
    let mut cl0 = zim.get_cluster(0).unwrap();
    assert_eq!(cl0.get_blob(0).unwrap(), &[97, 98, 107]);

    let mut cl1 = zim.get_cluster(zim.header.cluster_count - 1).unwrap();
    let b = cl1.get_blob(0).unwrap();
    assert_eq!(&b[0..10], &[71, 73, 70, 56, 57, 97, 44, 1, 150, 0]);
    assert_eq!(
        &b[b.len() - 10..],
        &[222, 192, 21, 240, 155, 91, 65, 0, 0, 59]
    );

    assert_eq!(zim.iterate_by_urls().count(), 3111);
}
