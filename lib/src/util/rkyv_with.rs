use std::path::PathBuf;

use rkyv::{
    Fallible,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
};
use uuid::Uuid;

/// `PathBuf` を rkyv でシリアライズ/デシリアライズする際に、
/// `ArchivedString` として扱うためのアダプタです。
///
/// `to_string_lossy()` を使用することで、非 UTF-8 なパスでもエラーを出さずに
/// 文字列化できます。
pub struct PathAsString;

pub struct UuidAsBytes;

pub struct HashAsBytes;

impl ArchiveWith<blake3::Hash> for HashAsBytes {
    type Archived = <[u8; 32] as rkyv::Archive>::Archived;
    type Resolver = <[u8; 32] as rkyv::Archive>::Resolver;

    unsafe fn resolve_with(
        field: &blake3::Hash,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        unsafe {
            rkyv::Archive::resolve(field.as_bytes(), pos, resolver, out);
        }
    }
}

impl<S: rkyv::ser::Serializer + ?Sized> SerializeWith<blake3::Hash, S> for HashAsBytes {
    fn serialize_with(
        field: &blake3::Hash,
        serializer: &mut S,
    ) -> Result<<Self as ArchiveWith<blake3::Hash>>::Resolver, S::Error> {
        rkyv::Serialize::serialize(field.as_bytes(), serializer)
    }
}

impl<D: Fallible + ?Sized>
    DeserializeWith<<[u8; 32] as rkyv::Archive>::Archived, blake3::Hash, D> for HashAsBytes
{
    fn deserialize_with(
        archived: &<[u8; 32] as rkyv::Archive>::Archived,
        deserializer: &mut D,
    ) -> Result<blake3::Hash, D::Error> {
        Ok(blake3::Hash::from_bytes(rkyv::Deserialize::deserialize(
            archived,
            deserializer,
        )?))
    }
}

impl ArchiveWith<Uuid> for UuidAsBytes {
    type Archived = <[u8; 16] as rkyv::Archive>::Archived;
    type Resolver = <[u8; 16] as rkyv::Archive>::Resolver;

    unsafe fn resolve_with(
        field: &Uuid,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        unsafe {
            rkyv::Archive::resolve(field.as_bytes(), pos, resolver, out);
        }
    }
}

impl<S: rkyv::ser::Serializer + ?Sized> SerializeWith<Uuid, S> for UuidAsBytes {
    fn serialize_with(
        field: &Uuid,
        serializer: &mut S,
    ) -> Result<<Self as ArchiveWith<Uuid>>::Resolver, S::Error> {
        rkyv::Serialize::serialize(field.as_bytes(), serializer)
    }
}

impl<D: Fallible + ?Sized>
    DeserializeWith<<[u8; 16] as rkyv::Archive>::Archived, Uuid, D> for UuidAsBytes
{
    fn deserialize_with(
        archived: &<[u8; 16] as rkyv::Archive>::Archived,
        deserializer: &mut D,
    ) -> Result<Uuid, D::Error> {
        Ok(Uuid::from_bytes(rkyv::Deserialize::deserialize(
            archived,
            deserializer,
        )?))
    }
}

impl ArchiveWith<PathBuf> for PathAsString {
    type Archived = rkyv::string::ArchivedString;
    type Resolver = rkyv::string::StringResolver;

    unsafe fn resolve_with(
        field: &PathBuf,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        // to_string_lossy() を使うことで、非UTF-8なパスでもエラーを出さずに文字列化
        let s = field.to_string_lossy();
        unsafe { rkyv::string::ArchivedString::resolve_from_str(&s, pos, resolver, out) };
    }
}

impl<S: rkyv::ser::Serializer + ?Sized> SerializeWith<PathBuf, S> for PathAsString {
    fn serialize_with(
        field: &PathBuf,
        serializer: &mut S,
    ) -> Result<<PathAsString as ArchiveWith<PathBuf>>::Resolver, S::Error> {
        let s = field.to_string_lossy();
        rkyv::string::ArchivedString::serialize_from_str(&s, serializer)
    }
}

impl<D: Fallible + ?Sized> DeserializeWith<rkyv::string::ArchivedString, PathBuf, D>
    for PathAsString
{
    fn deserialize_with(
        archived: &rkyv::string::ArchivedString,
        _: &mut D,
    ) -> Result<PathBuf, D::Error> {
        Ok(PathBuf::from(archived.as_str()))
    }
}
