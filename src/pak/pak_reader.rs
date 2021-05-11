use std::io::{Cursor, Read, Seek, SeekFrom};
use thiserror::Error;
use tokio_byteorder::{LittleEndian, BigEndian, AsyncReadBytesExt};
use std::collections::HashMap;

use super::pak_file::PakInfo;
use super::{PakVersions, PakVersionSizes};
use crate::cursor_ext::{CursorExt, DecompressType};
use crate::pak::pak_file::PakCompressionBlock;

// static PAK file magic
static PAK_MAGIC: u32 = 0x5A6F12E1;
static BUFFER_SIZE: u32 = 32 * 1024;

pub type PakData = (String, HashMap<String, PakEntry>);

#[derive(Error, Debug)]
pub enum PakReaderError {
    #[error("Error occurred while reading: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Error reading header")]
    ReadHeaderError,
    #[error("Mismatching magic header")]
    MagicMismatch,
    #[error("Unknown version")]
    UnknownVersion,
    #[error("Encryption not supported")]
    EncryptionNotSupported
}

#[derive(Debug)]
pub struct PakEntry {
    start: u64,
    offset: u64,
    size: u64,
    flags: u8,
    timestamp: u64,
    hash: Vec<u8>,
    uncompressed_size: u64,
    compression_index: u32,
    compression_block_size: u32,
    compression_blocks: Vec<PakCompressionBlock>,
    header_size: u64
}

#[derive(Debug)]
pub(crate) struct PakReader {
    reader: Cursor<Vec<u8>>,
}

impl PakReader {
    pub fn new(buffer: Vec<u8>) -> Self {
        Self {
            reader: Cursor::new(buffer)
        }
    }

    pub async fn get_pak_info(&mut self) -> Result<PakInfo, PakReaderError> {
        let sizes = PakVersionSizes::get_sizes();
        for size in sizes {
            let result = self.read_pak_info(size).await;
            if let Ok(info) = result {
                return Ok(info)
            }
        }

        Err(PakReaderError::UnknownVersion)
    }

    pub async fn get_pak_entries(&mut self, info: &PakInfo) -> Result<PakData, PakReaderError> {
        let index = self.read_pak_index(info).await?;
        let mut reader = Cursor::new(index);

        let mut entries = HashMap::new();
        let mut mount_point = reader.read_fstring().await?;
        let entry_count = reader.read_i32::<LittleEndian>().await?;

        mount_point = mount_point.replace("../../../", "");

        for _ in 0..entry_count {
            let file_name = reader.read_fstring().await?;
            let entry = self.read_pak_entry(&mut reader, info).await?;

            entries.insert(file_name, entry);
        }

        Ok((mount_point, entries))
    }

    pub async fn get_pak_entry_data(&mut self, entry: &PakEntry) -> Result<Vec<u8>, PakReaderError> {
        self.reader.set_position(entry.offset + entry.header_size);

        if entry.compression_index == 0 {
            Ok(self.reader.read_buffer(entry.size as usize).await?)
        } else {
            let mut index = 0;
            let mut offset = 0;
            let mut decompressed = vec![0u8; entry.uncompressed_size as usize];

            for block in &entry.compression_blocks {
                let uncompressed_block_size = (entry.uncompressed_size - entry.compression_block_size as u64 * index).min(entry.compression_block_size as u64);

                let compressed_size = block.get_size() as usize;
                let compressed_buffer = self.reader.read_buffer(uncompressed_block_size as usize).await?;
                let mut compression_reader = Cursor::new(compressed_buffer);
                let compression_method = match entry.compression_index {
                    1 => DecompressType::Zlib,
                    2 => DecompressType::GZip,
                    _ => panic!("invalid/unsupported compression index for compressed block")
                };

                let (bytes_read, decompressed_bytes) = compression_reader.read_decompress(compressed_size, compression_method).await?;
                decompressed.splice(offset..bytes_read, decompressed_bytes.iter().cloned());

                offset += bytes_read;
                index += 1;
            }

            Ok(decompressed)
        }

    }

