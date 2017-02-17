extern crate bsdiff;

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Read};
use std::fmt;

use bsdiff::index::{Cache, StoredSuffixArray};
use bsdiff::diff::DiffStat;

fn load<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let mut contents = Vec::new();
    File::open(path)?.read_to_end(&mut contents)?;
    Ok(contents)
}

struct FileCache {
	path: PathBuf
}

impl FileCache {
	fn new(path: PathBuf) -> FileCache {
		fs::create_dir_all(&path);
		FileCache {
			path: path
		}
	}
}

struct Hex<'a>(pub &'a [u8]);

impl<'a> fmt::Display for Hex<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0.iter() {
            try!(write!(f, "{:02x}", byte));
        }
        Ok(())
    }
}

impl Cache for FileCache {
    type Read = File;
    type Write = File;

    fn get(&self, digest: &[u8; 20]) -> io::Result<Option<Self::Read>> {
        match File::open(self.path.join(format!("{}", Hex(digest)))) {
            Ok(read) =>
                Ok(Some(read)),
            Err(ref e) if e.kind() == io::ErrorKind::NotFound =>
                Ok(None),
            Err(e) =>
                Err(e)
        }
    }

    fn get_writer(&self, digest: &[u8; 20]) -> io::Result<Self::Write> {
        File::create(self.path.join(format!("{}", Hex(digest))))
    }
}

fn main() {
    let a = load("tests/avian_linux").unwrap();
    let b = load("tests/avian_pr_linux").unwrap();

    let index = StoredSuffixArray::from_cache_or_compute(
        FileCache::new(PathBuf::from(".cache")),
        a).unwrap();

    let stat = DiffStat::from(index, &b);

    println!("{:?}", stat);
}