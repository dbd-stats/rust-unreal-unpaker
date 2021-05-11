use std::path::Path;

use thiserror::Error;
use tokio::{fs::File, io::AsyncReadExt};

use super::pak_reader::{PakReader, PakReaderError};
use std::collections::HashMap;
use crate::pak::pak_reader::PakEntry;

#[derive(Error, Debug)]
pub enum PakError {
    #[error("Error opening or reading file: {0}")]
    FileError(#[from] std::io::Error),
    #[error("Error parsing the pak file: {0}")]
    ReaderError(#[from] PakReaderError),
}

#[derive(Debug)]
pub struct PakInfo {
    pub encryption_index_guid: Vec<u8>,
    pub is_encrypted: bool,
    pub magic: u32,
    pub version: i32,
    pub sub_version: i32,
    pub index_offset: i64,
    pub index_size: i64,
    pub index_hash: Vec<u8>,
    pub index_frozen: u8,
    pub compression_methods: Vec<String>
}

#[derive(Debug)]
pub struct PakCompressionBlock {
    pub compression_start: i64,
    pub compression_end: i64
}

#[derive(Debug)]
pub struct PakFile {
    pub info: PakInfo,
    pub mount_point: String,
    pub file_indexes: HashMap<String, PakEntry>,
    reader: PakReader
}

impl PakFile {
    // Open the file and pass it to the from_file
    pub async fn from_path(path: &Path) -> Result<Self, PakError> {
        match File::open(path).await {
            Ok(file) => Self::from_file(file).await,
            Err(err) => Err(PakError::FileError(err)),
        }
    }

    // Read the buffer from the file, and pass it into from_memory which reads the content
    pub async fn from_file(mut file: File) -> Result<Self, PakError> {
        let mut buffer = Vec::new();
        match file.read_to_end(&mut buffer).await {
            Ok(_) => Self::from_memory(buffer).await,
            Err(e) => Err(PakError::FileError(e)),
        }
    }

    // Parse the PAK from memory, into the pak struct, with respective reader
    pub async fn from_memory(buffer: Vec<u8>) -> Result<Self, PakError> {
        let mut reader = PakReader::new(buffer);

        let pak_info = reader.get_pak_info().await?;
        let (mount_point, indexes) = reader.get_pak_entries(&pak_info).await?;

        Ok(Self {
            info: pak_info,
            mount_point: mount_point,
            file_indexes: indexes,
            reader: reader
        })
    }

    pub async fn get_entry_data<T: Into<String>>(&mut self, index: T) -> Result<Option<Vec<u8>>, PakError> {
        let entry = self.file_indexes.get(&index.into());
        if let Some(pak_entry) = entry {
            Ok(Some(self.reader.get_pak_entry_data(pak_entry).await?))
        } else {
            Ok(None)
        }
    }

}