    async fn read_pak_entry(&mut self, reader: &mut Cursor<Vec<u8>>, info: &PakInfo) -> Result<PakEntry, PakReaderError> {
        let start = reader.position();
        let offset = reader.read_u64::<LittleEndian>().await?;
        let size = reader.read_u64::<LittleEndian>().await?;
        let uncompressed_size = reader.read_u64::<LittleEndian>().await?;
        let mut compression_index: u32 = 0;
        let mut compression_block_size = 0;
        let mut flags = 0;
        let mut timestamp = 0;
        let mut compression_blocks = vec![];

        if info.version >= PakVersions::FNameBasedCompressionMethod as i32 {
            if info.sub_version == 1 {
                compression_index = reader.read_u8().await? as u32;
            } else {
                compression_index = reader.read_u32::<LittleEndian>().await?;
            }
        } else {
            let compression_flags = reader.read_u32::<LittleEndian>().await?;
            if compression_flags == 0 { // No commpression
                compression_index = 0;
            } else if compression_flags & 0x01 != 0 { // Zlib compression
                compression_index = 1;
            } else if compression_flags & 0x02 != 0 { // GZip Compression
                compression_index = 2;
            } else if compression_flags & 0x04 != 0 { // Custom Compression
                compression_index = 3;
            }
        }

        if info.version < PakVersions::NoTimestamps as i32 {
            timestamp = reader.read_u64::<LittleEndian>().await?;
        }

        // read hash
        let hash = reader.read_buffer(20).await?;

        if info.version >= PakVersions::CompressionEncryption as i32 {
            if compression_index != 0 {
                let size = reader.read_i32::<LittleEndian>().await?;
                if size > 0 {
                    for _ in 0..size {
                        let compression_start = reader.read_i64::<LittleEndian>().await?;
                        let compression_end = reader.read_i64::<LittleEndian>().await?;
                        compression_blocks.push(PakCompressionBlock { compression_start, compression_end });
                    }
                }
            }

            flags = reader.read_u8().await?;
            compression_block_size = reader.read_u32::<LittleEndian>().await?;
        }

        let header_size = reader.position() - start;

        Ok(PakEntry {
            start,
            offset,
            size,
            flags,
            timestamp,
            hash,
            uncompressed_size,
            compression_index,
            compression_block_size,
            compression_blocks,
            header_size
        })
    }

    async fn read_pak_index(&mut self, info: &PakInfo) -> Result<Vec<u8>, PakReaderError> {
        let position = self.reader.position();
        self.reader.seek_index(info.index_offset).await;
        let buffer = self.reader.read_buffer(info.index_size as usize).await?;
        self.reader.set_position(position);
        // TODO: decrypt memory
        Ok(buffer)
    }

    async fn read_pak_info(&mut self, version_size: PakVersionSizes) -> Result<PakInfo, PakReaderError> {
        // start reading from the end
        let version_size = version_size as usize;
        self.reader.seek(SeekFrom::End(-(version_size as i64)))?;

        // initialize buffer, and reader
        let mut header = self.reader.read_buffer(version_size).await?;
        let mut reader = &mut Cursor::new(header);

        // reset the main cursor back to the start
        self.reader.seek(SeekFrom::Start(0));

        // read the encryption guid
        let mut encryption_index_guid =  reader.read_buffer(16).await?;

        // is the pak file encrypted?
        let mut is_encrypted = reader.read_u8().await? > 0;
        let magic = reader.read_u32::<LittleEndian>().await?;

        // if the magic doesn't match, theres no point continuing
        if magic != PAK_MAGIC {
            return Err(PakReaderError::MagicMismatch);
        }

        let mut compression = vec![];
        let mut index_frozen = 0u8;
        let version = reader.read_i32::<LittleEndian>().await?;
        let index_offset = reader.read_i64::<LittleEndian>().await?;
        let index_size = reader.read_i64::<LittleEndian>().await?;
        let index_hash = reader.read_buffer( 20).await?;
        let sub_version = if version_size == PakVersionSizes::SizeV8A as usize && version == 8 {
            1
        } else {
            0
        };

        if version < PakVersions::IndexEncryption as i32 {
            is_encrypted = false;
        }

        if version < PakVersions::EncryptionKeyGuid as i32 {
            encryption_index_guid = Vec::default();
        }

        if version >= PakVersions::FrozenIndex as i32 {
            index_frozen = reader.read_u8().await?;
        }

        if version < PakVersions::FNameBasedCompressionMethod as i32 {
            compression = vec!["Zlib".into(), "Gzip".into(), "Oodle".into()];
        } else {
            let mut start = 0;
            let mut buffer = vec![];
            let remaining = version_size - reader.position() as usize;

            for idx in 0..remaining {
                let char = reader.read_u8().await?;
                if char == 0 {
                    if buffer.len() > 0 {
                        compression.push(buffer.iter().collect());
                        buffer.clear();
                    }

                    start = idx + 1;
                } else {
                    buffer.push(char as char);
                }
            }
        }

        if is_encrypted {
            return Err(PakReaderError::EncryptionNotSupported)
        }

        Ok(PakInfo {
            encryption_index_guid,
            is_encrypted,
            magic,
            version,
            index_offset,
            index_size,
            index_hash,
            index_frozen,
            sub_version,
            compression_methods: compression,
        })
    }
}

impl PakCompressionBlock {
    pub fn get_size(&self) -> i64 {
        self.compression_end - self.compression_start
    }
}