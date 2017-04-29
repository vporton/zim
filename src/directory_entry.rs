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
    pub mime_type: MimeType,
    pub namespace: char,
    pub revision: u32,
    pub url: String,
    pub title: String,
    pub target: Option<Target>
}

impl DirectoryEntry {
    pub fn new(zim: &Zim, s: &[u8]) -> Result<DirectoryEntry, ParsingError> {
        let mut cur = Cursor::new(s);
        let mime_id = try!(cur.read_u16::<LittleEndian>());
        let mime_type = try!(zim.get_mimetype(mime_id).ok_or(ParsingError{msg: "No such Mimetype", cause: None}));
        let _ = try!(cur.read_u8());
        let namespace = try!(cur.read_u8());
        let rev = try!(cur.read_u32::<LittleEndian>());
        let mut target = None;


        if mime_type == MimeType::Redirect {
            // this is an index into the URL table
            target = Some(Target::Redirect(try!(cur.read_u32::<LittleEndian>())));
        } else if mime_type == MimeType::LinkTarget || mime_type == MimeType::DeletedEntry {

        } else {
            let cluster_number = try!(cur.read_u32::<LittleEndian>());
            let blob_number = try!(cur.read_u32::<LittleEndian>());
            target = Some(Target::Cluster(cluster_number, blob_number));
        }
       
        let url = {
            let mut vec = Vec::new();
            let size = try!(cur.read_until(0, &mut vec));
            vec.truncate(size - 1);
            try!(String::from_utf8(vec))
        };
        let title = {
            let mut vec = Vec::new();
            let size = try!(cur.read_until(0, &mut vec));
            vec.truncate(size - 1);
            try!(String::from_utf8(vec))
        };


        Ok(DirectoryEntry{
            mime_type: mime_type,
            namespace: std::char::from_u32(namespace as u32).unwrap(),
            revision: rev,
            url: url,
            title: title,
            target: target,
        })
    }
}
