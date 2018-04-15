use std;
use directory_entry::DirectoryEntry;

use zim::Zim;

pub struct DirectoryIterator<'a> {
    max_articles: u32,
    article_to_yield: u32,
    zim: &'a Zim,
}

impl<'a> DirectoryIterator<'a> {
    pub fn new(zim: &'a Zim) -> DirectoryIterator<'a> {
        DirectoryIterator {
            max_articles: zim.header.article_count,
            article_to_yield: 0,
            zim: zim,
        }
    }
}

impl<'a> std::iter::Iterator for DirectoryIterator<'a> {
    type Item = DirectoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.article_to_yield < self.max_articles {
            let dir_entry_ptr = self.zim.url_list[self.article_to_yield as usize] as usize;
            self.article_to_yield += 1;

            let len = self.zim.master_view.len();
            let slice = self.zim.master_view.get(
                dir_entry_ptr..(len - dir_entry_ptr),
            );
            if slice.is_none() {
                return None;
            }

            DirectoryEntry::new(self.zim, slice.unwrap()).ok()
        } else {
            None
        }
    }
}
