use anyhow::Result;

#[derive(Debug)]
pub struct Filter {
    pub title: String,
    pub author: String,
    pub repo: String,
    pub state: String,
}

impl Filter {
    pub fn parse(input: &str) -> Result<Self> {
        let mut title = String::new();
        let mut author = String::new();
        let mut repo = String::new();
        let mut state = String::new();

        let known_keys = ["title:", "author:", "repo:", "state:"];
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
                    "repo:" => repo = value.to_string(),
                    "state:" => {
                        state = match value.to_lowercase().as_str() {
                            "open" => "Open".to_string(),
                            "draft" => "Draft".to_string(),
                            "resolved" => "Resolved".to_string(),
                            "canceled" => "Canceled".to_string(),
                            _ => String::new(), // Invalid state, keep empty
                        }
                    }
                    _ => (),
                }
            }
        }

        Ok(Filter {
            title,
            author,
            repo,
            state,
        })
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
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("author:John Doe").unwrap();
        assert!(filters.title.is_empty());
        assert_eq!(filters.author, "John Doe");
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("Rust Programming").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("Rust Programming author:John Doe").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert_eq!(filters.author, "John Doe");
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters =
            Filter::parse("Rust Programming author:John Doe repo:LedgerHQ/ledger-live").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert_eq!(filters.author, "John Doe");
        assert_eq!(filters.repo, "LedgerHQ/ledger-live");
        assert!(filters.state.is_empty());

        let filters = Filter::parse("title:Rust Programming state:open").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Open");

        let filters = Filter::parse("title:Rust Programming state:Open").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Open");

        let filters = Filter::parse("title:Rust Programming state:Resolved").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Resolved");

        let filters = Filter::parse("title:Rust Programming state:Resolved").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Resolved");

        let filters = Filter::parse("title:Rust Programming state:Pasfini").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());
    }
}
