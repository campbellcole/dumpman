use std::path::{PathBuf, Path};

#[inline]
pub fn join<'a, S, T>(root: &'a S, iter: T) -> PathBuf
where
    S: 'a,
    PathBuf: From<&'a S>,
    T: IntoIterator,
    <T as IntoIterator>::Item: AsRef<Path>,
{
    let mut buf = PathBuf::from(root);
    buf.extend(iter);
    return buf;
}