use std;
use std::io::Cursor;
use std::io::BufRead;

use byteorder::{LittleEndian, ReadBytesExt};

use target::Target;
use mime_type::MimeType;
use errors::ParsingError;
use zim::Zim;

/// Holds metadata about an article
#[derive(Debug)]
pub struct DirectoryEntry {
    /// MIME type number as defined in the MIME type list
    pub mime_type: MimeType,
    /// defines to which namespace this directory entry belongs
    pub namespace: char,
    /// identifies a revision of the contents of this directory entry, needed to identify
    /// updates or revisions in the original history
    pub revision: Option<u32>,
    /// the URL as refered in the URL pointer list
    pub url: String,
    /// title as refered in the Title pointer list or empty; in case it is empty,
    /// the URL is used as title
    pub title: String,
    pub target: Option<Target>,
}

impl DirectoryEntry {
    pub fn new(zim: &Zim, s: &[u8]) -> Result<DirectoryEntry, ParsingError> {
        let mut cur = Cursor::new(s);
        let mime_id = cur.read_u16::<LittleEndian>()?;
        let mime_type = zim.get_mimetype(mime_id)
            .ok_or(ParsingError {
                       msg: "No such Mimetype",
                       cause: None,
                   })?;
        let _ = cur.read_u8()?;
        let namespace = cur.read_u8()?;
        let rev = cur.read_u32::<LittleEndian>().ok();
        let mut target = None;

        if mime_type == MimeType::Redirect {
            // this is an index into the URL table
            target = Some(Target::Redirect(cur.read_u32::<LittleEndian>()?));
        } else if mime_type == MimeType::LinkTarget || mime_type == MimeType::DeletedEntry {

        } else {
            let cluster_number = cur.read_u32::<LittleEndian>()?;
            let blob_number = cur.read_u32::<LittleEndian>()?;
            target = Some(Target::Cluster(cluster_number, blob_number));
        }

        let url = {
            let mut vec = Vec::new();
            let size = cur.read_until(0, &mut vec)?;
            vec.truncate(size - 1);
            String::from_utf8(vec)?
        };
        let title = {
            let mut vec = Vec::new();
            let size = cur.read_until(0, &mut vec)?;
            vec.truncate(size - 1);
            String::from_utf8(vec)?
        };


        Ok(DirectoryEntry {
               mime_type: mime_type,
               namespace: std::char::from_u32(namespace as u32).unwrap(),
               revision: rev,
               url: url,
               title: title,
               target: target,
           })
    }
}
