use std::borrow::Cow;

pub fn basename(path: &'_ str) -> Cow<'_, str> {
    let mut pieces = path.rsplitn(2, |c| c == '/' || c == '\\');
    match pieces.next() {
        Some(p) => p.into(),
        None => path.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basename() {
        assert_eq!(basename("c:\\users\\fakeUser\\myfile.txt"), "myfile.txt");
        assert_eq!(basename("/home/fakeUser/myfile.txt"), "myfile.txt");
    }
}
