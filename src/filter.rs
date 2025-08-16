use anyhow::Result;

#[derive(Debug)]
pub struct Filter {
    pub title: String,
    pub author: String,
}

impl Filter {
    pub fn parse(input: &str) -> Result<Self> {
        let mut title = String::new();
        let mut author = String::new();

        let known_keys = ["title:", "author:"];
        let mut key_positions: Vec<(&str, usize)> = known_keys
            .iter()
            .filter_map(|key| input.find(key).map(|pos| (*key, pos)))
            .collect();

        if key_positions.is_empty() {
            title = input.to_string();
        } else {
            key_positions.sort_by_key(|&(_, pos)| pos);

            if let Some((_key, first_pos)) = key_positions.first() {
                if *first_pos > 0 {
                    title = input[0..*first_pos].trim().to_string();
                }
            }

            for i in 0..key_positions.len() {
                let (key, start) = key_positions[i];
                let value_start = start + key.len();

                let value_end = if i + 1 < key_positions.len() {
                    let (_, next_start) = key_positions[i + 1];
                    next_start
                } else {
                    input.len()
                };

                let value = input[value_start..value_end].trim();

                match key {
                    "title:" => title = value.to_string(),
                    "author:" => author = value.to_string(),
                    _ => (),
                }
            }
        }

        Ok(Filter { title, author })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse() {
        let filters = Filter::parse("title:Rust Programming author:John Doe").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert_eq!(filters.author, "John Doe");

        let filters = Filter::parse("author:John Doe").unwrap();
        assert!(filters.title.is_empty());
        assert_eq!(filters.author, "John Doe");

        let filters = Filter::parse("Rust Programming").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());

        let filters = Filter::parse("Rust Programming author:John Doe").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert_eq!(filters.author, "John Doe");
    }
}
