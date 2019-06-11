use crate::dos_error_codes::DosErrorCode;

use std::io::Read;
use std::io::Seek;
use std::collections::{HashMap, VecDeque};

pub trait DosFileSystem : std::fmt::Debug {
	/// Returns a file handle if successful. Error code if not.
	fn create(&mut self, filename: &[u8], attributes: u16) -> Result<u16, DosErrorCode>;
	/// Returns a file handle if successful. Error code if not.
	fn open(&mut self, filename: &[u8], access_mode: DosFileAccessMode) -> Result<u16, DosErrorCode>;
	/// Retruns error code if close failed.
	fn close(&mut self, handle: u16) -> Result<(), DosErrorCode>;
	/// Returns the byte count read. Error code if read failed.
	fn read(&mut self, handle: u16, destination: &mut [u8]) -> Result<u16, DosErrorCode>;
	/// Returns the byte count written. Error code if write failed.
	fn write(&mut self, handle: u16, data: &[u8]) -> Result<u16, DosErrorCode>;
	/// Returns the new position within the file relative to the start. Error code if seek failed.
	fn seek(&mut self, handle: u16, offset: u32, origin: DosFileSeekOrigin) -> Result<u32, DosErrorCode>;
	fn find_first_file(&mut self, destination: &mut [u8], attributes: u16, search_spec: &[u8]) -> Result<(), DosErrorCode>;
	fn find_next_file(&mut self, destination: &mut [u8]) -> Result<(), DosErrorCode>;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DosFileAccessMode {
	ReadOnly,
	WriteOnly,
	ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DosFileSeekOrigin {
	Start,
	Current,
	End,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DosFileName {
	title: Vec<u8>,
	ext: Vec<u8>,
}

impl DosFileName {
	fn parse(dos_filename: &[u8]) -> DosFileName {
		let (title, ext) = split_filename(dos_filename);
		DosFileName{title: title.to_ascii_uppercase(), ext: ext.unwrap_or(&[]).to_ascii_uppercase()}
	}

	fn real_dos_name(&self) -> Vec<u8> {
		let mut result = self.title.clone();
		if !self.ext.is_empty() {
			result.push(b'.');
			result.extend(&self.ext);
		}
		result
	}
}

#[derive(Debug)]
struct DirListingCache {
	dir_path: std::path::PathBuf,
	real_to_dos_names: HashMap<String, DosFileName>,
	dos_to_real_names: HashMap<DosFileName, String>,
}

impl DirListingCache {
	fn new(dir_path: std::path::PathBuf) -> DirListingCache {
		let mut dir_listing = DirListingCache {
			dir_path,
			real_to_dos_names: HashMap::new(),
			dos_to_real_names: HashMap::new(),
		};
		dir_listing.list_dir(&mut |_|{});
		dir_listing
	}
	
	fn get_dos_name(&mut self, real_filename: &str) -> DosFileName {
		if let Some(existing_dos_name) = self.real_to_dos_names.get(real_filename) {
			existing_dos_name.clone()
		} else {
			let mut dos_name = real_to_dos_name(&real_filename, None);
			let mut name_index = 1;
			while self.dos_to_real_names.contains_key(&dos_name) {
				dos_name = real_to_dos_name(&real_filename, Some(name_index));
				name_index += 1;
			}
			self.dos_to_real_names.insert(dos_name.clone(), real_filename.to_string());
			self.real_to_dos_names.insert(real_filename.to_string(), dos_name.clone());
			dos_name
		}
	}
	
	fn get_real_name(&mut self, dos_filename: &DosFileName) -> String {
		self.list_dir(&mut |_|{});
		if let Some(existing_real_name) = self.dos_to_real_names.get(&dos_filename) {
			existing_real_name.clone()
		} else {
			let mut real_name = ascii_filename_to_string(&dos_filename.real_dos_name());
			self.dos_to_real_names.insert(dos_filename.clone(), real_name.clone());
			self.real_to_dos_names.insert(real_name.clone(), dos_filename.clone());
			real_name
		}
	}

	fn list_dir(&mut self, on_found_file: &mut FnMut(DosFileName)) {
		if let Ok(read_dir) = std::fs::read_dir(&self.dir_path) {
			for dir_file in read_dir {
				if let Ok(dir_file_entry) = dir_file {
					if let Ok(dir_file_entry_name) = dir_file_entry.file_name().into_string() {
						on_found_file(self.get_dos_name(&dir_file_entry_name));
					}
				}
			}
		}
	}
	
	//fn get_real_name(filename: &[u8])
}

fn ascii_filename_to_string(ascii: &[u8]) -> String {
	ascii.iter().map(|c| c.to_ascii_uppercase() as char).collect()
}

fn real_to_dos_name(filename: &str, extra_index: Option<usize>) -> DosFileName {
	let mut ascii_name = vec![];
	for c in filename.chars() {
		if c <= 255 as char {
			ascii_name.push((c as u8).to_ascii_uppercase());
		} else {
			ascii_name.push(b'_');
		}
	}
	let (file_title, file_ext) = split_filename(&ascii_name);
	let mut short_title = file_title.to_vec();
	short_title.truncate(8);
	let mut short_ext = file_ext.unwrap_or(&[]).to_vec();
	short_ext.truncate(3);
	
	let mut title_index_text = vec![];
	if let Some(extra_index) = extra_index {
		title_index_text.push(b'~');
		extra_index.to_string().chars().for_each(|c| title_index_text.push(c as u8));
	}
	let current_title_len = short_title.len() + title_index_text.len();
	if current_title_len > 8 {
		short_title = short_title[..short_title.len() - (current_title_len - 8)].to_vec();
	}
	short_title.extend(&title_index_text);
	
	DosFileName {
		title: short_title,
		ext: short_ext,
	}
}

fn split_filename(filename: &[u8]) -> (&[u8], Option<&[u8]>) {
	if let Some(dot_pos) = filename.iter().rposition(|c| *c == b'.') {
		let after_dot = &filename[dot_pos + 1..];
		if after_dot.len() <= 3 {
			(&filename[..dot_pos], Some(after_dot))
		} else {
			(&filename[..dot_pos], Some(&after_dot[..3]))
		}
	} else {
		(filename, None)
	}
}

// https://ss64.com/nt/syntax-wildcards.html
fn filename_matches_spec(filename: &DosFileName, search_spec: &[u8]) -> bool {
	let match_against_spec = |text: &[u8], spec: &[u8]| {
		//dbg!((ascii_filename_to_string(text), ascii_filename_to_string(spec)));
		let mut spec_pos = 0;
		let mut just_processed_star = false;
		for c in text {
			if let Some(&spec_char) = spec.get(spec_pos) {
				if spec_char == b'*' {
					if let Some(next_spec_char) = spec.get(spec_pos + 1) {
						if *c == *next_spec_char {
							spec_pos += 1;
						}
					}
					just_processed_star = true;
				} else if spec_char == b'?' {
					spec_pos += 1;
				} else if *c == spec_char {
					spec_pos += 1;
				} else {
					return false;
				}
			} else {
				return false;
			}
		}
		if just_processed_star {
			spec_pos += 1;
		}
		spec_pos == spec.len()
	};
	
	let (spec_title, spec_ext) = split_filename(search_spec);
	let title_matches = match_against_spec(&filename.title, spec_title);
	let ext_matches = if let Some(spec_ext) = spec_ext {
		match_against_spec(&filename.ext, spec_ext)
	} else {
		true
	};
	title_matches && ext_matches
}

#[derive(Debug)]
pub struct StandardDosFileSystem {
	root_path: std::path::PathBuf,
	file_handles: Vec<Option<std::fs::File>>,
	dir_listing: DirListingCache,
	current_file_queue: Option<VecDeque<DosFileName>>,
}

impl StandardDosFileSystem {
	pub fn new(root_path: std::path::PathBuf) -> StandardDosFileSystem {
		StandardDosFileSystem {
			root_path: root_path.clone(),
			file_handles: vec![],
			current_file_queue: None,
			dir_listing: DirListingCache::new(root_path.clone()),
		}
	}
	
	fn get_empty_slot(&mut self) -> usize {
		match self.file_handles.iter().position(|ref slot| slot.is_none()) {
			Some(pos) => pos,
			None => {
				let pos = self.file_handles.len();
				self.file_handles.push(None);
				pos
			}
		}
	}
	
	/*fn get_real_filepath(&self, filename: &[u8]) -> std::path::PathBuf {
		if filename.contains(&b'\\') {
			unimplemented!("DOS directory mapping to real directories");
		}
		let mut string_filename = String::from_utf8_lossy(filename).into_owned();
		
		if let Ok(read_dir) = std::fs::read_dir(&self.root_path) {
			for dir_file in read_dir {
				if let Ok(dir_file_entry) = dir_file {
					if let Ok(dir_file_entry_name) = dir_file_entry.file_name().into_string() {
						if dir_file_entry_name.to_uppercase() == string_filename.to_uppercase() {
							string_filename = dir_file_entry_name;
						}
					}
				}
			}
		}
		self.root_path.join(string_filename)
	}*/
	
	fn get_real_filepath(&mut self, filename: &[u8]) -> std::path::PathBuf {
		let real_name = self.dir_listing.get_real_name(&DosFileName::parse(filename));
		self.root_path.join(real_name)
	}
}

fn std_file_error_to_dos_error(err: std::io::Error) -> DosErrorCode {
	match err.kind() {
		std::io::ErrorKind::NotFound => DosErrorCode::FileNotFound,
		std::io::ErrorKind::PermissionDenied => DosErrorCode::AccessDenied,
		std::io::ErrorKind::AlreadyExists => DosErrorCode::FileAlreadyExists,
		_ => {
			eprintln!("Unexpected file error: {:?}", err);
			DosErrorCode::PathNotFound
		}
	}
}

impl DosFileSystem for StandardDosFileSystem {
	fn create(&mut self, filename: &[u8], attributes: u16) -> Result<u16, DosErrorCode> {
		let real_filepath = self.get_real_filepath(filename);
		let slot = self.get_empty_slot();
		match std::fs::File::create(real_filepath) {
			Ok(file) => {
				self.file_handles[slot] = Some(file);
				Ok(slot as u16 + 1)
			}
			Err(err) => Err(std_file_error_to_dos_error(err)),
		}
	}
	
	fn open(&mut self, filename: &[u8], access_mode: DosFileAccessMode) -> Result<u16, DosErrorCode> {
		// TODO: 776655
		let real_filepath = self.get_real_filepath(filename);
		let slot = self.get_empty_slot();
		
		let mut open_options = std::fs::OpenOptions::new();

		open_options
			.read(access_mode == DosFileAccessMode::ReadOnly || access_mode == DosFileAccessMode::ReadWrite)
			.write(access_mode == DosFileAccessMode::WriteOnly || access_mode == DosFileAccessMode::ReadWrite)
			.create(access_mode == DosFileAccessMode::WriteOnly || access_mode == DosFileAccessMode::ReadWrite);
		
		match open_options.open(real_filepath) {
			Ok(file) => {
				self.file_handles[slot] = Some(file);
				Ok(slot as u16 + 1)
			}
			Err(err) => Err(std_file_error_to_dos_error(err)),
		}
	}
	
	fn close(&mut self, handle: u16) -> Result<(), DosErrorCode> {
		if handle == 0 {
			Err(DosErrorCode::InvalidFileHandle)
		} else {
			let handle_index = (handle - 1) as usize;
			if let Some(Some(ref mut file)) = self.file_handles.get_mut(handle_index) {
				self.file_handles[handle_index] = None;
				Ok(())
			} else {
				Err(DosErrorCode::InvalidFileHandle)
			}
		}
	}
	
	fn read(&mut self, handle: u16, destination: &mut [u8]) -> Result<u16, DosErrorCode> {
		if handle == 0 {
			Err(DosErrorCode::InvalidFileHandle)
		} else {
			let handle_index = (handle - 1) as usize;
			if let Some(Some(ref mut file)) = self.file_handles.get_mut(handle_index) {
				match file.read(destination) {
					Ok(read_count) => Ok(read_count as u16),
					Err(err) => Err(std_file_error_to_dos_error(err)),
				}
			} else {
				Err(DosErrorCode::InvalidFileHandle)
			}
		}
	}
	
	fn write(&mut self, handle: u16, data: &[u8]) -> Result<u16, DosErrorCode> {
		unimplemented!()
	}
	
	fn seek(&mut self, handle: u16, offset: u32, origin: DosFileSeekOrigin) -> Result<u32, DosErrorCode> {
		if handle == 0 {
			Err(DosErrorCode::InvalidFileHandle)
		} else {
			let handle_index = (handle - 1) as usize;
			if let Some(Some(ref mut file)) = self.file_handles.get_mut(handle_index) {
				let seek_from = match origin {
					DosFileSeekOrigin::Start => std::io::SeekFrom::Start(offset as u64),
					DosFileSeekOrigin::Current => std::io::SeekFrom::Current(offset as i64),
					DosFileSeekOrigin::End => std::io::SeekFrom::End(offset as i64),
				};
				match file.seek(seek_from) {
					Ok(file_pos) => Ok(file_pos as u32),
					Err(err) => Err(std_file_error_to_dos_error(err)),
				}
			} else {
				Err(DosErrorCode::InvalidFileHandle)
			}
		}
	}
	
	fn find_first_file(&mut self, destination: &mut [u8], attributes: u16, search_spec: &[u8]) -> Result<(), DosErrorCode> {
		let real_filepath = self.get_real_filepath(search_spec);
		let mut file_queue = VecDeque::new();
		self.dir_listing.list_dir(&mut |dos_name| {
			//dbg!(ascii_filename_to_string(&dos_name.real_dos_name()));
			if filename_matches_spec(&dos_name, search_spec) {
				file_queue.push_back(dos_name);
			}
		});
		self.current_file_queue = Some(file_queue);
		
		self.find_next_file(destination)
	}
	
	fn find_next_file(&mut self, destination: &mut [u8]) -> Result<(), DosErrorCode> {
		if let Some(ref mut current_file_queue) = self.current_file_queue {
			if let Some(ref next_file) = current_file_queue.pop_front() {
				let next_name = next_file.real_dos_name();
				dbg!(ascii_filename_to_string(&next_name));
				// http://stanislavs.org/helppc/int_21-4e.html
				let filename_off = 0x1e;
				destination[0x15..=filename_off].iter_mut().for_each(|b| *b = 0);
				let filename_dest = &mut destination[filename_off..];
				filename_dest[..next_name.len()].clone_from_slice(&next_name);
				filename_dest[next_name.len()] = 0;
				Ok(())
			} else {
				Err(DosErrorCode::NoMoreFiles)
			}
		} else {
			Err(DosErrorCode::NoMoreFiles)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test] fn test_dir_listing_cache() {
		let mut dir_listing = DirListingCache::new();
		assert_eq!(String::from_utf8_lossy(&dir_listing.get_dos_name("foot.text").real_dos_name()), String::from_utf8_lossy(b"FOOT.TEX"));
		assert_eq!(String::from_utf8_lossy(&dir_listing.get_dos_name("foot.text2").real_dos_name()), String::from_utf8_lossy(b"FOOT~1.TEX"));
		assert_eq!(String::from_utf8_lossy(&dir_listing.get_dos_name("filewithlongname.txt").real_dos_name()), String::from_utf8_lossy(b"FILEWITH.TXT"));
		assert_eq!(String::from_utf8_lossy(&dir_listing.get_dos_name("filewithlongername.txt").real_dos_name()), String::from_utf8_lossy(b"FILEWI~1.TXT"));
		assert_eq!(String::from_utf8_lossy(&dir_listing.get_dos_name("filewithlongerername.txt").real_dos_name()), String::from_utf8_lossy(b"FILEWI~2.TXT"));
	}
}
