use anyhow::Result;

#[derive(Debug)]
pub struct Filter {
    pub title: String,
    pub author: String,
}

impl Filter {
    pub fn parse(_input: &str) -> Result<Self> {
        let title = String::new();
        let author = String::new();

        Ok(Filter { title, author })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse() {
        let filters = Filter::parse("title:Rustprogramming author:JohnDoe").unwrap();

        assert_eq!(filters.title, "Rustprogramming");
        assert_eq!(filters.author, "JohnDoe");
    }
}
