pub trait IterExt: Iterator + Sized {
    fn zip_exact<R: Iterator>(
        self,
        rights: impl IntoIterator<Item = R::Item, IntoIter = R>,
    ) -> LengthCheckingZipIterator<Self, R> {
        zip_exact(self, rights)
    }

    fn only_element(self) -> Option<Self::Item> {
        only_element(self)
    }
}
impl<T: Iterator> IterExt for T {}

pub fn only_element<T>(iter: impl IntoIterator<Item = T>) -> Option<T> {
    let mut the_only = None;
    for element in iter.into_iter() {
        if the_only.is_some() {
            return None;
        }
        the_only = Some(element);
    }
    the_only
}

pub fn zip_exact<L: Iterator, R: Iterator>(
    lefts: impl IntoIterator<Item = L::Item, IntoIter = L>,
    rights: impl IntoIterator<Item = R::Item, IntoIter = R>,
) -> LengthCheckingZipIterator<L, R> {
    LengthCheckingZipIterator {
        lefts: lefts.into_iter(),
        rights: rights.into_iter(),
        errored: false,
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct LengthMismatchError;

pub type LengthCheckedResult<L, R> = Result<(L, R), LengthMismatchError>;

pub struct LengthCheckingZipIterator<L, R> {
    lefts: L,
    rights: R,
    errored: bool,
}

impl<L: Iterator, R: Iterator> Iterator for LengthCheckingZipIterator<L, R> {
    type Item = LengthCheckedResult<L::Item, R::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.errored {
            return None;
        }
        match (self.lefts.next(), self.rights.next()) {
            (None, None) => None,
            (Some(left), Some(right)) => Some(Ok((left, right))),
            _different => {
                self.errored = true;
                Some(Err(LengthMismatchError))
            }
        }
    }
}
