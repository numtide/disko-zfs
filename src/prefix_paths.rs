use std::str::MatchIndices;

pub struct PrefixPaths<'a>(&'a str, MatchIndices<'a, char>);

impl<'a> PrefixPaths<'a> {
    pub fn new(string: &'a str) -> PrefixPaths<'a> {
        PrefixPaths(string, string.match_indices('/'))
    }
}

impl<'a> Iterator for PrefixPaths<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.1.next().map(|(index, _)| &self.0[0..index])
    }
}
