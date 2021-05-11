pub mod pak_file;
pub(crate) mod pak_reader;

macro_rules! get_size {
    ($e:ty) => {
        std::mem::size_of::<$e>() as isize
    };
}

#[derive(Debug, PartialEq, Eq)]
pub enum PakVersionSizes {
    // sizeof(Magic) + sizeof(Version) + sizeof(IndexOffset) + sizeof(IndexSize) + sizeof(IndexHash) + sizeof(EncryptedIndex) + sizeof(Guid)
    Size = get_size!(i32) * 2 + get_size!(u64) * 2 + 20 + 1 + 16,
    SizeV8 = PakVersionSizes::Size as isize + (32 * 4),
    SizeV8A = PakVersionSizes::SizeV8 as isize + 32,
    SizeV9 = PakVersionSizes::SizeV8A as isize + 1
}

pub(crate) enum PakVersions {
    Initial = 1,
    NoTimestamps = 2,
    CompressionEncryption = 3,
    IndexEncryption = 4,
    RelativeChunkOffsets = 5,
    DeleteRecords = 6,
    EncryptionKeyGuid = 7,
    FNameBasedCompressionMethod = 8,
    FrozenIndex = 9,

    Last,
}

impl PakVersionSizes {
    pub fn get_sizes() -> Vec<Self> {
        return vec![
            Self::Size,
            Self::SizeV8,
            Self::SizeV8A,
            Self::SizeV9
        ]
    }
}