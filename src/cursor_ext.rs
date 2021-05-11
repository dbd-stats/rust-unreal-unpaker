use async_trait::async_trait;
use std::io::{Cursor, Error, Read};
use tokio_byteorder::{AsyncReadBytesExt, LittleEndian};
use tokio::io::{AsyncRead, SeekFrom, AsyncSeekExt};
use compress::*;

#[derive(Debug)]
pub enum DecompressType {
    Zlib,
    GZip
}

#[async_trait]
pub trait CursorExt: AsyncRead {
    async fn read_decompress(&mut self, size: usize, decompress_type: DecompressType) -> Result<(usize, Vec<u8>), Error>;
    async fn read_fstring(&mut self) -> Result<String, Error>;
    async fn read_buffer(&mut self, size: usize) -> Result<Vec<u8>, Error>;
    async fn seek_index(&mut self, index: i64);
}

#[async_trait]
impl<T: AsRef<[u8]> + Unpin + Send> CursorExt for Cursor<T> {
    async fn read_decompress(&mut self, size: usize, decompress_type: DecompressType) -> Result<(usize, Vec<u8>), Error> {
        let buffer = self.read_buffer(size).await?;
        let mut reader = Cursor::new(buffer);
        let mut decompressed = Vec::new();
        let mut bytes_read = 0;

        match decompress_type {
            DecompressType::Zlib => {
                let mut decoder = zlib::Decoder::new(reader);
                bytes_read = decoder.read_to_end(&mut decompressed)?;
            },
            DecompressType::GZip => {
                let mut decoder = flate::Decoder::new(reader);
                bytes_read = decoder.read_to_end(&mut decompressed)?;
            },
            _ => panic!("Invalid decompression type: {:?}", decompress_type)
        }

        Ok((bytes_read, decompressed))
    }

    async fn read_fstring(&mut self) -> Result<String, Error> {
        let mut len = self.read_i32::<LittleEndian>().await?;
        let mut data = String::default();

        if len > 0 {
            for _ in 0..len-1 {
                let char = self.read_u8().await?;
                data.push(char as char);
            }

            // discard the last char (\0)
            let _ = self.read_u8().await?;
        } else {
            len = -len;
            for _ in 0..len {
                let mut raw_char = self.read_u16::<LittleEndian>().await?;
                if raw_char & 0xff00 != 0 {
                    raw_char = '$' as u16;
                }

                let char = (raw_char & 255) as u8;
                data.push(char as char);
            }
        }

        Ok(data)
    }

    async fn read_buffer(&mut self, size: usize) -> Result<Vec<u8>, Error> {
        let mut buffer: Vec<u8> = vec![0u8; size];
        tokio::io::AsyncReadExt::read_exact(self, &mut buffer).await?;
        Ok(buffer)
    }

    async fn seek_index(&mut self, index: i64) {
        if index < 0 {
            self.seek(SeekFrom::End(index)).await;
        } else {
            self.seek(SeekFrom::Start(index as u64)).await;
        }
    }
}