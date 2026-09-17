use std::path::PathBuf;

use rkyv::{
    Fallible,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
};

pub struct PathAsString;

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
