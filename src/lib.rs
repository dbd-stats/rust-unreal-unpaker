extern crate tokio;

#[macro_use]
extern crate lazy_static;

mod cursor_ext;
pub mod pak;

use pak::*;

#[cfg(test)]
mod tests {
    use std::{fs::File, io::{Write, Read}};

    use super::pak_reader::PakReader;
    use super::pak_file::PakFile;

    lazy_static! {
        static ref TEST_FILE_BUFFER: Vec<u8> = get_test_buffer();
    }

    fn dump_buffer_to_file(name: &str, buffer: &Vec<u8>) {
        let mut file = File::create(name).unwrap();
        file.write_all(buffer.as_slice());
    }

    fn get_test_buffer() -> Vec<u8> {
        let mut buffer = Vec::new();
        let mut file = File::open("test-paks/pakchunk0-WindowsNoEditor.pak").unwrap();

        file.read_to_end(&mut buffer).unwrap();
        return buffer;
    }

    #[tokio::test]
    async fn read_pak_header() {
        let mut reader = PakReader::new(TEST_FILE_BUFFER.clone());
        reader.get_pak_info().await.unwrap();
    }

    #[tokio::test]
    async fn read_pak_data() {
        let pak = PakFile::from_memory(TEST_FILE_BUFFER.clone()).await.unwrap();
    }

    #[tokio::test]
    async fn read_pak_entry_data_uncompressed() {
        let mut pak = PakFile::from_memory(TEST_FILE_BUFFER.clone()).await.unwrap();
        let entry_data = pak.get_entry_data("DeadByDaylight/Content/Blueprints/Props/05-Suburbs/BP_Garbage1.uasset").await.unwrap().unwrap();
        dump_buffer_to_file("test-data/test-uncompressed.dat", &entry_data);
    }

    #[tokio::test]
    async fn read_pak_entry_data_lzib() {
        let mut pak = PakFile::from_memory(TEST_FILE_BUFFER.clone()).await.unwrap();
        let entry_data = pak.get_entry_data("DeadByDaylight/Config/DefaultGameUserSettings.ini").await.unwrap().unwrap();
        dump_buffer_to_file("test-data/test-zlib.dat", &entry_data);
    }
}
